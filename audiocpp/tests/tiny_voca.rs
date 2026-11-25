//! Text to speech with tiny_voca (24 MB, its own phonemizer: no eSpeak).
#![cfg(feature = "model-tiny_voca")]

mod common;

use audiocpp::{Backend, Mode, Model, ModelConfig, OptionScope, Options, Registry, Request, ResultData, Task};

#[test]
fn speaks_english_and_reuses_its_session() {
    let Some(path) = common::model(common::TINY_VOCA) else { return };
    let registry = Registry::new().unwrap();
    let model = Model::load(&registry, &path, &ModelConfig::family("tiny_voca"), &Options::new()).unwrap();
    assert_eq!(model.family(), "tiny_voca");
    assert!(model.supports(&Task::Tts, Mode::Offline));
    assert!(!model.supports(&Task::Asr, Mode::Offline));

    let names: Vec<String> = model.options(OptionScope::Request).into_iter().map(|o| o.name).collect();
    assert!(names.iter().any(|n| n == "speed"), "request options: {names:?}");

    let mut session = model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new()).unwrap();
    let mut request = Request::new();
    request.text("Hello from Rust. This is audio dot cpp speaking.").unwrap().voice_id("M2").unwrap().seed(0).unwrap();

    let first = session.run(&request).unwrap();
    let audio = first.audio().expect("a TTS result carries audio");
    assert_eq!(audio.sample_rate, 24_000);
    assert_eq!(audio.channels, 1);
    assert!(audio.duration().as_secs_f32() > 1.0, "{:?}", audio.duration());
    assert!(audio.samples.iter().all(|s| s.is_finite()));
    assert!(common::rms(audio.samples) > 1e-3, "silent output");

    // The same request again on the same session: graphs and caches are reused,
    // and every input must be uploaded again (a bug class seen in other ggml
    // engines). Same seed, same text: the same length.
    let second = session.run(&request).unwrap();
    assert_eq!(second.audio().unwrap().samples.len(), audio.samples.len());
}

#[test]
fn speaking_rate_changes_the_duration() {
    let Some(path) = common::model(common::TINY_VOCA) else { return };
    let registry = Registry::new().unwrap();
    let model = Model::load(&registry, &path, &ModelConfig::family("tiny_voca"), &Options::new()).unwrap();
    let mut session = model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new()).unwrap();
    let mut length = |rate: f32| {
        let mut request = Request::new();
        request.text("The rate of speech is a request option.").unwrap().seed(0).unwrap().speaking_rate(rate).unwrap();
        session.run(&request).unwrap().audio().unwrap().frames()
    };
    let normal = length(1.0);
    let fast = length(1.5);
    assert!(fast < normal, "faster speech is shorter: {fast} vs {normal} frames");
}
