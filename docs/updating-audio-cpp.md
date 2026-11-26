# Following audio.cpp

audio.cpp is a git submodule at `audiocpp-sys/audio.cpp`. **The commit it points at is the pin**: what every build of this crate compiles.

**The pin follows audio.cpp's latest `main` commit.** audio.cpp moves fast (often 20+ commits between releases), and fixes land on `main` first.

`.github/workflows/follow-audio-cpp.yml` moves the pin every day, and on demand from Actions → *follow audio.cpp* → *Run workflow*:
1. `cargo xtask bump` moves the submodule to audio.cpp's `main`, proves the bindings against its C header, and regenerates the `model-*` features.
2. It builds audio.cpp and runs the model tests (Linux).
3. It pushes `deps(audio.cpp): follow main to <commit>`.

If the C header changed, or the build or a test fails, the run stops before the push and fails, which notifies the repository owner. Then follow the steps below by hand.

An app that wants a fixed audio.cpp pins **this** crate by `rev` in its `Cargo.toml`: each commit here pins exactly one audio.cpp commit.

## Status

```bash
cargo xtask status --fetch
#  audio.cpp pin   dd1ad8bd  v0.9.6-41-gdd1ad8bd
#  checkout        dd1ad8bd  = pin
#  release lane    04a37f2e  v0.9.6  +41 new / 0 only in the pin
#  main lane       dd1ad8bd  origin/main  +0 new / 0 only in the pin
```

## Moving it by hand

```bash
cargo xtask bump                 # fetch, check out origin/main, check-abi, features
git -C audiocpp-sys/audio.cpp log --oneline <old>..<new>    # read what came in
cargo test --release --features model-tiny_voca,model-sense_asr
```

To pin something other than `main` (a release tag, or a commit before a regression), check it out yourself and run the checks:

```bash
git -C audiocpp-sys/audio.cpp fetch origin --tags
git -C audiocpp-sys/audio.cpp checkout --detach v0.9.7      # or any commit
cargo xtask check-abi
cargo xtask features
```

The daily workflow moves a hand-set pin back to `main` on its next run. Disable it (Actions → *follow audio.cpp* → *Disable workflow*) for as long as the pin must stay put.

Then, depending on what the checks say:

1. **`check-abi` fails.** The C header changed.
   - Read its diff: `git -C audiocpp-sys/audio.cpp diff <old> <new> -- include/audiocpp.h`.
   - Update `audiocpp-sys/src/lib.rs` (and the constants `AUDIOCPP_ABI_VERSION_*`), expose anything new in the safe crate, then run `cargo xtask check-abi --update`.
   - A new **minor** means new functions; a new **major** means a breaking change, so bump this crate's major too.
2. **`features` changed `Cargo.toml`.** Families were added or removed. A removed family is a breaking change for anyone who enabled its feature: note it in the CHANGELOG.
3. **Families an app ships.** Run its own checks too: synthesize its test sentences and compare duration, RTF and a round-trip WER against the old pin. audio.cpp changes model behaviour as well as code.
4. **Commit together:** the submodule move, the `Cargo.toml` files, `docs/model-features.md`, `abi.lock` and a CHANGELOG entry.

## Testing another tree without moving the pin

Build against another checkout through `AUDIOCPP_SOURCE_DIR`, with a separate target directory so the two builds do not overwrite each other:

```bash
git -C audiocpp-sys/audio.cpp worktree add --detach ../../../audio.cpp-other <commit>   # beside this repo
AUDIOCPP_SOURCE_DIR=$PWD/../audio.cpp-other CARGO_TARGET_DIR=target-other \
    cargo test --release --features model-tiny_voca,model-sense_asr
```

## When a check fails on the new pin

First run the same input through audio.cpp's own CLI, built from that tree. If the CLI fails too, the regression is upstream: report it (or send the fix, [`upstream.md`](upstream.md)) and pin the last good commit until it is fixed. If only the binding fails, the bug is here.
