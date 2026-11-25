use std::marker::PhantomData;
use std::os::raw::{c_float, c_int, c_void};
use std::ptr::{self, NonNull};
use std::time::Duration;

use audiocpp_sys as sys;

use crate::ffi;

/// Audio borrowed from a result: interleaved samples in [-1, 1].
#[derive(Debug, Clone, Copy)]
pub struct Audio<'a> {
    pub samples: &'a [f32],
    pub sample_rate: u32,
    pub channels: u16,
}

impl Audio<'_> {
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }

    pub fn duration(&self) -> Duration {
        if self.sample_rate == 0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(self.frames() as f64 / f64::from(self.sample_rate))
        }
    }

    pub fn to_vec(&self) -> Vec<f32> {
        self.samples.to_vec()
    }
}

/// Text output (ASR) and the language the model reports ("" when none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub language: String,
}

/// A timed piece of text. Positions are samples of the request's audio.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start_sample: i64,
    pub end_sample: i64,
    pub confidence: f32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerTurn {
    pub start_sample: i64,
    pub end_sample: i64,
    pub speaker_id: String,
    pub confidence: f32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub word: String,
    pub start_sample: i64,
    pub end_sample: i64,
    pub confidence: f32,
}

/// The kind of an opaque payload a family produces or consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    SpeakerEmbedding,
    StyleEmbedding,
    PromptEmbedding,
    AcousticTokens,
    Midi,
    TranscriptAlignment,
    DiarizationState,
    VadState,
    Custom,
    Unknown(i32),
}

impl ArtifactKind {
    pub(crate) fn raw(self) -> sys::audiocpp_artifact_kind {
        match self {
            ArtifactKind::SpeakerEmbedding => sys::AUDIOCPP_ARTIFACT_SPEAKER_EMBEDDING,
            ArtifactKind::StyleEmbedding => sys::AUDIOCPP_ARTIFACT_STYLE_EMBEDDING,
            ArtifactKind::PromptEmbedding => sys::AUDIOCPP_ARTIFACT_PROMPT_EMBEDDING,
            ArtifactKind::AcousticTokens => sys::AUDIOCPP_ARTIFACT_ACOUSTIC_TOKENS,
            ArtifactKind::Midi => sys::AUDIOCPP_ARTIFACT_MIDI,
            ArtifactKind::TranscriptAlignment => sys::AUDIOCPP_ARTIFACT_TRANSCRIPT_ALIGNMENT,
            ArtifactKind::DiarizationState => sys::AUDIOCPP_ARTIFACT_DIARIZATION_STATE,
            ArtifactKind::VadState => sys::AUDIOCPP_ARTIFACT_VAD_STATE,
            ArtifactKind::Custom => sys::AUDIOCPP_ARTIFACT_CUSTOM,
            ArtifactKind::Unknown(code) => code,
        }
    }

    fn from_raw(code: sys::audiocpp_artifact_kind) -> Self {
        match code {
            sys::AUDIOCPP_ARTIFACT_SPEAKER_EMBEDDING => ArtifactKind::SpeakerEmbedding,
            sys::AUDIOCPP_ARTIFACT_STYLE_EMBEDDING => ArtifactKind::StyleEmbedding,
            sys::AUDIOCPP_ARTIFACT_PROMPT_EMBEDDING => ArtifactKind::PromptEmbedding,
            sys::AUDIOCPP_ARTIFACT_ACOUSTIC_TOKENS => ArtifactKind::AcousticTokens,
            sys::AUDIOCPP_ARTIFACT_MIDI => ArtifactKind::Midi,
            sys::AUDIOCPP_ARTIFACT_TRANSCRIPT_ALIGNMENT => ArtifactKind::TranscriptAlignment,
            sys::AUDIOCPP_ARTIFACT_DIARIZATION_STATE => ArtifactKind::DiarizationState,
            sys::AUDIOCPP_ARTIFACT_VAD_STATE => ArtifactKind::VadState,
            sys::AUDIOCPP_ARTIFACT_CUSTOM => ArtifactKind::Custom,
            other => ArtifactKind::Unknown(other),
        }
    }
}

/// An artifact copied out of a result: persist it, and feed it back with
/// [`crate::Request::add_artifact`] to reuse a voice without re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub id: String,
    pub payload: Vec<u8>,
    pub meta: Vec<(String, String)>,
}

/// Read access to a result's outputs, shared by [`TaskResult`] and the
/// [`ResultView`] of a stream event. Audio is borrowed; everything else is
/// copied out.
pub trait ResultData {
    #[doc(hidden)]
    fn raw(&self) -> *const sys::audiocpp_result;

    /// The audio output, if the task produced one.
    fn audio(&self) -> Option<Audio<'_>> {
        let r = self.raw();
        if r.is_null() {
            return None;
        }
        let (mut samples, mut frames, mut rate, mut channels): (*const c_float, usize, c_int, c_int) =
            (ptr::null(), 0, 0, 0);
        // SAFETY: valid result; out pointers valid; the samples are borrowed for &self.
        let st = unsafe { sys::audiocpp_result_audio(r, &mut samples, &mut frames, &mut rate, &mut channels) };
        if st != sys::AUDIOCPP_OK {
            return None;
        }
        Some(make_audio(samples, frames, rate, channels))
    }

    /// The text output (ASR, alignment), if the task produced one.
    fn text(&self) -> Option<Transcript> {
        let r = self.raw();
        if r.is_null() {
            return None;
        }
        let (mut text, mut language) = (ptr::null(), ptr::null());
        // SAFETY: valid result; borrowed strings copied at once.
        let st = unsafe { sys::audiocpp_result_text(r, &mut text, &mut language) };
        (st == sys::AUDIOCPP_OK)
            .then(|| unsafe { Transcript { text: ffi::string(text), language: ffi::string(language) } })
    }

    /// A family-specific structured output: (schema id, JSON data).
    fn custom_schema(&self) -> Option<(String, String)> {
        let r = self.raw();
        if r.is_null() {
            return None;
        }
        let (mut schema, mut data) = (ptr::null(), ptr::null());
        let st = unsafe { sys::audiocpp_result_custom_schema(r, &mut schema, &mut data) };
        (st == sys::AUDIOCPP_OK).then(|| unsafe { (ffi::string(schema), ffi::string(data)) })
    }

    fn segments(&self) -> Vec<Segment> {
        let r = self.raw();
        if r.is_null() {
            return Vec::new();
        }
        let n = unsafe { sys::audiocpp_result_segment_count(r) };
        (0..n)
            .filter_map(|i| {
                let (mut start, mut end, mut confidence, mut text) = (0i64, 0i64, 0f32, ptr::null());
                let st =
                    unsafe { sys::audiocpp_result_segment(r, i, &mut start, &mut end, &mut confidence, &mut text) };
                (st == sys::AUDIOCPP_OK).then(|| Segment {
                    start_sample: start,
                    end_sample: end,
                    confidence,
                    text: unsafe { ffi::string(text) },
                })
            })
            .collect()
    }

    fn speaker_turns(&self) -> Vec<SpeakerTurn> {
        let r = self.raw();
        if r.is_null() {
            return Vec::new();
        }
        let n = unsafe { sys::audiocpp_result_speaker_turn_count(r) };
        (0..n)
            .filter_map(|i| {
                let (mut start, mut end, mut speaker, mut confidence, mut text) =
                    (0i64, 0i64, ptr::null(), 0f32, ptr::null());
                let st = unsafe {
                    sys::audiocpp_result_speaker_turn(
                        r,
                        i,
                        &mut start,
                        &mut end,
                        &mut speaker,
                        &mut confidence,
                        &mut text,
                    )
                };
                (st == sys::AUDIOCPP_OK).then(|| unsafe {
                    SpeakerTurn {
                        start_sample: start,
                        end_sample: end,
                        speaker_id: ffi::string(speaker),
                        confidence,
                        text: ffi::string(text),
                    }
                })
            })
            .collect()
    }

    fn words(&self) -> Vec<Word> {
        let r = self.raw();
        if r.is_null() {
            return Vec::new();
        }
        let n = unsafe { sys::audiocpp_result_word_count(r) };
        (0..n)
            .filter_map(|i| {
                let (mut word, mut start, mut end, mut confidence) = (ptr::null(), 0i64, 0i64, 0f32);
                let st = unsafe { sys::audiocpp_result_word(r, i, &mut word, &mut start, &mut end, &mut confidence) };
                (st == sys::AUDIOCPP_OK).then(|| Word {
                    word: unsafe { ffi::string(word) },
                    start_sample: start,
                    end_sample: end,
                    confidence,
                })
            })
            .collect()
    }

    /// Every named audio output (source separation, multi-speaker TTS), with its
    /// id copied out.
    fn named_audio(&self) -> Vec<(String, Audio<'_>)> {
        let r = self.raw();
        if r.is_null() {
            return Vec::new();
        }
        let n = unsafe { sys::audiocpp_result_named_audio_count(r) };
        (0..n)
            .filter_map(|i| {
                let (mut id, mut samples, mut frames, mut rate, mut channels) =
                    (ptr::null(), ptr::null(), 0usize, 0, 0);
                let st = unsafe {
                    sys::audiocpp_result_named_audio(r, i, &mut id, &mut samples, &mut frames, &mut rate, &mut channels)
                };
                (st == sys::AUDIOCPP_OK)
                    .then(|| (unsafe { ffi::string(id) }, make_audio(samples, frames, rate, channels)))
            })
            .collect()
    }

    fn artifacts(&self) -> Vec<Artifact> {
        let r = self.raw();
        if r.is_null() {
            return Vec::new();
        }
        let n = unsafe { sys::audiocpp_result_artifact_count(r) };
        (0..n)
            .filter_map(|i| {
                let (mut kind, mut id, mut payload, mut bytes): (sys::audiocpp_artifact_kind, _, *const c_void, usize) =
                    (0, ptr::null(), ptr::null(), 0);
                let st = unsafe { sys::audiocpp_result_artifact(r, i, &mut kind, &mut id, &mut payload, &mut bytes) };
                if st != sys::AUDIOCPP_OK {
                    return None;
                }
                let data = if payload.is_null() || bytes == 0 {
                    Vec::new()
                } else {
                    // SAFETY: the payload holds `bytes` bytes, borrowed for this call.
                    unsafe { std::slice::from_raw_parts(payload.cast::<u8>(), bytes) }.to_vec()
                };
                let m = unsafe { sys::audiocpp_result_artifact_meta_count(r, i) };
                let meta = (0..m)
                    .filter_map(|j| {
                        let (mut key, mut value) = (ptr::null(), ptr::null());
                        let st = unsafe { sys::audiocpp_result_artifact_meta(r, i, j, &mut key, &mut value) };
                        (st == sys::AUDIOCPP_OK).then(|| unsafe { (ffi::string(key), ffi::string(value)) })
                    })
                    .collect();
                Some(Artifact {
                    kind: ArtifactKind::from_raw(kind),
                    id: unsafe { ffi::string(id) },
                    payload: data,
                    meta,
                })
            })
            .collect()
    }
}

fn make_audio<'a>(samples: *const c_float, frames: usize, rate: c_int, channels: c_int) -> Audio<'a> {
    let channels = u16::try_from(channels.max(1)).unwrap_or(1);
    let len = frames.saturating_mul(usize::from(channels));
    let samples = if samples.is_null() || len == 0 {
        &[][..]
    } else {
        // SAFETY: audio.cpp returns `frames * channels` floats, borrowed for the result's lifetime.
        unsafe { std::slice::from_raw_parts(samples, len) }
    };
    Audio { samples, sample_rate: u32::try_from(rate).unwrap_or(0), channels }
}

/// The outcome of [`crate::Session::run`] or [`crate::Stream::finish`]. Read it
/// through [`ResultData`] (in the prelude of this crate's root).
pub struct TaskResult {
    ptr: NonNull<sys::audiocpp_result>,
}

// SAFETY: see Registry.
unsafe impl Send for TaskResult {}

impl TaskResult {
    pub(crate) fn from_raw(ptr: NonNull<sys::audiocpp_result>) -> Self {
        TaskResult { ptr }
    }
}

impl ResultData for TaskResult {
    fn raw(&self) -> *const sys::audiocpp_result {
        self.ptr.as_ptr()
    }
}

impl Drop for TaskResult {
    fn drop(&mut self) {
        // SAFETY: we own the result.
        unsafe { sys::audiocpp_result_free(self.ptr.as_ptr()) }
    }
}

impl std::fmt::Debug for TaskResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskResult")
            .field("audio", &self.audio().map(|a| (a.frames(), a.sample_rate, a.channels)))
            .field("text", &self.text())
            .finish_non_exhaustive()
    }
}

/// A result borrowed from a stream event (see [`crate::Event::result`]).
pub struct ResultView<'a> {
    ptr: *const sys::audiocpp_result,
    _event: PhantomData<&'a ()>,
}

impl ResultView<'_> {
    pub(crate) fn new(ptr: *const sys::audiocpp_result) -> Self {
        ResultView { ptr, _event: PhantomData }
    }
}

impl ResultData for ResultView<'_> {
    fn raw(&self) -> *const sys::audiocpp_result {
        self.ptr
    }
}
