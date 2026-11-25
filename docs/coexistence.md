# Living next to other ggml libraries

A speech app rarely loads one engine. ZER0 / FreeSpeech runs speech-to-text through `transcribe-cpp`, text-to-speech through this crate, and may run an LLM through llama.cpp. Each of them is built on its **own copy of ggml**, at its own version, with its own patches. They can share a process safely if one rule holds.

## The rule: at most one static ggml

| Library | How this crate ships it | Exports |
|---|---|---|
| audio.cpp (`audiocpp-sys`) | **shared**: `audiocpp.dll` / `libaudiocpp.so` / `.dylib` | only `audiocpp_*`. ggml and the other static libraries inside are hidden (a linker version script plus `--exclude-libs,ALL` on ELF, an export list on Mach-O; on Windows a DLL exports only what is marked, and audio.cpp marks only its API) |
| transcribe.cpp (`transcribe-cpp`) | static by default (ggml linked into your executable), or `shared` | static: its ggml symbols are part of your executable; shared: hidden like audio.cpp's |

- **audio.cpp shared + transcribe.cpp static** (the default combination): safe. audiocpp's ggml is private to its library, so its internal calls never reach the executable's ggml, and the reverse holds too.
- **Both shared:** safe, the same way.
- **Two static ggml copies** (for example transcribe.cpp static and llama.cpp static in one executable): the linker sees `ggml_*` twice, from two different versions. That is a duplicate-symbol error, or worse, one copy silently calling the other's functions. **Keep at most one static ggml user per binary**; make the others shared. This crate never links audio.cpp statically for that reason.

The upstream test `audiocpp_c_api_exports` asserts audio.cpp's export surface on every platform, so this property does not erode silently.

## What two ggml copies cost

- **Size:** each copy carries its own CPU kernels and, per enabled GPU backend, its own GPU kernels. CPU builds stay small (a few MB); CUDA kernels make each copy much larger.
- **Initialisation:** each library initialises its backends and devices on its own.
- **Memory:** no weights are shared between engines anyway. Budget VRAM per engine, because neither sees the other's allocations.

Merging them into one ggml would need both engines to build against the same ggml version and patch set. That is real work, and it would need upstream support; not planned until package size makes it necessary.

## Running STT and TTS at the same time

A voice app listens (VAD, streaming STT) while it speaks (TTS), so both engines compute at once:

- **Threads.** Give each session a share of the cores (`Backend::cpu().threads(n)` here, `n_threads` in transcribe-cpp) instead of letting both default to all of them.
- **Devices.** Choose the GPU explicitly for each engine (`Backend { kind, device, .. }`); "best" in each engine may pick differently.
- **Sessions are per thread.** One audio.cpp `Session` per concurrent task; the same holds for transcribe-cpp sessions.
- **Logging.** Each engine has its own log sink: route both to the same place (`audiocpp::logging::route_to_log()` and transcribe-cpp's logging) so a log tells which engine spoke.
