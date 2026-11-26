# Building

`audiocpp-sys/build.rs` builds audio.cpp from the pinned submodule (`audiocpp-sys/audio.cpp`) with CMake, through the [`cmake`](https://crates.io/crates/cmake) crate, into **one shared library** that contains audio.cpp, its ggml and the selected model families, and exports only the `audiocpp_*` functions.

## Requirements

| Platform | Needs |
|---|---|
| all | Rust 1.80+, CMake 3.20+, a C++17 compiler, git (for the submodule). Ninja is used when found (faster). |
| Windows | Visual Studio 2022 or newer with the C++ workload. The build finds it through `vswhere` and loads `vcvars64.bat` itself, so a plain terminal works. |
| Linux | gcc or clang; `libgomp` for the `openmp` feature. Vulkan: the Vulkan SDK / headers + `glslc`. |
| macOS | Xcode command-line tools. Metal is on by default. The `openmp` feature has no effect: Apple's clang has no OpenMP, so ggml uses its own thread pool, as in audio.cpp's macOS releases. |
| CUDA | the CUDA toolkit; `AUDIOCPP_CUDA_ARCHITECTURES` (default `native`: the GPU of the build machine). |

## What the build does

1. Picks the model set: every enabled `model-<target>` feature plus `AUDIOCPP_MODELS`, as `AUDIOCPP_MODEL_SET=custom`. `all-models` means `full`. No model means `core` (the runtime with no family).
2. Configures audio.cpp with:
   - `AUDIOCPP_BUILD_C_API=ON` and `AUDIOCPP_DEPLOYMENT_BUILD=ON` (the model specs are compiled in: no `model_specs/` folder ships);
   - tests, examples and benches off;
   - `ENGINE_ENABLE_NATIVE_CPU=OFF` unless the `native` feature is set (audio.cpp defaults it ON; a library tuned to the build machine crashes with SIGILL on older CPUs);
   - the backend features as `ENGINE_ENABLE_*`.
3. Builds **only the `audiocpp` target**: audio.cpp's default target also builds its CLI, server, GGUF tool and benchmarks.
4. Always in `Release` (`AUDIOCPP_CMAKE_PROFILE` to change it): a debug audio.cpp is unusably slow, whatever the Cargo profile.
5. Links the library and puts its folder on the link search path. Cargo then adds it to `PATH` / `LD_LIBRARY_PATH` for `cargo run` and `cargo test`. On Windows, `audiocpp.dll` is also copied next to the profile's binaries and examples.

### Windows specifics

They come from `transcribe-cpp-sys`'s build script, where each fixed a measured failure:

- **CMake's own MSVC flags, set explicitly.** Left alone, `cmake-rs` fills `CMAKE_C_FLAGS` / `CMAKE_CXX_FLAGS` with the cc crate's flags (`-nologo -MD -Brepro -W0`). That drops CMake's defaults `/EHsc /DWIN32 /D_WINDOWS`. Without `/EHsc`, a C++ exception does not run destructors while it unwinds, and audio.cpp's C API catches every exception to turn it into a status. Under the Visual Studio generator, `cmake-rs` also overwrites the Release flags without `/O2` (about 7× slower ggml, measured in `transcribe-cpp-sys`). The build therefore defines `CMAKE_<LANG>_FLAGS` and `CMAKE_<LANG>_FLAGS_RELEASE` to CMake's own MSVC defaults. transcribe.cpp protects itself in its CMakeLists; audio.cpp relies on CMake's defaults.
- **Ninja through vcvars.** When `cl.exe` is not on `PATH`, the build imports `vcvars64.bat`'s environment and uses Ninja (on `PATH`, or the one Visual Studio ships). `AUDIOCPP_NO_NINJA=1` forces the default generator.
- **Short build root.** The build runs through an NTFS junction `%LOCALAPPDATA%\acpp\<hash>` → `OUT_DIR`, because MSBuild ignores long-path support and deep checkouts exceed `MAX_PATH`.
- **Compiler cache.** `sccache` or `ccache` is used for C and C++ when found (`AUDIOCPP_NO_CCACHE=1` to disable). ggml's own `GGML_CCACHE` hook stays off: it also wraps the resource compiler, which sccache cannot run.
- A build tree configured with another generator, or at another path, is wiped and configured again.

## Environment variables

| Variable | Effect |
|---|---|
| `AUDIOCPP_DIR` | Link the prebuilt library in this directory and skip the build. It must hold `audiocpp.dll` + `audiocpp.lib`, `libaudiocpp.so` or `libaudiocpp.dylib` (for example `target/release/build/audiocpp-sys-*/out/build/bin`). Features are then inert: the prebuilt already decided its families. |
| `AUDIOCPP_SOURCE_DIR` | Build this audio.cpp tree instead of the submodule (a lane worktree, a local checkout with a fix under test). |
| `AUDIOCPP_MODELS` | Extra model targets, comma separated (aliases work too). |
| `AUDIOCPP_MODEL_SET` | Force `full`, `core` or `custom`. |
| `AUDIOCPP_CMAKE_ARGS` | Extra configure arguments, applied last so they win (`-DGGML_AVX512=ON`, …). Double quotes group a value with spaces. |
| `AUDIOCPP_CMAKE_PROFILE` | CMake build type (default `Release`). |
| `AUDIOCPP_CUDA_ARCHITECTURES` | `CMAKE_CUDA_ARCHITECTURES` for `cuda` (default `native`; set an explicit list such as `75;86;89` for a release). |
| `AUDIOCPP_NO_NINJA`, `AUDIOCPP_NO_CCACHE` | Opt out of Ninja / the compiler cache. |
| `CMAKE_GENERATOR` | When set, the build uses it and skips its own Ninja setup. |

## Build time and size

The first build compiles audio.cpp's framework (shared by every family) plus each selected family. On a recent laptop with a couple of small families that is a few minutes; every family adds to it, and `all-models` takes a long time and yields a large library. Later builds are incremental; with sccache, a clean rebuild is much faster. Point several apps at one prebuilt with `AUDIOCPP_DIR` to build it once.

## Shipping

The app must ship the library next to its executable. `audiocpp-sys` declares `links = "audiocpp"`, so a dependent package's build script reads the folder from `DEP_AUDIOCPP_LIB_DIR`:

```rust
// build.rs of the app (or of a crate between the app and audiocpp)
fn main() {
    if let Ok(dir) = std::env::var("DEP_AUDIOCPP_LIB_DIR") {
        println!("cargo:rustc-env=AUDIOCPP_LIB_DIR={dir}"); // e.g. for a bundling step
    }
}
```

- **Tauri:** copy `audiocpp.dll` / `libaudiocpp.so` / `libaudiocpp.dylib` into the bundle next to the executable (a `bundle.resources` entry fed by a build step, or the platform's frameworks / libs list).
- **Linux:** link the app with an rpath of `$ORIGIN` (`-C link-arg=-Wl,-rpath,$ORIGIN`) and ship `libaudiocpp.so*` beside it.
- **macOS:** `@executable_path` (or `@loader_path` inside a bundle).
- **Windows:** the DLL beside the `.exe`, plus the Visual C++ runtime (and `vcomp140.dll` with the `openmp` feature).
- **`assets/` beside the library.** `DEP_AUDIOCPP_LIB_DIR` also holds `assets/framework/models/silero_vad/` (1.2 MB): the default VAD that ASR families chunk long audio with. audio.cpp finds it beside the library, or up to three directories above it, so copy the `assets/` folder along with the library. An app without ASR can leave it out.
- Model specs are compiled into the library, so no `model_specs/` directory is needed. eSpeak data is only needed with `gpl-espeak`, for the families that use it.

## Troubleshooting

| Symptom | Cause |
|---|---|
| `no audio.cpp sources at …` | The submodule is not checked out: `git submodule update --init audiocpp-sys/audio.cpp`. |
| `STATUS_DLL_NOT_FOUND` (0xc0000135) when running a binary directly | `audiocpp.dll` is not next to it: copy it from `DEP_AUDIOCPP_LIB_DIR` (cargo run/test find it on their own). |
| `Status::UnsupportedFamily` on load | The model's family is not compiled in: enable its `model-*` feature. |
| `Status::AbiMismatch` | The loaded library is not the one these bindings were built for (an old `AUDIOCPP_DIR`, a stray DLL on `PATH`). |
| A build tree that keeps failing after a Visual Studio update | Build trees cache the compiler path: `cargo clean -p audiocpp-sys`. |
