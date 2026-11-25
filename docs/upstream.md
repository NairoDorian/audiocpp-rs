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
| 8 | **Canary's transcript language** | `src/models/canary_asr/session.cpp` sets the transcript language to the request's *target* language (a caller hint or a translation target), so `audiocpp_result_text`'s language reads as if the model had detected it. It should stay empty when nothing was detected. A one-line fix. |
| 9 | **The MSVC barrier fix in ggml's CPU thread pool** | On MSVC, `ggml_thread_cpu_relax()` compiles to nothing (MSVC defines `_M_X64`, not `__x86_64__`), so barrier waiters spin without `pause` and can starve a worker that has not arrived yet, up to a deadlock under oversubscription. transcribe.cpp carries the fix as `patches/ggml/0001-fix-threadpool-oversubscription.patch` (a `YieldProcessor()` relax plus a bounded spin before `SwitchToThread()`). It belongs in ggml-org/ggml first, then in audio.cpp's vendored ggml. Until then, keep the `openmp` feature on with MSVC: OpenMP's barrier replaces ggml's. |
| 10 | Anything the binding's tests find | A bug found here is fixed there, with a test, then the pin moves. |

### Code written before

speech.cpp v7, the retired attempt to merge transcribe.cpp into an audio.cpp fork, already implemented #4: a resolver for bundled assets (the path as given, then an environment variable, then beside the library, then the source tree) and optional Silero weights compiled into the binary. It also ported families audio.cpp still lacks: plain Whisper (with the whisper.cpp `.bin` loader), Voxtral Mini 3B, Canary-Qwen 2.5B and MedASR, each of which passed its parity gate against transcribe.cpp. That code targets audio.cpp's framework as of September 2026 and leans on helpers audio.cpp does not have (a tokenizer hub, a mel extractor, a run-control type), so each would be a rewrite against today's framework rather than a copy. It lives in the speech.cpp v7 history (tag `legacy/v7-audiocpp-fork`), for as long as that archive is kept.

Before sending one: check audio.cpp's open PRs and its `CONTRIBUTING.md`, keep each PR to one subject, and include the test that proves it (audio.cpp tests its C API with `tests/capi/*.c`).

## The sibling engine

transcribe.cpp (the author's fork, used by ZER0 for speech-to-text) follows the same policy from the other side. Two items found while building this crate belong there:

- ggml's `GGML_CCACHE` hook wraps the Windows resource compiler in sccache, which cannot run it: transcribe.cpp's version resource fails to build whenever sccache is on `PATH`. The fix is to set the compiler launchers for C/C++ only (what `audiocpp-sys/build.rs` does for audio.cpp).
- Official transcribe.cpp has a Windows DLL-lookup fix for Cargo's new build-dir layout (#264) that the fork has not merged yet; `audiocpp-sys` already uses its approach.
