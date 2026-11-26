//! Build script for `audiocpp-sys`.
//!
//! Source build (the default): CMake builds the pinned audio.cpp tree in
//! `audio.cpp/` (a git submodule) into ONE shared library, `audiocpp.dll` /
//! `libaudiocpp.so` / `libaudiocpp.dylib`, which carries its own ggml and
//! exports nothing but the `audiocpp_*` C ABI. Only the `audiocpp` target is
//! built: audio.cpp's default target also builds its CLI, server and tools.
//!
//! Cargo features -> CMake:
//!   model-<target>  -> AUDIOCPP_MODELS += <target> (AUDIOCPP_MODEL_SET=custom)
//!   all-models      -> AUDIOCPP_MODEL_SET=full
//!   (no model)      -> AUDIOCPP_MODEL_SET=core: the runtime, no model family
//!   openmp          -> ENGINE_ENABLE_OPENMP
//!   native          -> ENGINE_ENABLE_NATIVE_CPU (OFF = ggml's portable AVX2-class floor)
//!   metal / vulkan / cuda / hip -> ENGINE_ENABLE_* (metal on Apple targets only)
//!   gpl-espeak      -> AUDIOCPP_STATIC_ESPEAK
//! Always: AUDIOCPP_BUILD_C_API=ON, AUDIOCPP_DEPLOYMENT_BUILD=ON (the model
//! specs are compiled in: nothing has to ship next to the app), tests off.
//!
//! Environment:
//!   AUDIOCPP_DIR           link a prebuilt library from this directory; skip the build
//!   AUDIOCPP_SOURCE_DIR    build this audio.cpp tree instead of the submodule
//!   AUDIOCPP_MODELS        extra model targets, comma separated
//!   AUDIOCPP_MODEL_SET     force full | core | custom
//!   AUDIOCPP_CMAKE_ARGS    extra configure arguments (applied last, so they win)
//!   AUDIOCPP_CMAKE_PROFILE CMake build type (default Release, whatever the Cargo profile)
//!   AUDIOCPP_CUDA_ARCHITECTURES  CMAKE_CUDA_ARCHITECTURES for `cuda` (default: native)
//!   AUDIOCPP_NO_NINJA / AUDIOCPP_NO_CCACHE  opt out of Ninja / the compiler cache
//!
//! The Windows parts (forced MSVC optimisation, Ninja through vcvars64, the
//! short NTFS junction, DLL staging) follow transcribe-cpp-sys's build.rs, where
//! each one fixed a measured failure; the comments say which.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LIB: &str = "audiocpp";

fn feature(name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{}", name.to_ascii_uppercase().replace('-', "_"))).is_some()
}

fn on_off(on: bool) -> &'static str {
    if on {
        "ON"
    } else {
        "OFF"
    }
}

fn main() {
    for var in [
        "AUDIOCPP_DIR",
        "AUDIOCPP_SOURCE_DIR",
        "AUDIOCPP_MODELS",
        "AUDIOCPP_MODEL_SET",
        "AUDIOCPP_CMAKE_ARGS",
        "AUDIOCPP_CMAKE_PROFILE",
        "AUDIOCPP_CUDA_ARCHITECTURES",
        "AUDIOCPP_NO_NINJA",
        "AUDIOCPP_NO_CCACHE",
        "CMAKE_GENERATOR",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-changed=build.rs");

    // docs.rs and similar: the Rust declarations need no native library.
    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let lib_dir = match env::var_os("AUDIOCPP_DIR") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if !dir.join(library_file(&target_os)).is_file() {
                panic!("audiocpp-sys: AUDIOCPP_DIR={} does not contain {}", dir.display(), library_file(&target_os));
            }
            dir
        }
        None => build_from_source(&target_os),
    };
    emit_link(&lib_dir, &target_os);
}

fn library_file(target_os: &str) -> &'static str {
    match target_os {
        "windows" => "audiocpp.dll",
        "macos" | "ios" => "libaudiocpp.dylib",
        _ => "libaudiocpp.so",
    }
}

fn selected_models() -> Vec<String> {
    let mut models: Vec<String> = env::vars()
        .filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_MODEL_").map(str::to_ascii_lowercase))
        .collect();
    if let Ok(extra) = env::var("AUDIOCPP_MODELS") {
        models.extend(extra.split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
    }
    models.sort();
    models.dedup();
    models
}

fn describe(src: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(src)
        .args(["describe", "--tags", "--match", "v*", "--always", "--abbrev=8"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_from_source(target_os: &str) -> PathBuf {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = env::var_os("AUDIOCPP_SOURCE_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("audio.cpp"));
    if !src.join("include").join("audiocpp.h").is_file() {
        panic!(
            "audiocpp-sys: no audio.cpp sources at {} (include/audiocpp.h is missing).\n  \
             In a git checkout: git submodule update --init audiocpp-sys/audio.cpp\n  \
             Or set AUDIOCPP_SOURCE_DIR to an audio.cpp tree, or AUDIOCPP_DIR to a prebuilt library.",
            src.display()
        );
    }
    for file in ["include/audiocpp.h", "CMakeLists.txt", "src/capi/audiocpp.cpp"] {
        println!("cargo:rerun-if-changed={}", src.join(file).display());
    }
    watch_checkout(&src);

    let models = selected_models();
    let model_set = env::var("AUDIOCPP_MODEL_SET").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if feature("all-models") {
            "full".into()
        } else if models.is_empty() {
            "core".into()
        } else {
            "custom".into()
        }
    });
    let version = describe(&src);
    println!(
        "cargo:warning=audiocpp-sys: building audio.cpp {version} (model set {model_set}: [{}])",
        models.join(", ")
    );

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let is_apple = matches!(target_os, "macos" | "ios");
    let profile = env::var("AUDIOCPP_CMAKE_PROFILE").unwrap_or_else(|_| "Release".into());

    let mut cfg = cmake::Config::new(&src);
    cfg.profile(&profile)
        .build_target(LIB)
        .define("AUDIOCPP_BUILD_C_API", "ON")
        .define("AUDIOCPP_DEPLOYMENT_BUILD", "ON")
        .define("AUDIOCPP_MODEL_SET", &model_set)
        .define("AUDIOCPP_MODELS", models.join(","))
        .define("AUDIOCPP_VERSION", &version)
        .define("ENGINE_BUILD_TESTS", "OFF")
        .define("ENGINE_BUILD_EXAMPLES", "OFF")
        .define("ENGINE_BUILD_MODEL_TESTS", "OFF")
        .define("ENGINE_BUILD_EXTENDED_TESTS", "OFF")
        .define("ENGINE_BUILD_WARMBENCH", "OFF")
        // audio.cpp defaults this ON: a library built for the build machine's ISA
        // crashes (SIGILL) on older CPUs. Off = ggml's portable floor.
        .define("ENGINE_ENABLE_NATIVE_CPU", on_off(feature("native")))
        // Apple's clang has no OpenMP; audio.cpp's own macOS builds turn it off too. ggml then
        // uses its own thread pool.
        .define("ENGINE_ENABLE_OPENMP", on_off(feature("openmp") && !is_apple))
        .define("GGML_OPENMP", on_off(feature("openmp") && !is_apple))
        .define("ENGINE_ENABLE_VULKAN", on_off(feature("vulkan")))
        .define("ENGINE_ENABLE_CUDA", on_off(feature("cuda")))
        .define("ENGINE_ENABLE_HIP", on_off(feature("hip")))
        .define("ENGINE_ENABLE_METAL", on_off(is_apple && feature("metal")))
        .define("AUDIOCPP_STATIC_ESPEAK", on_off(feature("gpl-espeak")))
        // ggml's own cache hook wraps every rule, the Windows resource compiler
        // included, which sccache cannot run. The launcher below covers C/C++.
        .define("GGML_CCACHE", "OFF");

    // MSVC: give CMake its own default flags explicitly. Left unset, cmake-rs
    // fills CMAKE_<LANG>_FLAGS with the cc crate's ("-nologo -MD -Brepro -W0"),
    // which drops CMake's "/EHsc /DWIN32 /D_WINDOWS": without /EHsc a C++
    // exception no longer runs destructors while it unwinds, and audio.cpp's C
    // ABI turns every exception into a status (transcribe.cpp re-adds /EHs in its
    // own CMakeLists; audio.cpp relies on CMake's default). Under the Visual
    // Studio generator cmake-rs also overwrites the Release flags without /O2
    // (transcribe-cpp-sys measured ~7x slower ggml). cmake-rs only fills these
    // variables when they are not defined, so defining them wins. Elsewhere the
    // cc flags stay: they carry the target flags a cross-compile needs.
    if target_env == "msvc" {
        cfg.define("CMAKE_C_FLAGS", "/DWIN32 /D_WINDOWS")
            .define("CMAKE_CXX_FLAGS", "/DWIN32 /D_WINDOWS /EHsc")
            .define("CMAKE_C_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG")
            .define("CMAKE_CXX_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG");
    }

    if feature("cuda") {
        let arch = env::var("AUDIOCPP_CUDA_ARCHITECTURES").unwrap_or_else(|_| "native".into());
        cfg.define("CMAKE_CUDA_ARCHITECTURES", arch);
    }

    // Ninja (fast, and honours compiler launchers). On Windows it needs the
    // MSVC environment, which is loaded from vcvars64.bat when cl.exe is absent.
    let mut using_ninja = false;
    if env::var_os("AUDIOCPP_NO_NINJA").is_none()
        && env::var_os("CMAKE_GENERATOR").is_none()
        && setup_msvc_environment()
    {
        if let Some(ninja) = find_ninja() {
            cfg.generator("Ninja");
            if ninja.is_absolute() {
                cfg.define("CMAKE_MAKE_PROGRAM", ninja.to_string_lossy().replace('\\', "/"));
            }
            using_ninja = true;
        }
    }
    if using_ninja && env::var_os("AUDIOCPP_NO_CCACHE").is_none() {
        if let Some(cache) = find_ccache() {
            let cache = cache.to_string_lossy().replace('\\', "/");
            cfg.define("CMAKE_C_COMPILER_LAUNCHER", &cache);
            cfg.define("CMAKE_CXX_COMPILER_LAUNCHER", &cache);
        }
    }

    if let Ok(extra) = env::var("AUDIOCPP_CMAKE_ARGS") {
        for arg in split_cmake_args(&extra) {
            cfg.configure_arg(arg);
        }
    }

    // Windows MAX_PATH: build through a short junction to OUT_DIR.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let short = windows_short_out_dir(&out_dir);
    if let Some(short) = &short {
        cfg.out_dir(short);
    }
    clean_if_generator_changed(&short.clone().unwrap_or_else(|| out_dir.clone()).join("build"), using_ninja);

    cfg.build();

    // Resolve the artifacts through the real OUT_DIR (the junction points at
    // it): cargo puts link-search dirs inside the target dir on PATH /
    // LD_LIBRARY_PATH for `cargo test` and `cargo run`, and the junction is not.
    let build = out_dir.join("build");
    let file = library_file(target_os);
    let lib_dir = [build.join("bin"), build.join("bin").join(&profile)]
        .into_iter()
        .find(|dir| dir.join(file).is_file())
        .unwrap_or_else(|| panic!("audiocpp-sys: the build produced no {file} under {}", build.join("bin").display()));

    println!("cargo:include={}", src.join("include").display());
    println!("cargo:version={version}");
    println!("cargo:models={}", models.join(","));
    stage_bundled_assets(&src, &lib_dir);
    lib_dir
}

/// Rerun when the audio.cpp checkout moves (a pin bump), whatever files the move changes: watch
/// the checkout's HEAD, and the branch it points at. A tree without git keeps the file triggers.
fn watch_checkout(src: &Path) {
    let Ok(out) = Command::new("git").arg("-C").arg(src).args(["rev-parse", "--absolute-git-dir"]).output() else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let git_dir = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    if let Some(reference) =
        fs::read_to_string(&head).ok().and_then(|h| h.strip_prefix("ref: ").map(|r| r.trim().to_string()))
    {
        println!("cargo:rerun-if-changed={}", git_dir.join(reference).display());
    }
}

/// audio.cpp assets that its defaults point at, relative to its source tree: the Silero VAD
/// model that ASR families use for VAD chunking (`<family>.vad_model_path`, 1.2 MB).
const BUNDLED_ASSETS: &str = "assets/framework/models/silero_vad";

/// audio.cpp looks for a default asset relative to the working directory, then beside the
/// library and up to three directories above it (audio.cpp #1403). Copy the assets beside
/// the library, so the defaults work wherever the host process runs.
fn stage_bundled_assets(src: &Path, lib_dir: &Path) {
    let from = src.join(BUNDLED_ASSETS);
    if from.is_dir() {
        if let Err(e) = copy_dir(&from, &lib_dir.join(BUNDLED_ASSETS)) {
            println!("cargo:warning=audiocpp-sys: could not stage {BUNDLED_ASSETS} beside the library: {e}");
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn emit_link(lib_dir: &Path, target_os: &str) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib={LIB}");
    // Dependents' build scripts read this as DEP_AUDIOCPP_LIB_DIR, to copy the
    // library into an app bundle.
    println!("cargo:lib_dir={}", lib_dir.display());
    if target_os == "windows" {
        stage_windows_dll(lib_dir);
    }
}

/// Windows has no rpath: a process finds audiocpp.dll on PATH or next to the
/// exe. `cargo test` / `cargo run` put the link-search dir on PATH; copies
/// next to the profile's bins and examples cover programs launched directly.
/// The profile root is the parent of OUT_DIR's `build` ancestor, which holds for
/// both Cargo's old and its 1.100 build-dir layouts (transcribe.cpp #264).
fn stage_windows_dll(lib_dir: &Path) {
    let dll = lib_dir.join("audiocpp.dll");
    if !dll.is_file() {
        return;
    }
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let Some(profile_dir) =
        out_dir.ancestors().skip(1).find(|p| p.file_name().is_some_and(|n| n == "build")).and_then(Path::parent)
    else {
        return;
    };
    let assets = lib_dir.join(BUNDLED_ASSETS);
    for sub in ["", "deps", "examples"] {
        let dir = profile_dir.join(sub);
        if dir.is_dir() || sub.is_empty() {
            let _ = fs::create_dir_all(&dir);
            if let Err(e) = fs::copy(&dll, dir.join("audiocpp.dll")) {
                println!("cargo:warning=audiocpp-sys: could not stage audiocpp.dll in {}: {e}", dir.display());
            }
            // The staged DLL is the one a process loads, so its defaults look beside it.
            if assets.is_dir() {
                if let Err(e) = copy_dir(&assets, &dir.join(BUNDLED_ASSETS)) {
                    println!("cargo:warning=audiocpp-sys: could not stage {BUNDLED_ASSETS} in {}: {e}", dir.display());
                }
            }
        }
    }
}

/// Split a CMake argument string, honouring double quotes.
fn split_cmake_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A CMake cache from another generator or another directory makes the next
/// configure fail; start that build tree over.
fn clean_if_generator_changed(build_dir: &Path, using_ninja: bool) {
    let cache = build_dir.join("CMakeCache.txt");
    let Ok(text) = fs::read_to_string(&cache) else {
        return;
    };
    let has_ninja = text.contains("CMAKE_GENERATOR:INTERNAL=Ninja");
    let here = build_dir.to_string_lossy().replace('\\', "/");
    let moved = text.lines().any(|line| {
        line.strip_prefix("CMAKE_CACHEFILE_DIR:INTERNAL=").is_some_and(|dir| !dir.trim().eq_ignore_ascii_case(&here))
    });
    if has_ninja != using_ninja || moved {
        let _ = fs::remove_dir_all(build_dir);
    }
}

/// Windows MAX_PATH mitigation: a short NTFS junction (%LOCALAPPDATA%\acpp\<hash>,
/// no admin rights needed) resolving to OUT_DIR. MSBuild's FileTracker ignores
/// LongPathsEnabled, so the build root itself must be short. Best effort: None
/// builds in OUT_DIR directly.
fn windows_short_out_dir(out_dir: &Path) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let out_dir = PathBuf::from(out_dir.to_string_lossy().replace('/', "\\"));
    let base = env::var_os("LOCALAPPDATA").or_else(|| env::var_os("TEMP")).map(PathBuf::from)?.join("acpp");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a: stable across rustc versions
    for b in out_dir.to_string_lossy().bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let link = base.join(format!("{hash:016x}"));
    fs::create_dir_all(&out_dir).ok()?;
    if fs::symlink_metadata(&link).is_ok() {
        if same_dir(&link, &out_dir) {
            return Some(link);
        }
        fs::remove_dir(&link).ok()?;
    }
    fs::create_dir_all(&base).ok()?;
    let created = Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&link)
        .arg(&out_dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if created && same_dir(&link, &out_dir) {
        Some(link)
    } else {
        println!(
            "cargo:warning=audiocpp-sys: no short build junction; building in OUT_DIR (deep paths may hit MAX_PATH)"
        );
        None
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

fn find_ninja() -> Option<PathBuf> {
    if Command::new("ninja").arg("--version").output().is_ok_and(|o| o.status.success()) {
        return Some(PathBuf::from("ninja"));
    }
    // Visual Studio ships one.
    let vs = vs_install_dir()?;
    let ninja = vs.join(r"Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja\ninja.exe");
    ninja.is_file().then_some(ninja)
}

fn find_ccache() -> Option<PathBuf> {
    ["sccache", "ccache"]
        .into_iter()
        .find(|bin| Command::new(bin).arg("--version").output().is_ok_and(|o| o.status.success()))
        .map(PathBuf::from)
}

fn vs_install_dir() -> Option<PathBuf> {
    let pf86 = env::var_os("ProgramFiles(x86)").map(PathBuf::from)?;
    let vswhere = pf86.join(r"Microsoft Visual Studio\Installer\vswhere.exe");
    let out = Command::new(vswhere)
        .args([
            "-latest",
            "-prerelease",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let dir = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

/// On Windows, make cl.exe reachable for the Ninja generator by importing the
/// environment of vcvars64.bat into this process (CMake and Ninja inherit it).
/// True when the environment is usable (always, off Windows).
fn setup_msvc_environment() -> bool {
    if !cfg!(windows) || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return true;
    }
    if Command::new("cl.exe").output().is_ok() {
        return true;
    }
    let Some(vs) = vs_install_dir() else {
        return false;
    };
    let vcvars = vs.join(r"VC\Auxiliary\Build\vcvars64.bat");
    if !vcvars.is_file() {
        return false;
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/d", "/s", "/c"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("\"\"{}\" >nul && set\"", vcvars.display()));
    }
    let Ok(out) = cmd.output() else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty() && key != "PROMPT" {
                env::set_var(key, value.trim());
            }
        }
    }
    true
}
