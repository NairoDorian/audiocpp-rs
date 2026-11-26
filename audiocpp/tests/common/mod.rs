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

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}
