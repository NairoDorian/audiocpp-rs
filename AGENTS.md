# audiocpp-rs — agent conventions

**Read first:** [`README.md`](README.md), then [`docs/api.md`](docs/api.md) (the C contract → Rust rules) and [`docs/building.md`](docs/building.md).

## What this repository is

Two crates over audio.cpp's C ABI and nothing else:
- `audiocpp-sys`: hand-written FFI for `include/audiocpp.h` + `build.rs`, which builds the pinned audio.cpp submodule (`audiocpp-sys/audio.cpp`) into one shared library;
- `audiocpp`: the safe API.

`xtask` holds the repository checks. The consumer is ZER0 (to be renamed FreeSpeech), which also runs `transcribe-cpp`; that is why coexistence matters (`docs/coexistence.md`).

## Never

- **Never edit files under `audiocpp-sys/audio.cpp/`.** It is upstream's tree at a pinned commit. A change the engine needs is a pull request to audio.cpp (`docs/upstream.md`); then the pin moves (`docs/updating-audio-cpp.md`).
- Never call audio.cpp's C++ internals, or include any header but `audiocpp.h`. The C ABI is the only contract.
- Never hand-edit the blocks between `# BEGIN GENERATED MODEL FEATURES` and `# END …` in either `Cargo.toml`, nor `docs/model-features.md`: run `cargo xtask features`.
- Never run `cargo xtask check-abi --update` without reading the header diff and updating `audiocpp-sys/src/lib.rs` first. That file is the reviewed binding; the lock only records that the review happened.
- Never make the library static by default: two static ggml copies in one binary collide (`docs/coexistence.md`).
- Never let a panic or an exception cross the FFI boundary: the log trampoline catches panics; C++ never throws through `audiocpp.h`.
- Commit and push only when the author asks.

## Commands

```bash
uv run scripts/fetch_test_models.py                         # test models into models/ (sha256-pinned)
cargo test --release --features model-tiny_voca,model-sense_asr
cargo xtask check-abi                                       # bindings == pinned header
cargo xtask features --check                                # features == pinned model targets
cargo xtask status --fetch                                  # the pin vs audio.cpp's release / main
cargo clippy --release --features model-tiny_voca,model-sense_asr --all-targets
```

- Python is run through `uv run`.
- Build in release: audio.cpp is always compiled in Release, but the Rust side of the tests is quicker too.
- The first build compiles audio.cpp (minutes); `AUDIOCPP_DIR` can point at an existing build to skip it.
- Model tests print `SKIPPED` when a model file is missing. A skip is not a pass: CI sets `AUDIOCPP_REQUIRE_MODELS=1`.
- **Timings need an idle machine.** Before quoting an RTF, check that no other build or benchmark is running. Other agent sessions share this workstation: on 2026-11-25 one ran a transcribe.cpp benchmark concurrently, and the same synthesis varied 20×. Tests prove behaviour, not speed.

## Adding to the safe API

| To add | Do |
|---|---|
| A new C function (after a pin move) | Declare it in `audiocpp-sys/src/lib.rs` in header order, wrap it in the right module of `audiocpp/src/`, add a test, `cargo xtask check-abi --update`, and a CHANGELOG line. |
| A wrapper method | Read `audiocpp_last_error` through `error::check` / `Error::from_call` right after the failing call; copy strings out (`ffi::string`); borrow only audio, tied to `&self`. |
| A thread-safety claim | Only what audio.cpp's `docs/c_api.md` guarantees: handles `Send`, never `Sync`. |

## Style

Rust 2021, `cargo fmt`, max line width 120 (`rustfmt.toml`). Comments explain constraints that the code cannot show (the C contract, a measured failure); not what the next line does.
