//! Speak, then listen: TTS into ASR, both in audio.cpp, and score the words.
//!
//!   cargo run --release --example roundtrip --features model-tiny_voca,model-sense_asr -- \
//!       models/tiny-voca-GGUF/tiny-voca-orig.gguf \
//!       models/SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf \
//!       "The quick brown fox jumps over the lazy dog." [VOICE]

use audiocpp::{Backend, Mode, Model, ModelConfig, Options, Registry, Request, ResultData, Task};

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn word_errors(reference: &[String], hypothesis: &[String]) -> usize {
    let mut prev: Vec<usize> = (0..=hypothesis.len()).collect();
    for (i, r) in reference.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, h) in hypothesis.iter().enumerate() {
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(r != h)));
        }
        prev = cur;
    }
    prev[hypothesis.len()]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: roundtrip TTS_MODEL ASR_MODEL TEXT [VOICE]");
        std::process::exit(2);
    }
    let registry = Registry::new()?;
    let tts_model = Model::load(&registry, &args[0], &ModelConfig::default(), &Options::new())?;
    let asr_model = Model::load(&registry, &args[1], &ModelConfig::default(), &Options::new())?;
    let mut tts = tts_model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new())?;
    let mut asr = asr_model.session(&Task::Asr, Mode::Offline, &Backend::cpu(), &Options::new())?;

    let mut say = Request::new();
    say.text(&args[2])?;
    if let Some(voice) = args.get(3) {
        say.voice_id(voice)?;
    }
    let spoken = tts.run(&say)?;
    let audio = spoken.audio().ok_or("no audio")?;

    let mut listen = Request::new();
    listen.audio(audio.samples, audio.sample_rate, audio.channels)?;
    // Default options: families that chunk with a VAD find its model beside the library.
    let heard = asr.run(&listen)?.text().ok_or("no text")?.text;

    let (reference, hypothesis) = (words(&args[2]), words(&heard));
    let errors = word_errors(&reference, &hypothesis);
    println!("said:  {}\nheard: {heard}", args[2]);
    println!(
        "{} → {}: WER {:.1}% ({errors}/{})",
        tts_model.family(),
        asr_model.family(),
        100.0 * errors as f64 / reference.len().max(1) as f64,
        reference.len()
    );
    Ok(())
}
