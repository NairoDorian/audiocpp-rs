# audiocpp-rs

Rust bindings to [**audio.cpp**](https://github.com/0xShug0/audio.cpp): local text-to-speech, voice cloning, speech recognition, VAD, diarization and more than a hundred other audio model families on ggml, **in-process**, with the families chosen at build time.

| Crate | What it is |
|---|---|
| [`audiocpp`](audiocpp/) | The safe API: `Registry`, `Model`, `Session`, `Request`, `TaskResult`, `Stream`. |
| [`audiocpp-sys`](audiocpp-sys/) | The raw C ABI (`audiocpp.h`, 77 functions) and the build: audio.cpp from a pinned release, compiled into one shared library with only the families you ask for. |

Written for [ZER0 / FreeSpeech](#who-it-is-for), next to [`transcribe-cpp`](https://github.com/NairoDorian/transcribe.cpp): transcribe.cpp does speech-to-text, audio.cpp gives the app a voice. Both live in one process (see [`docs/coexistence.md`](docs/coexistence.md)).

## Status

| | |
|---|---|
| audio.cpp pin | **audio.cpp's latest `main` commit**, followed daily by a workflow that builds and tests each move ([`docs/updating-audio-cpp.md`](docs/updating-audio-cpp.md)); C ABI 0.4.0 |
| Bindings | the whole C ABI: offline and streaming sessions, requests (text, audio, voices, style, options, artifacts), results (audio, text, segments, words, speaker turns, named audio, artifacts, custom schema), model introspection, logging |
| Verified on | Windows 11, MSVC 14.52, CPU (see the CHANGELOG for the test run) |
| Not yet verified | Linux, macOS, Android, GPU backends (they build through the same features; nobody has run them here yet) |

## Quick start

```toml
[dependencies]
audiocpp = { git = "https://github.com/NairoDorian/audiocpp-rs", features = ["model-kokoro_tts"] }
```

```rust
use audiocpp::{Backend, Mode, Model, ModelConfig, Options, Registry, Request, ResultData, Task};

fn main() -> audiocpp::Result<()> {
    let registry = Registry::new()?;
    let model = Model::load(&registry, "models/kokoro.gguf", &ModelConfig::default(), &Options::new())?;
    let mut tts = model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new())?;

    let mut request = Request::new();
    request.text("Hello from FreeSpeech.")?.voice_id("af_heart")?;
    let result = tts.run(&request)?;
    let audio = result.audio().expect("TTS returns audio"); // borrowed &[f32], no copy
    println!("{:.2} s at {} Hz", audio.duration().as_secs_f32(), audio.sample_rate);
    Ok(())
}
```

Keep the `Session` and reuse it: audio.cpp keeps its graphs and caches warm between runs.

From this repository (Windows, Linux or macOS):

```bash
git clone --recursive https://github.com/NairoDorian/audiocpp-rs && cd audiocpp-rs
uv run scripts/fetch_test_models.py                      # tiny_voca (24 MB) + SenseVoice (254 MB), sha256-pinned
cargo test --release --features model-tiny_voca,model-sense_asr
cargo run --release --example tts --features model-tiny_voca -- \
    models/tiny-voca-GGUF/tiny-voca-orig.gguf "Hello from Rust." hello.wav M2
cargo run --release --example roundtrip --features model-tiny_voca,model-sense_asr -- \
    models/tiny-voca-GGUF/tiny-voca-orig.gguf \
    models/SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf \
    "The quick brown fox jumps over the lazy dog."
```

The first build compiles audio.cpp's runtime and the selected families (several minutes); later builds are incremental.

## Choosing what goes into the library

**Model families** are Cargo features, one per audio.cpp model target: `model-kokoro_tts`, `model-kitten_tts2`, `model-qwen3_tts`, `model-chatterbox`, `model-parakeet_tdt`, … The full list (147 when this was written) is generated from the pinned tree in [`docs/model-features.md`](docs/model-features.md). No model feature builds the runtime alone; `all-models` builds every family (a long build, a big library).

| Feature | Effect |
|---|---|
| `model-<target>` | compile that family in (`AUDIOCPP_MODELS`) |
| `all-models` | every family |
| `vulkan`, `cuda`, `hip`, `metal` | GPU backends (`metal` only acts on Apple targets; it is a default feature) |
| `openmp` (default) | OpenMP for audio.cpp's host code, as upstream builds it. Keep it on with MSVC: audio.cpp's own ggml thread pool lacks an MSVC barrier fix ([`docs/upstream.md`](docs/upstream.md) #9). No effect on macOS, where Apple's clang has no OpenMP |
| `native` | tune the CPU kernels to the build machine: **never for a build you ship** (older CPUs crash) |
| `gpl-espeak` | statically link eSpeak-ng (GPL-3.0-or-later) for families that phonemize through it: changes your licence |
| `log` | `audiocpp::logging::route_to_log()` sends audio.cpp's log lines to the `log` crate |

Environment variables for the build (`AUDIOCPP_DIR` for a prebuilt library, `AUDIOCPP_MODELS`, `AUDIOCPP_CMAKE_ARGS`, …) are in [`docs/building.md`](docs/building.md).

## Model files

The library loads audio.cpp's GGUF files (or model packages). Each family's file layout, voices and options are in audio.cpp's model cards (`audiocpp-sys/audio.cpp/docs/models/<family>.md`) and specs (`model_specs/<family>.json`). The crate **does not download** anything: the app owns downloads, and should pin each file by sha256 (many specs track a branch, not a revision). See [`docs/models.md`](docs/models.md).

A model tells you what it accepts at runtime, so an app can build its settings screens without hardcoding a family:

```rust
for option in model.options(audiocpp::OptionScope::Request) {
    println!("{} = {} ({})", option.name, option.default_value, option.description);
}
```

## Shipping it in an app

- The library is **shared**: `audiocpp.dll` (+ `audiocpp.lib`), `libaudiocpp.so`, `libaudiocpp.dylib`. Your build script finds it in `DEP_AUDIOCPP_LIB_DIR` (set by `audiocpp-sys`'s `links = "audiocpp"`) and copies it next to the executable or into the app bundle. `cargo run` and `cargo test` find it on their own.
- Windows needs the Visual C++ runtime (`vcruntime140.dll`, plus `vcomp140.dll` with `openmp`).
- On Linux and macOS, give your executable an rpath of `$ORIGIN` / `@executable_path` and ship the library beside it.
- Details, Tauri included: [`docs/building.md`](docs/building.md#shipping).

## Who it is for

[ZER0](https://github.com/NairoDorian) (to be renamed **FreeSpeech**) is a desktop dictation app that already runs speech-to-text in-process through `transcribe-cpp`. This crate gives it audio.cpp's voices, without a server process and without Python. The Android app (FreeSpeech Android) is the second consumer; Android builds are not done yet ([`docs/upstream.md`](docs/upstream.md)).

## Documentation

| Read | For |
|---|---|
| [`docs/api.md`](docs/api.md) | The API, and how the C contract maps to Rust: ownership, threads, borrowed audio, errors, logging, streaming |
| [`docs/building.md`](docs/building.md) | The build script, every feature and variable, Windows specifics, shipping |
| [`docs/coexistence.md`](docs/coexistence.md) | Running next to transcribe-cpp, llama.cpp and other ggml users |
| [`docs/models.md`](docs/models.md) · [`docs/model-features.md`](docs/model-features.md) | Choosing families, getting files, licences |
| [`docs/updating-audio-cpp.md`](docs/updating-audio-cpp.md) | Moving the audio.cpp pin (release or main lane) |
| [`docs/upstream.md`](docs/upstream.md) | What to send to audio.cpp as pull requests, and why |
| [`AGENTS.md`](AGENTS.md) | Conventions for coding agents working here |

## Licence

The crates' own code is MIT ([`LICENSE`](LICENSE)). audio.cpp (the `audiocpp-sys/audio.cpp` submodule, compiled into the library) is Apache-2.0 with its own third-party notices. Model weights have their own licences: read each model card before shipping one.
