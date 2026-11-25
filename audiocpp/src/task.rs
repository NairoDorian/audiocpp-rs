use std::fmt;

use audiocpp_sys as sys;

use crate::ffi;

/// What a session does. The tokens are audio.cpp's C ABI vocabulary
/// (`audiocpp_task_name`); a model spec may spell some of them differently,
/// which [`Task::from_spec_name`] translates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Task {
    /// `vad`: voice activity detection.
    Vad,
    /// `asr`: speech to text.
    Asr,
    /// `diar`: diarization (who spoke when).
    Diarization,
    /// `sep`: source separation.
    Separation,
    /// `gen`: music, sound-effect and other audio generation.
    Generation,
    /// `tts`: text to speech.
    Tts,
    /// `clon`: voice cloning from a reference recording.
    VoiceClone,
    /// `vc`: voice conversion.
    VoiceConversion,
    /// `s2s`: speech to speech.
    SpeechToSpeech,
    /// `align`: forced alignment.
    Alignment,
    /// `vdes`: voice design from a description.
    VoiceDesign,
    /// `spk`: speaker embedding / identification.
    Speaker,
    /// `svc`: singing voice conversion.
    SingingVoiceConversion,
    /// `midi`: audio to MIDI.
    Midi,
    /// `wake`: wake-word detection.
    WakeWord,
    /// `cls`: audio classification.
    Classification,
    /// A token this crate does not know yet (a newer audio.cpp).
    Other(String),
}

impl Task {
    /// The C ABI token ("tts", "asr", "clon", ...).
    pub fn as_str(&self) -> &str {
        match self {
            Task::Vad => "vad",
            Task::Asr => "asr",
            Task::Diarization => "diar",
            Task::Separation => "sep",
            Task::Generation => "gen",
            Task::Tts => "tts",
            Task::VoiceClone => "clon",
            Task::VoiceConversion => "vc",
            Task::SpeechToSpeech => "s2s",
            Task::Alignment => "align",
            Task::VoiceDesign => "vdes",
            Task::Speaker => "spk",
            Task::SingingVoiceConversion => "svc",
            Task::Midi => "midi",
            Task::WakeWord => "wake",
            Task::Classification => "cls",
            Task::Other(token) => token,
        }
    }

    /// The task for a C ABI token; unknown tokens become [`Task::Other`].
    pub fn from_token(token: &str) -> Task {
        match token {
            "vad" => Task::Vad,
            "asr" => Task::Asr,
            "diar" => Task::Diarization,
            "sep" => Task::Separation,
            "gen" => Task::Generation,
            "tts" => Task::Tts,
            "clon" => Task::VoiceClone,
            "vc" => Task::VoiceConversion,
            "s2s" => Task::SpeechToSpeech,
            "align" => Task::Alignment,
            "vdes" => Task::VoiceDesign,
            "spk" => Task::Speaker,
            "svc" => Task::SingingVoiceConversion,
            "midi" => Task::Midi,
            "wake" => Task::WakeWord,
            "cls" => Task::Classification,
            other => Task::Other(other.to_string()),
        }
    }

    /// The task a model spec's task name means ("music" is [`Task::Generation`],
    /// "clone" is [`Task::VoiceClone`]), or None when this build has no such task.
    pub fn from_spec_name(spec_task: &str) -> Option<Task> {
        let name = ffi::cstring("audiocpp_task_from_spec_name", "task name", spec_task).ok()?;
        // SAFETY: a NUL-terminated argument; the result is static or NULL.
        let token = unsafe { sys::audiocpp_task_from_spec_name(name.as_ptr()) };
        (!token.is_null()).then(|| Task::from_token(&unsafe { ffi::string(token) }))
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every task this build of audio.cpp accepts, in its order.
pub fn tasks() -> Vec<Task> {
    // SAFETY: plain queries; names are static.
    let n = unsafe { sys::audiocpp_task_count() };
    (0..n)
        .filter_map(|i| {
            let p = unsafe { sys::audiocpp_task_name(i) };
            (!p.is_null()).then(|| Task::from_token(&unsafe { ffi::string(p) }))
        })
        .collect()
}

/// How a session takes its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// One request in, one result out.
    Offline,
    /// Audio pushed in chunks, events pulled out ([`crate::Stream`]).
    Streaming,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Offline => "offline",
            Mode::Streaming => "streaming",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
