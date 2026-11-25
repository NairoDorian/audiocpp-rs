#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// A test model under `models/` (or `$AUDIOCPP_TEST_MODELS`). None, with a loud
/// SKIPPED line, when it is absent; a panic instead when AUDIOCPP_REQUIRE_MODELS
/// is set, so CI cannot mistake a skip for a pass.
pub fn model(relative: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("AUDIOCPP_TEST_MODELS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("models"));
    let path = dir.join(relative);
    if path.is_file() {
        return Some(path);
    }
    if std::env::var_os("AUDIOCPP_REQUIRE_MODELS").is_some() {
        panic!("missing test model {} (uv run scripts/fetch_test_models.py)", path.display());
    }
    eprintln!("SKIPPED: {} is missing (uv run scripts/fetch_test_models.py)", path.display());
    None
}

pub const TINY_VOCA: &str = "tiny-voca-GGUF/tiny-voca-orig.gguf";
pub const SENSEVOICE: &str = "SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf";

pub fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Word-level edit distance.
pub fn word_errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let (r, h) = (words(reference), words(hypothesis));
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, hw) in h.iter().enumerate() {
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(rw != hw)));
        }
        prev = cur;
    }
    (prev[h.len()], r.len())
}

/// Mono band-limited resampling (Kaiser-windowed sinc), test-only: audio.cpp's
/// VAD chunking wants 16 kHz while tiny_voca speaks at 24 kHz.
pub fn resample_mono(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return input.to_vec();
    }
    let ratio = f64::from(to) / f64::from(from);
    let scale = ratio.min(1.0);
    let cutoff = 0.97 * scale;
    let half = 32.0 / scale; // kernel half-width, in input samples
    let beta = 9.0;
    let i0 = |x: f64| {
        let (mut sum, mut term, q) = (1.0, 1.0, x * x / 4.0);
        for k in 1..64 {
            term *= q / f64::from(k * k);
            sum += term;
        }
        sum
    };
    let i0_beta = i0(beta);
    let n_out = (input.len() as f64 * ratio).round() as usize;
    (0..n_out)
        .map(|j| {
            let t = j as f64 / ratio;
            let first = ((t - half).ceil() as i64).max(0);
            let last = ((t + half).floor() as i64).min(input.len() as i64 - 1);
            let (mut acc, mut wsum) = (0.0, 0.0);
            for k in first..=last {
                let x = t - k as f64;
                let u = x / half;
                if u.abs() >= 1.0 {
                    continue;
                }
                let arg = cutoff * x;
                let sinc = if arg.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * arg).sin() / (std::f64::consts::PI * arg)
                };
                let h = sinc * i0(beta * (1.0 - u * u).sqrt()) / i0_beta;
                acc += h * f64::from(input[k as usize]);
                wsum += h;
            }
            if wsum == 0.0 {
                0.0
            } else {
                (acc / wsum) as f32
            }
        })
        .collect()
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}
