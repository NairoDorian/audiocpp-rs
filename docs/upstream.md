# Upstream: what to send to audio.cpp

This crate changes nothing inside audio.cpp. Whatever the binding needs from the engine goes upstream as a pull request to [0xShug0/audio.cpp](https://github.com/0xShug0/audio.cpp), and comes back through a pin move. In order of value:

| # | Pull request | Why |
|---|---|---|
| 1 | **Install rules and a link manifest for the C API** (`install(TARGETS audiocpp …)`, headers, plus an `audiocpp-link.json` with the libraries a static consumer must link) | audio.cpp has no install rules today, so `build.rs` finds the library in the build tree. An install tree makes prebuilt artifacts and the static library (`AUDIOCPP_BUILD_C_API_STATIC`) usable from other build systems. transcribe.cpp's `transcribe-link.json` is the model. |
| 2 | **Rust bindings in-tree** (`bindings/rust/`: this repository's two crates) | One more consumer of the C API keeps it honest, upstream CI would build it, and other Rust apps get audio.cpp for free. Offer it once the API has settled here. |
| 3 | **An Android (NDK) build lane** | The FreeSpeech Android app needs `libaudiocpp.so` for arm64: Bionic, `c++_shared`, 16 KB page alignment. Nothing in audio.cpp documents an Android build yet; expect small fixes. |
| 4 | **No working-directory default for bundled assets** | Several ASR families default `vad_model_path` to `assets/framework/models/silero_vad`, relative to the process's working directory, so an embedded library fails on long audio unless the app sets the option. Better: resolve framework assets next to the library, or embed the 1.2 MB Silero weights in it (transcribe.cpp embeds its Silero weights). |
| 5 | **Resample for the VAD chunker** | With `audio_chunk_mode=vad` the request audio goes to Silero as given, and Silero only takes 16 kHz, while the ASR model itself resamples. The chunker could resample a copy for the VAD. |
| 6 | **Report unknown session options** | An unknown session option key is silently ignored (a bare `vad_model_path` instead of `sense_asr.vad_model_path` does nothing). Rejecting it, or accepting the bare name the spec declares, would turn a silent misconfiguration into an error. |
| 7 | **UTF-8 paths on Windows inside the library** | Today a non-ASCII model path works only when the host executable runs with the UTF-8 code page; converting to wide paths inside `audiocpp_model_load` would remove that requirement. |
| 8 | Anything the binding's tests find | A bug found here is fixed there, with a test, then the pin moves. |

Before sending one: check audio.cpp's open PRs and its `CONTRIBUTING.md`, keep each PR to one subject, and include the test that proves it (audio.cpp tests its C API with `tests/capi/*.c`).

## The sibling engine

transcribe.cpp (the author's fork, used by ZER0 for speech-to-text) follows the same policy from the other side. Two items found while building this crate belong there:

- ggml's `GGML_CCACHE` hook wraps the Windows resource compiler in sccache, which cannot run it: transcribe.cpp's version resource fails to build whenever sccache is on `PATH`. The fix is to set the compiler launchers for C/C++ only (what `audiocpp-sys/build.rs` does for audio.cpp).
- Official transcribe.cpp has a Windows DLL-lookup fix for Cargo's new build-dir layout (#264) that the fork has not merged yet; `audiocpp-sys` already uses its approach.
