# Changelog

## 0.1.0 — 2026-11-25 (unreleased)

First version: Rust bindings to audio.cpp's C ABI, for ZER0 / FreeSpeech.

### What exists
- **`audiocpp-sys`**: hand-written FFI for all 77 functions of `audiocpp.h` (ABI 0.4.0).
  - `cargo xtask check-abi` proves it against the pinned header: names, parameter counts, the ABI version, and the header hash in `abi.lock`.
  - `build.rs` builds the pinned audio.cpp **v0.9.6** (submodule `audiocpp-sys/audio.cpp`, release lane) into one shared library with only the enabled families.
  - Features: 145 generated `model-<target>` features, `all-models`, `vulkan` / `cuda` / `hip` / `metal`, `openmp` (default), `native`, `gpl-espeak`.
  - Prebuilt libraries through `AUDIOCPP_DIR`, other trees through `AUDIOCPP_SOURCE_DIR`, extra CMake arguments through `AUDIOCPP_CMAKE_ARGS`.
- **`audiocpp`**: the safe API.
  - Handle types: `Registry`, `Model`, `Session`, `Request`, `TaskResult` / `ResultData`, `Stream` / `Event`.
  - Option introspection (`Model::options`), tasks and modes, backends, artifacts, logging (with `log` support).
  - Handles are `Send` and not `Sync`, audio is borrowed, errors carry audio.cpp's own message, and the ABI is checked once per process.
- **Examples**: `info`, `tts`, `asr`, `roundtrip`.
- **Tests**:
  - the contract without models;
  - tiny_voca synthesis, including session reuse and speaking rate;
  - a TTS → ASR round trip inside audio.cpp (tiny_voca → SenseVoice), with and without VAD chunking.
- **Docs**: README, `docs/` (api, building, coexistence, models, model-features, updating-audio-cpp, upstream), AGENTS.md.
- **CI**: format, ABI and feature checks on every push; a Windows / Linux / macOS build-and-test matrix on demand.

### Found while building it
- **Portable CPU kernels.** audio.cpp defaults `ENGINE_ENABLE_NATIVE_CPU=ON`, which ties the library to the build machine's CPU (SIGILL elsewhere). The crate defaults it OFF (ggml's AVX2-class floor); the `native` feature opts in.
- **MSVC flags.**
  - `cmake-rs` replaces `CMAKE_CXX_FLAGS` with the cc crate's flags, which drops `/EHsc`: C++ exceptions would unwind without running destructors.
  - The build now defines CMake's own MSVC defaults (`/DWIN32 /D_WINDOWS /EHsc`, Release `/O2 /Ob2 /DNDEBUG`).
  - transcribe.cpp protects itself against this in its CMakeLists; audio.cpp does not.
- **The sccache hook.** ggml's `GGML_CCACHE` hook wraps the Windows resource compiler in sccache, which cannot run it. The hook is off here; sccache is applied to C/C++ through compiler launchers.
- **audio.cpp behaviours that an embedding app must know** (`docs/api.md`, and PR candidates in `docs/upstream.md`):
  - session option names carry the family prefix, and an unknown session key is silently ignored;
  - several ASR families default their VAD model path to a path relative to the working directory;
  - VAD chunking needs 16 kHz input.

### Verified
Windows 11, i9-13900H, MSVC 14.52, CPU, audio.cpp v0.9.6, features `model-tiny_voca,model-sense_asr`:
`AUDIOCPP_REQUIRE_MODELS=1 cargo test --release --features model-tiny_voca,model-sense_asr` (results below).

**12 passed, 0 failed, 0 skipped** (models were required, so a missing model would have failed):

| Test binary | Tests | Result |
|---|---|---|
| `api` | ABI vs bindings, task vocabulary, compiled families (`marblenet_vad, sense_asr, silero_vad, tiny_voca`), missing-file error, setter validation, backend and option defaults | 7 passed |
| `tiny_voca` | speaks English at 24 kHz (> 1 s, finite, non-silent); a second run on the same session gives the same length; a higher speaking rate gives shorter audio | 2 passed |
| `roundtrip` | "The quick brown fox jumps over the lazy dog." → tiny_voca → SenseVoice → **heard exactly** (WER 0/9), with `audio_chunk_mode=none`, and again with VAD chunking from an explicit Silero path at 16 kHz | 2 passed |
| doc-test | the crate-level example compiles | 1 passed |

`cargo xtask check-abi` and `cargo xtask features --check` pass at v0.9.6. A clean build of audio.cpp with these two families took 6 min 49 s (3 min 23 s for a rebuild with sccache). `audiocpp.dll` is 4.9 MB.

Timings from this session are **not benchmarks**. Other agent sessions were running a transcribe.cpp benchmark and several builds on the same machine (CPU at 100 %). Identical tiny_voca syntheses took between 2 s and 42 s, for both the native and the portable library, and the round-trip tests took 730 s instead of seconds. Benchmark on an idle machine: ≥ 3 runs, run 1 = warm-up, the mean of runs 2..N.

### Not done yet
- Linux, macOS, Android and GPU backends have not been run here: the on-demand CI matrix is the next proof.
- No streaming test yet: a streaming TTS family and a model to test it are still to be chosen.
