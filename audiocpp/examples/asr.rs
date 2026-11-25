//! Speech to text from a WAV file (any rate and channel count: audio.cpp
//! converts it for the model).
//!
//!   cargo run --release --example asr --features model-sense_asr -- \
//!       models/SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf speech.wav [FAMILY]

use std::time::Instant;

use audiocpp::{Backend, Mode, Model, ModelConfig, OptionScope, Options, Registry, Request, ResultData, Task};

fn read_wav(path: &str) -> Result<(Vec<f32>, u32, u16), Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok((samples, spec.sample_rate, spec.channels))
}

/// Several audio.cpp ASR families split long audio with a VAD whose default
/// model path is relative to the working directory. With AUDIOCPP_VAD_MODEL set
/// (the folder holding silero_vad_16k.safetensors) the session uses it;
/// otherwise the request asks for fixed windows, which need no VAD model.
/// Options are only set when the model declares them, under the exact name it
/// reports: session option names carry a family prefix ("sense_asr.vad_model_path"),
/// and an unknown session key is ignored without an error.
fn chunking(model: &Model) -> (Options, Option<&'static str>) {
    let vad_key =
        model.options(OptionScope::Session).into_iter().map(|o| o.name).find(|n| n.ends_with("vad_model_path"));
    let has_chunk_mode = model.options(OptionScope::Request).iter().any(|o| o.name == "audio_chunk_mode");
    match (std::env::var("AUDIOCPP_VAD_MODEL"), vad_key) {
        (Ok(path), Some(key)) => (Options::new().with(key, path), None),
        _ if has_chunk_mode => (Options::new(), Some("fixed")),
        _ => (Options::new(), None),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: asr MODEL AUDIO.wav [FAMILY]");
        std::process::exit(2);
    }
    let config = args.get(2).map(|f| ModelConfig::family(f.as_str())).unwrap_or_default();
    let (samples, rate, channels) = read_wav(&args[1])?;

    let registry = Registry::new()?;
    let model = Model::load(&registry, &args[0], &config, &Options::new())?;
    let (session_options, chunk_mode) = chunking(&model);
    let mut session = model.session(&Task::Asr, Mode::Offline, &Backend::cpu(), &session_options)?;

    let mut request = Request::new();
    request.audio(&samples, rate, channels)?;
    if let Some(mode) = chunk_mode {
        request.option("audio_chunk_mode", mode)?;
    }
    let t0 = Instant::now();
    let result = session.run(&request)?;
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let audio_secs = samples.len() as f64 / f64::from(channels) / f64::from(rate);

    let transcript = result.text().ok_or("the model returned no text")?;
    println!("{}", transcript.text);
    eprintln!(
        "{} on {audio_secs:.2} s of audio: {ms:.0} ms (RTF {:.3}){}",
        model.family(),
        ms / 1e3 / audio_secs.max(1e-9),
        if transcript.language.is_empty() { String::new() } else { format!(", language {}", transcript.language) }
    );
    for s in result.segments() {
        eprintln!("  [{} .. {}] {}", s.start_sample, s.end_sample, s.text);
    }
    Ok(())
}
