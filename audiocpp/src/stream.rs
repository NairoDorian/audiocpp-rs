use std::os::raw::c_int;
use std::ptr::{self, NonNull};

use audiocpp_sys as sys;

use crate::error::check;
use crate::{Error, Request, Result, ResultView, Session, TaskResult};

/// What a streaming session takes as input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamInput {
    None,
    AudioChunks,
    Unknown(i32),
}

/// How a streaming session gives its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutput {
    /// Only [`Stream::finish`] carries the result.
    FinalResult,
    /// Events come back from [`Stream::push`] and [`Stream::next_event`].
    PullEvents,
    Unknown(i32),
}

/// A streaming session's chunking preference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamPolicy {
    pub input: StreamInput,
    pub output: StreamOutput,
    /// Push this many frames per call where you can.
    pub preferred_chunk_samples: i64,
    pub preferred_chunk_seconds: f64,
}

impl StreamPolicy {
    pub(crate) fn query(session: *const sys::audiocpp_session) -> Result<Self> {
        let (mut input, mut output, mut samples, mut seconds): (c_int, c_int, i64, f64) = (0, 0, 0, 0.0);
        // SAFETY: valid session; every out pointer is valid.
        check(
            unsafe { sys::audiocpp_stream_policy(session, &mut input, &mut output, &mut samples, &mut seconds) },
            "audiocpp_stream_policy",
        )?;
        Ok(StreamPolicy {
            input: match input {
                sys::AUDIOCPP_STREAM_INPUT_NONE => StreamInput::None,
                sys::AUDIOCPP_STREAM_INPUT_AUDIO_CHUNKS => StreamInput::AudioChunks,
                other => StreamInput::Unknown(other),
            },
            output: match output {
                sys::AUDIOCPP_STREAM_OUTPUT_FINAL_RESULT => StreamOutput::FinalResult,
                sys::AUDIOCPP_STREAM_OUTPUT_PULL_EVENTS => StreamOutput::PullEvents,
                other => StreamOutput::Unknown(other),
            },
            preferred_chunk_samples: samples,
            preferred_chunk_seconds: seconds,
        })
    }
}

/// A stream in flight on a [`Session`]: push audio, pull events, finish.
/// Pull-based: no callback crosses the FFI boundary.
pub struct Stream<'s> {
    session: &'s mut Session,
}

impl<'s> Stream<'s> {
    pub(crate) fn start(session: &'s mut Session, request: Option<&Request>) -> Result<Self> {
        // SAFETY: valid session; NULL request allowed.
        check(
            unsafe { sys::audiocpp_stream_start(session.as_ptr(), request.map_or(ptr::null(), Request::as_ptr)) },
            "audiocpp_stream_start",
        )?;
        Ok(Stream { session })
    }

    /// Feeds one chunk of interleaved PCM. `start_sample` is the chunk's
    /// position in the whole stream. Returns the event the chunk produced, if any.
    pub fn push(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
        start_sample: i64,
    ) -> Result<Option<Event>> {
        const CALL: &str = "audiocpp_stream_push";
        if channels == 0 || sample_rate == 0 || samples.len() % usize::from(channels) != 0 {
            return Err(Error::invalid(CALL, "bad sample_rate / channels / sample count"));
        }
        let rate = c_int::try_from(sample_rate).map_err(|_| Error::invalid(CALL, "sample_rate is too large"))?;
        let mut event = ptr::null_mut();
        // SAFETY: valid session; the samples are copied by the call.
        check(
            unsafe {
                sys::audiocpp_stream_push(
                    self.session.as_ptr(),
                    samples.as_ptr(),
                    samples.len() / usize::from(channels),
                    rate,
                    c_int::from(channels),
                    start_sample,
                    &mut event,
                )
            },
            CALL,
        )?;
        Ok(NonNull::new(event).map(|ptr| Event { ptr }))
    }

    /// The next event the family queued on its own; None when the queue is empty.
    pub fn next_event(&mut self) -> Result<Option<Event>> {
        let mut event = ptr::null_mut();
        // SAFETY: valid session.
        check(
            unsafe { sys::audiocpp_stream_next_event(self.session.as_ptr(), &mut event) },
            "audiocpp_stream_next_event",
        )?;
        Ok(NonNull::new(event).map(|ptr| Event { ptr }))
    }

    /// Ends the stream and returns the final result.
    pub fn finish(self) -> Result<TaskResult> {
        const CALL: &str = "audiocpp_stream_finish";
        let mut out = ptr::null_mut();
        // SAFETY: valid session.
        check(unsafe { sys::audiocpp_stream_finish(self.session.as_ptr(), &mut out) }, CALL)?;
        NonNull::new(out).map(TaskResult::from_raw).ok_or_else(|| Error::null_handle(CALL))
    }

    /// Drops the stream's state; the stream can be fed again from the start.
    pub fn reset(&mut self) -> Result<()> {
        // SAFETY: valid session.
        check(unsafe { sys::audiocpp_stream_reset(self.session.as_ptr()) }, "audiocpp_stream_reset")
    }
}

/// What a voice-activity event marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceActivityKind {
    SpeechStart,
    SpeechEnd,
    SpeechSegment,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceActivity {
    pub kind: VoiceActivityKind,
    pub sample: i64,
    pub probability: f32,
}

/// One event of a stream. Its text, audio and segments read through
/// [`Event::result`] (partial text appears as the result's text).
pub struct Event {
    ptr: NonNull<sys::audiocpp_event>,
}

// SAFETY: see Registry.
unsafe impl Send for Event {}

impl Event {
    pub fn is_final(&self) -> bool {
        // SAFETY: valid event.
        unsafe { sys::audiocpp_event_is_final(self.ptr.as_ptr()) == 1 }
    }

    /// The event's outputs, borrowed from the event.
    pub fn result(&self) -> ResultView<'_> {
        // SAFETY: valid event; the view lives no longer than &self.
        ResultView::new(unsafe { sys::audiocpp_event_as_result(self.ptr.as_ptr()) })
    }

    pub fn voice_activity(&self) -> Vec<VoiceActivity> {
        let n = unsafe { sys::audiocpp_event_voice_activity_count(self.ptr.as_ptr()) };
        (0..n)
            .filter_map(|i| {
                let (mut kind, mut sample, mut probability) = (0, 0i64, 0f32);
                let st = unsafe {
                    sys::audiocpp_event_voice_activity(self.ptr.as_ptr(), i, &mut kind, &mut sample, &mut probability)
                };
                (st == sys::AUDIOCPP_OK).then(|| VoiceActivity {
                    kind: match kind {
                        sys::AUDIOCPP_VOICE_ACTIVITY_SPEECH_START => VoiceActivityKind::SpeechStart,
                        sys::AUDIOCPP_VOICE_ACTIVITY_SPEECH_END => VoiceActivityKind::SpeechEnd,
                        sys::AUDIOCPP_VOICE_ACTIVITY_SPEECH_SEGMENT => VoiceActivityKind::SpeechSegment,
                        other => VoiceActivityKind::Unknown(other),
                    },
                    sample,
                    probability,
                })
            })
            .collect()
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: we own the event.
        unsafe { sys::audiocpp_event_free(self.ptr.as_ptr()) }
    }
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Event").field("final", &self.is_final()).finish_non_exhaustive()
    }
}
