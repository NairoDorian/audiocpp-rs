# Models

## Choosing families

Every audio.cpp model target is a Cargo feature, `model-<target>` ([`model-features.md`](model-features.md) lists them, generated from the pinned tree). A family's code is only compiled when its feature is on, so an app ships what it uses:

```toml
audiocpp = { git = "https://github.com/NairoDorian/audiocpp-rs", features = ["model-kokoro_tts", "model-kitten_tts2"] }
```

`Registry::new()?.families()` lists what a build contains. Loading a file of a family that is not compiled in fails with `Status::UnsupportedFamily`.

What each family does, its languages, voices, options and measured quality are in audio.cpp's own documentation, in the submodule:

- `audiocpp-sys/audio.cpp/docs/models/<family>.md` and `docs/community_models/`: the model cards;
- `audiocpp-sys/audio.cpp/model_specs/<family>.json`: packages, files, options and capabilities;
- `audiocpp-sys/audio.cpp/README.md`: the table of every family by task.

At runtime, `model.supports(&Task::Tts, Mode::Streaming)`, `model.languages()`, `model.supports_speaker_reference()` and `model.options(OptionScope::Request)` answer the same questions without reading any file.

## Getting the files

The library **does not download**. A model spec names where audio.cpp's packages live (`package_defaults.download` and `packages[].files` in `model_specs/<family>.json`; most are on Hugging Face, many under `audio-cpp/audio.cpp-gguf`). A typical package is a single GGUF with its config embedded; some families need a directory.

For an app:
1. Download the package files yourself (the app owns the network, and its progress UI).
2. **Pin each file by sha256.** Many specs point at a branch (`revision: main`), whose files can change under the same name. Read the hash from the Hugging Face API (`/api/models/<repo>/tree/<revision>`, field `lfs.oid`) and verify after download. `scripts/fetch_test_models.py` shows the pattern for the test models.
3. Load the GGUF (or the package directory) with `Model::load`. Pass `ModelConfig::family("…")` when the file does not identify its family on its own.

## Files a family needs besides its GGUF

Some families load a second model at run time.
- The common case is ASR chunking, which uses a VAD: Silero, by default from a path relative to the working directory.
- Ship audio.cpp's `assets/framework/models/silero_vad/` folder (one 1.2 MB file) with the app, and pass its absolute path as the family's `…vad_model_path` session option.
- Or set `audio_chunk_mode` to `fixed` / `none`.
- [`api.md`](api.md#things-worth-knowing) has the details, and the 16 kHz requirement that comes with VAD chunking.

## Licences

Two kinds of licence apply, and both matter for an app you distribute:

- **The code.** audio.cpp is Apache-2.0. eSpeak-ng, which some TTS families use to turn text into phonemes, is GPL-3.0-or-later. It stays out of the build unless you enable `gpl-espeak`. Without it, a family that needs it either asks for an external eSpeak library at runtime (an option such as `espeak_library_path`) or cannot phonemize. Families with their own text frontend (tiny_voca is one) need neither.
- **The weights.** Every model has its own licence, and some are non-commercial or gated. Read the model card before shipping a model.

## Test models of this repository

| File (under `models/`) | Feature | Use |
|---|---|---|
| `tiny-voca-GGUF/tiny-voca-orig.gguf` (24 MB) | `model-tiny_voca` | English TTS, 4.5M parameters, own phonemizer, voices `M2` and `F2`, 24 kHz |
| `SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf` (254 MB) | `model-sense_asr` | multilingual ASR: the listener in the round-trip test |

`uv run scripts/fetch_test_models.py` fetches both, pinned by revision and sha256.
