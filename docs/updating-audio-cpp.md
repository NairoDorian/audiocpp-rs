# Moving the audio.cpp pin

audio.cpp is a git submodule at `audiocpp-sys/audio.cpp`. **The commit it points at is the pin**: what every build of this crate compiles. It follows one of two lanes:

| Lane | Target | When |
|---|---|---|
| **release** (default) | the latest `vX.Y.Z` tag | normal updates; each release is a tested set of models |
| **main** | `origin/main` | a model or fix that is not released yet (audio.cpp moves fast: often 20+ commits between releases) |

## Status

```bash
cargo xtask status --fetch
#  audio.cpp pin   04a37f2e  v0.9.6
#  checkout        04a37f2e  = pin
#  release lane    04a37f2e  v0.9.6  same as the pin
#  main lane       4363a483  origin/main  +23 new / 0 only in the pin
```

## Moving it

```bash
git -C audiocpp-sys/audio.cpp fetch origin --tags
git -C audiocpp-sys/audio.cpp log --oneline <old>..<new>    # read what comes in
git -C audiocpp-sys/audio.cpp checkout --detach v0.9.7      # or a main commit

cargo xtask check-abi            # did audiocpp.h change? (fails until reviewed)
cargo xtask features             # regenerate the model-* features (new/removed families)
cargo test --release --features model-tiny_voca,model-sense_asr
```

Then, depending on what the checks say:

1. **`check-abi` fails.** The C header changed. Read its diff (`git -C audiocpp-sys/audio.cpp diff <old> <new> -- include/audiocpp.h`), update `audiocpp-sys/src/lib.rs` (and the constants `AUDIOCPP_ABI_VERSION_*`), expose anything new in the safe crate, then `cargo xtask check-abi --update`. A new **minor** means new functions; a new **major** means a breaking change, so bump this crate's major too.
2. **`features` changed `Cargo.toml`.** Families were added or removed. A removed family is a breaking change for anyone who enabled its feature: note it in the CHANGELOG.
3. **Families an app ships.** Run its own checks too: synthesize its test sentences and compare duration, RTF and a round-trip WER against the old pin. audio.cpp releases change model behaviour as well as code.
4. Commit the submodule move, `Cargo.toml`, `docs/model-features.md`, `abi.lock` and the CHANGELOG entry together: `deps(audio.cpp): pin v0.9.7`.

## Testing a lane without moving the pin

Build against another checkout through `AUDIOCPP_SOURCE_DIR`, with a separate target directory so the two builds do not overwrite each other:

```bash
git -C audiocpp-sys/audio.cpp worktree add --detach ../../../audio.cpp-main origin/main   # beside this repo
AUDIOCPP_SOURCE_DIR=$PWD/../audio.cpp-main CARGO_TARGET_DIR=target-main \
    cargo test --release --features model-tiny_voca,model-sense_asr
```

## When a check fails on the new pin

First run the same input through audio.cpp's own CLI, built from that tree. If the CLI fails too, the regression is upstream: report it (or send the fix, [`upstream.md`](upstream.md)) and stay on the old pin. If only the binding fails, the bug is here.
