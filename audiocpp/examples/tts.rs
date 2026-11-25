//! Text to speech into a WAV file.
//!
//!   cargo run --release --example tts --features model-tiny_voca -- \
//!       models/tiny-voca-GGUF/tiny-voca-orig.gguf "Hello from Rust." hello.wav [VOICE] [FAMILY]

use std::time::Instant;

use audiocpp::{Backend, Mode, Model, ModelConfig, Options, Registry, Request, ResultData, Task};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: tts MODEL TEXT OUT.wav [VOICE] [FAMILY]");
        std::process::exit(2);
    }
    let config = args.get(4).map(|f| ModelConfig::family(f.as_str())).unwrap_or_default();

    let registry = Registry::new()?;
    let t0 = Instant::now();
    let model = Model::load(&registry, &args[0], &config, &Options::new())?;
    println!("loaded {} in {:.0} ms", model.family(), t0.elapsed().as_secs_f64() * 1e3);

    let mut session = model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new())?;
    let mut request = Request::new();
    request.text(&args[1])?;
    if let Some(voice) = args.get(3) {
        request.voice_id(voice)?;
    }

    let t1 = Instant::now();
    let result = session.run(&request)?;
    let ms = t1.elapsed().as_secs_f64() * 1e3;
    let audio = result.audio().ok_or("the model returned no audio")?;
    let secs = audio.duration().as_secs_f64();
    println!("{secs:.2} s of audio at {} Hz in {ms:.0} ms (RTF {:.3})", audio.sample_rate, ms / 1e3 / secs.max(1e-9));

    let spec = hound::WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&args[2], spec)?;
    for &s in audio.samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    writer.finalize()?;
    println!("wrote {}", args[2]);
    Ok(())
}
