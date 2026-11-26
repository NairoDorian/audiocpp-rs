//! Speak, then listen, inside audio.cpp: tiny_voca (TTS) into SenseVoice (ASR).
//! Proves both directions of the binding on real models with no audio fixture.
#![cfg(all(feature = "model-tiny_voca", feature = "model-sense_asr"))]

mod common;

use std::path::{Path, PathBuf};

use audiocpp::{Backend, Mode, Model, ModelConfig, OptionScope, Options, Registry, Request, ResultData, Session, Task};

const TEXT: &str = "The quick brown fox jumps over the lazy dog.";

/// Silero VAD weights as audio.cpp ships them in its tree.
fn silero_vad_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../audiocpp-sys/audio.cpp/assets/framework/models/silero_vad")
}

fn speak(registry: &Registry, tts_path: &Path) -> (Vec<f32>, u32, u16) {
    let model = Model::load(registry, tts_path, &ModelConfig::default(), &Options::new()).unwrap();
    let mut tts = model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new()).unwrap();
    let mut say = Request::new();
    say.text(TEXT).unwrap().voice_id("M2").unwrap().seed(0).unwrap();
    let spoken = tts.run(&say).unwrap();
    let audio = spoken.audio().unwrap();
    (audio.to_vec(), audio.sample_rate, audio.channels)
}

/// tiny_voca speaks at 24 kHz; audio.cpp converts the input for the 16 kHz model, and its VAD
/// chunking resamples for Silero too (audio.cpp #1404). `chunk_mode` None keeps the default.
fn listen(asr: &mut Session, samples: &[f32], rate: u32, channels: u16, chunk_mode: Option<&str>) -> String {
    let mut request = Request::new();
    request.audio(samples, rate, channels).unwrap();
    if let Some(mode) = chunk_mode {
        request.option("audio_chunk_mode", mode).unwrap();
    }
    asr.run(&request).unwrap().text().expect("an ASR result carries text").text
}

fn assert_words(heard: &str) {
    let (errors, total) = common::word_errors(TEXT, heard);
    println!("said:  {TEXT}\nheard: {heard}\nWER {errors}/{total}");
    assert!(errors <= 2, "round trip lost too many words: {errors}/{total} (heard {heard:?})");
}

#[test]
fn tts_into_asr_keeps_the_words() {
    let (Some(tts_path), Some(asr_path)) = (common::model(common::TINY_VOCA), common::model(common::SENSEVOICE)) else {
        return;
    };
    let registry = Registry::new().unwrap();
    let (samples, rate, channels) = speak(&registry, &tts_path);

    let asr_model = Model::load(&registry, &asr_path, &ModelConfig::default(), &Options::new()).unwrap();
    assert!(asr_model.supports(&Task::Asr, Mode::Offline), "{} does not do ASR", asr_model.family());
    let declared: Vec<String> = asr_model.options(OptionScope::Request).into_iter().map(|o| o.name).collect();
    assert!(declared.iter().any(|n| n == "audio_chunk_mode"), "{declared:?}");

    // "none": one window, no VAD.
    let mut asr = asr_model.session(&Task::Asr, Mode::Offline, &Backend::cpu(), &Options::new()).unwrap();
    assert_words(&listen(&mut asr, &samples, rate, channels, Some("none")));
}

#[test]
fn default_options_chunk_with_the_vad_beside_the_library() {
    let (Some(tts_path), Some(asr_path)) = (common::model(common::TINY_VOCA), common::model(common::SENSEVOICE)) else {
        return;
    };
    let registry = Registry::new().unwrap();
    let (samples, rate, channels) = speak(&registry, &tts_path);

    // No VAD option and no chunk mode: SenseVoice's default ("auto") chunks with the default
    // Silero VAD, which audio.cpp finds beside the library (#1403) when the working directory
    // lacks it. audiocpp-sys stages assets/ beside every copy of the library it makes.
    let asr_model = Model::load(&registry, &asr_path, &ModelConfig::default(), &Options::new()).unwrap();
    let mut asr = asr_model.session(&Task::Asr, Mode::Offline, &Backend::cpu(), &Options::new()).unwrap();
    assert_words(&listen(&mut asr, &samples, rate, channels, None));
}

#[test]
fn vad_chunking_works_with_an_explicit_vad_path() {
    let (Some(tts_path), Some(asr_path)) = (common::model(common::TINY_VOCA), common::model(common::SENSEVOICE)) else {
        return;
    };
    // std::path::absolute, not canonicalize: no \\?\ verbatim prefix on Windows.
    let vad = std::path::absolute(silero_vad_dir()).unwrap();
    assert!(vad.join("silero_vad_16k.safetensors").is_file(), "audio.cpp's Silero VAD assets at {}", vad.display());
    let registry = Registry::new().unwrap();
    let (samples, rate, channels) = speak(&registry, &tts_path);

    let asr_model = Model::load(&registry, &asr_path, &ModelConfig::default(), &Options::new()).unwrap();
    // Session options carry the family prefix ("sense_asr.vad_model_path"), and
    // audio.cpp ignores an unknown session key silently: take the declared name.
    let session_names: Vec<String> = asr_model.options(OptionScope::Session).into_iter().map(|o| o.name).collect();
    let key = session_names.iter().find(|n| n.ends_with("vad_model_path")).expect("declared VAD path option");
    assert!(key.contains('.'), "session option names are family-prefixed: {session_names:?}");
    let session_options = Options::new().with(key.as_str(), vad.display());
    let mut asr = asr_model.session(&Task::Asr, Mode::Offline, &Backend::cpu(), &session_options).unwrap();
    assert_words(&listen(&mut asr, &samples, rate, channels, Some("vad")));
}
