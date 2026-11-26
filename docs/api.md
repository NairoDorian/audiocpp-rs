# The API, and the C contract behind it

`audiocpp` mirrors audio.cpp's C ABI (`audiocpp-sys/audio.cpp/include/audiocpp.h`, explained in that tree's `docs/c_api.md`) one to one. This page is the map between the two, and the rules the types enforce.

## The objects

```
Registry ──load──► Model ──session(task, mode, backend)──► Session ──run(Request)──► TaskResult
                                                               └──stream(Request?)──► Stream ──push/next_event──► Event
                                                                                         └──finish──► TaskResult
```

| Rust | C | Notes |
|---|---|---|
| `Registry::new()` | `audiocpp_registry_create(NULL, …)` | The loaders compiled in (`families()`). Checks the library's ABI once. |
| `Model::load(&registry, path, &ModelConfig, &Options)` | `audiocpp_model_load` | `ModelConfig::family("…")` = `--family`; the options are load options. |
| `model.session(&Task, Mode, &Backend, &Options)` | `audiocpp_session_create` | The options are session options. `session_with_impl` picks a backend implementation. |
| `Request` | `audiocpp_request_*` | Text, audio, voice id or reference audio, style, options, option arrays, artifacts. Setters chain: `req.text("…")?.voice_id("…")?`. |
| `session.run(&request)` | `audiocpp_session_run` | Prepares implicitly; `prepare()` pays that cost ahead of a latency-sensitive run. |
| `TaskResult` + `ResultData` | `audiocpp_result_*` | `audio()`, `text()`, `segments()`, `words()`, `speaker_turns()`, `named_audio()`, `artifacts()`, `custom_schema()`. |
| `session.stream(Some(&request))` | `audiocpp_stream_start` | Then `push`, `next_event`, `finish`, `reset`; `session.stream_policy()` gives the preferred chunk. |
| `Event` | `audiocpp_event_*` | `is_final()`, `result()` (a `ResultView`: partial text appears as its text), `voice_activity()`. |
| `logging::*` | `audiocpp_log_set` | Process-wide. |

`Task` covers every token of the C vocabulary (`tts`, `asr`, `vad`, `diar`, `sep`, `gen`, `clon`, `vc`, `s2s`, `align`, `vdes`, `spk`, `svc`, `midi`, `wake`, `cls`); `Task::Other` carries one a newer audio.cpp adds. `audiocpp::tasks()` lists what the loaded library accepts, and `Task::from_spec_name` translates a model spec's spelling ("music" → `Generation`).

## Rules the types enforce

**Ownership.** Every type owns exactly one C handle and frees it in `Drop`. audio.cpp's handles keep their parents alive (a session holds its model, a model its registry), so a model can be dropped while its sessions keep running, and nothing has a lifetime parameter except borrowed outputs.

**Threads.** The C API says handles are *not individually thread-safe* and *separate handles may be used concurrently*. So every handle is `Send` (it may move to another thread) and none is `Sync` (no two threads at once). To run two requests in parallel, open two sessions. To open sessions from several threads, open them on one thread and move each to its worker, or wrap the `Model` in a `Mutex`.

**Borrowed audio, copied text.** `TaskResult::audio()` returns `Audio<'_>`, whose `samples: &[f32]` points into the result (no copy, which matters for long TTS output). Everything else (text, segments, artifacts) is copied into owned Rust values. A `ResultView` borrows from its `Event`.

**Errors.** Every fallible call returns `audiocpp::Result<T>`. An `Error` carries the `Status`, the C function that failed (`call()`) and audio.cpp's message (`message()`). That message comes from `audiocpp_last_error()`, which is **per thread and may be cleared by the next call**: the wrapper reads it immediately on the failing thread, so the `Error` value is complete even after it crosses threads. Rust-side validation (a NUL byte in a string, a sample count that is not a multiple of the channels) fails with `Status::InvalidArgument` before C is called.

**ABI.** `Registry::new()` compares, once per process, the loaded library's `audiocpp_abi_version()` with the version these bindings were written for (`AbiVersion::BINDINGS`): another major, or an older minor, fails with `Status::AbiMismatch` instead of crashing on a missing symbol later.

**Logging.** audio.cpp and ggml log to stderr until told otherwise. `logging::set_log_callback(|level, text| …)` routes every line (from any thread) to your closure; a panic inside it is caught, never unwound into C. `logging::route_to_log()` (feature `log`) sends them to the `log` crate under target `audiocpp`; `silence()` drops them; `reset()` restores stderr.

## Things worth knowing

- **Use option names exactly as the model reports them.**
  - Request options are bare (`audio_chunk_mode`, `speed`).
  - Session options carry the family prefix (`sense_asr.vad_model_path`), as `model.options(OptionScope::Session)` lists them, even though the JSON spec writes them bare.
  - audio.cpp **silently ignores an unknown session option**, so a wrong name is not an error, just no effect. Look names up with `model.options(scope)` instead of hardcoding them.
- **ASR chunking uses a VAD, found beside the library.** Several audio.cpp ASR families (sense_asr, qwen3_asr, parakeet_tdt, vibevoice_asr, moonshine_asr, lfm2_audio, …) split long audio with Silero VAD.
  - The default model is `assets/framework/models/silero_vad`. audio.cpp looks for it relative to the working directory, then beside the library and up to three directories above it.
  - `audiocpp-sys` stages that folder (one 1.2 MB file) beside every copy of the library it builds. **Ship `assets/` next to `audiocpp.dll`** (see [building.md](building.md#shipping)), and the default works wherever the app runs.
  - To use another VAD file, set the family's `…vad_model_path` session option. To skip the VAD, set the request option `audio_chunk_mode` to `fixed` (fixed windows) or `none` (short utterances).
  - The request audio can be at any sample rate: the chunker resamples for Silero.
  - `tests/roundtrip.rs` covers the default, an explicit path, and no chunking.
- **Threads and contention.** Small models run many small graphs, and every OpenMP thread waits at a barrier after each one: when another process takes CPU time, one preempted thread stalls them all. On 2026-11-25 the same tiny_voca synthesis took between 2 s and 42 s while another benchmark loaded the machine. Give each engine a thread budget below the core count, and benchmark only on an idle machine.
- **Sample positions** in segments, words and speaker turns are in samples of the request's audio.
- **Seeds.** `request.seed(n)` is the request option `seed`; `-1` asks audio.cpp for a fresh random seed. Without it, families keep their own default.
- **Languages.** `text_with_language` also sets the request option `language`, like `audiocpp_cli --language`. Families that validate their options strictly reject a language they do not declare: use `text` + `text_language` then. `model.languages()` lists what a model declares.
- **Paths on Windows.** audio.cpp takes UTF-8 paths. For a path with non-ASCII characters, the host executable must run with the UTF-8 active code page (an application manifest entry), as audio.cpp's own executables do.
- **Threads per session.** audio.cpp's own default without a config is **one** CPU thread. `Backend::cpu()` uses `default_threads()` (the machine's parallelism, capped at 8). When STT and TTS run at the same time, give each a share of the cores.
- **The raw surface.** `audiocpp::sys` re-exports `audiocpp-sys` for anything the safe API does not cover.
