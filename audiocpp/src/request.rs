use std::fmt;
use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;

use audiocpp_sys as sys;

use crate::error::check;
use crate::{ffi, ArtifactKind, Error, Result};

/// The input of one run: text, audio, a voice, style and options. Every setter
/// copies its arguments, so nothing needs to outlive the call.
///
/// Setters return `Result<&mut Self>`, so they chain with `?`:
/// `req.text("Hello")?.voice_id("af_heart")?.speaking_rate(1.1)?;`
pub struct Request {
    ptr: NonNull<sys::audiocpp_request>,
}

// SAFETY: see Registry.
unsafe impl Send for Request {}

fn check_audio(call: &'static str, samples: &[f32], sample_rate: u32, channels: u16) -> Result<(usize, c_int, c_int)> {
    if channels == 0 || sample_rate == 0 {
        return Err(Error::invalid(call, "sample_rate and channels must be positive"));
    }
    if samples.len() % channels as usize != 0 {
        return Err(Error::invalid(call, "the sample count is not a multiple of the channel count"));
    }
    let rate = c_int::try_from(sample_rate).map_err(|_| Error::invalid(call, "sample_rate is too large"))?;
    Ok((samples.len() / channels as usize, rate, c_int::from(channels)))
}

impl Request {
    pub fn new() -> Self {
        // SAFETY: returns a new handle; NULL only when memory is exhausted.
        let raw = unsafe { sys::audiocpp_request_create() };
        Request { ptr: NonNull::new(raw).expect("audiocpp_request_create returned NULL (out of memory)") }
    }

    fn set_str(
        &mut self,
        call: &'static str,
        what: &str,
        value: &str,
        f: unsafe extern "C" fn(*mut sys::audiocpp_request, *const c_char) -> sys::audiocpp_status,
    ) -> Result<&mut Self> {
        let v = ffi::cstring(call, what, value)?;
        // SAFETY: valid handle and string; the call copies it.
        check(unsafe { f(self.ptr.as_ptr(), v.as_ptr()) }, call)?;
        Ok(self)
    }

    fn set_f32(
        &mut self,
        call: &'static str,
        value: f32,
        f: unsafe extern "C" fn(*mut sys::audiocpp_request, f32) -> sys::audiocpp_status,
    ) -> Result<&mut Self> {
        // SAFETY: valid handle.
        check(unsafe { f(self.ptr.as_ptr(), value) }, call)?;
        Ok(self)
    }

    /// The text to speak (TTS) or align. Language: [`Request::text_language`],
    /// or [`Request::text_with_language`] to also set the `language` option.
    pub fn text(&mut self, text: &str) -> Result<&mut Self> {
        let t = ffi::cstring("audiocpp_request_set_text", "text", text)?;
        // SAFETY: valid handle and string; NULL language.
        check(
            unsafe { sys::audiocpp_request_set_text(self.ptr.as_ptr(), t.as_ptr(), std::ptr::null()) },
            "audiocpp_request_set_text",
        )?;
        Ok(self)
    }

    /// The text, its language, and the request option `language` (what
    /// `audiocpp_cli --language` does). Models that validate their options
    /// strictly reject a language they do not declare: then use
    /// [`Request::text`] + [`Request::text_language`].
    pub fn text_with_language(&mut self, text: &str, language: &str) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_text";
        let t = ffi::cstring(CALL, "text", text)?;
        let l = ffi::cstring(CALL, "language", language)?;
        // SAFETY: valid handle and strings.
        check(unsafe { sys::audiocpp_request_set_text(self.ptr.as_ptr(), t.as_ptr(), l.as_ptr()) }, CALL)?;
        Ok(self)
    }

    /// The transcript language alone, without the `language` option.
    pub fn text_language(&mut self, language: &str) -> Result<&mut Self> {
        self.set_str(
            "audiocpp_request_set_text_language",
            "language",
            language,
            sys::audiocpp_request_set_text_language,
        )
    }

    /// Input audio: interleaved float PCM in [-1, 1], any rate and channel count.
    pub fn audio(&mut self, samples: &[f32], sample_rate: u32, channels: u16) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_audio";
        let (frames, rate, ch) = check_audio(CALL, samples, sample_rate, channels)?;
        // SAFETY: valid handle; `samples` holds frames * channels floats, copied by the call.
        check(unsafe { sys::audiocpp_request_set_audio(self.ptr.as_ptr(), samples.as_ptr(), frames, rate, ch) }, CALL)?;
        Ok(self)
    }

    /// A reference voice, for cloning and conversion families
    /// ([`crate::Model::supports_speaker_reference`]).
    pub fn voice_audio(&mut self, samples: &[f32], sample_rate: u32, channels: u16) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_voice_audio";
        let (frames, rate, ch) = check_audio(CALL, samples, sample_rate, channels)?;
        // SAFETY: as in `audio`.
        check(
            unsafe { sys::audiocpp_request_set_voice_audio(self.ptr.as_ptr(), samples.as_ptr(), frames, rate, ch) },
            CALL,
        )?;
        Ok(self)
    }

    /// A built-in or cached voice, by id (what `audiocpp_cli --voice-id` sets).
    pub fn voice_id(&mut self, id: &str) -> Result<&mut Self> {
        self.set_str("audiocpp_request_set_voice_id", "voice id", id, sys::audiocpp_request_set_voice_id)
    }

    /// Style conditioning, for families that take it
    /// ([`crate::Model::supports_style_condition`]).
    pub fn style_language(&mut self, language: &str) -> Result<&mut Self> {
        self.set_str(
            "audiocpp_request_set_style_language",
            "language",
            language,
            sys::audiocpp_request_set_style_language,
        )
    }

    pub fn emotion(&mut self, emotion: &str) -> Result<&mut Self> {
        self.set_str("audiocpp_request_set_emotion", "emotion", emotion, sys::audiocpp_request_set_emotion)
    }

    /// 1.0 = the model's normal rate.
    pub fn speaking_rate(&mut self, rate: f32) -> Result<&mut Self> {
        self.set_f32("audiocpp_request_set_speaking_rate", rate, sys::audiocpp_request_set_speaking_rate)
    }

    pub fn pitch_shift(&mut self, shift: f32) -> Result<&mut Self> {
        self.set_f32("audiocpp_request_set_pitch_shift", shift, sys::audiocpp_request_set_pitch_shift)
    }

    pub fn energy_scale(&mut self, scale: f32) -> Result<&mut Self> {
        self.set_f32("audiocpp_request_set_energy_scale", scale, sys::audiocpp_request_set_energy_scale)
    }

    pub fn style_tag(&mut self, key: &str, value: &str) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_style_tag";
        let k = ffi::cstring(CALL, "key", key)?;
        let v = ffi::cstring(CALL, "value", value)?;
        // SAFETY: valid handle and strings.
        check(unsafe { sys::audiocpp_request_set_style_tag(self.ptr.as_ptr(), k.as_ptr(), v.as_ptr()) }, CALL)?;
        Ok(self)
    }

    /// A request option, as the model declares it ([`crate::OptionScope::Request`]).
    /// A later call with the same key replaces the value.
    pub fn option(&mut self, key: &str, value: impl ToString) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_option";
        let k = ffi::cstring(CALL, "key", key)?;
        let v = ffi::cstring(CALL, "value", &value.to_string())?;
        // SAFETY: valid handle and strings.
        check(unsafe { sys::audiocpp_request_set_option(self.ptr.as_ptr(), k.as_ptr(), v.as_ptr()) }, CALL)?;
        Ok(self)
    }

    /// The sampling seed (option `seed`); -1 asks audio.cpp for a fresh random one.
    pub fn seed(&mut self, seed: i64) -> Result<&mut Self> {
        self.option("seed", seed)
    }

    /// A list-valued option (the `*_list` types a spec declares). Replaces any
    /// earlier list under the same key; an empty list is distinct from none.
    pub fn option_array(&mut self, key: &str, values: &[&str]) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_option_array";
        let k = ffi::cstring(CALL, "key", key)?;
        let owned = values.iter().map(|v| ffi::cstring(CALL, "value", v)).collect::<Result<Vec<_>>>()?;
        let ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        // SAFETY: valid handle; `ptrs` holds `values.len()` valid strings, copied by the call.
        check(
            unsafe { sys::audiocpp_request_set_option_array(self.ptr.as_ptr(), k.as_ptr(), ptrs.as_ptr(), ptrs.len()) },
            CALL,
        )?;
        Ok(self)
    }

    /// Feeds an artifact (e.g. a speaker embedding saved from an earlier result)
    /// back into a request. Returns its index, for [`Request::artifact_meta`].
    pub fn add_artifact(&mut self, kind: ArtifactKind, id: &str, payload: &[u8]) -> Result<usize> {
        const CALL: &str = "audiocpp_request_add_artifact";
        let id = ffi::cstring(CALL, "id", id)?;
        let mut index = 0usize;
        // SAFETY: valid handle; the payload is copied by the call.
        check(
            unsafe {
                sys::audiocpp_request_add_artifact(
                    self.ptr.as_ptr(),
                    kind.raw(),
                    id.as_ptr(),
                    payload.as_ptr().cast(),
                    payload.len(),
                    &mut index,
                )
            },
            CALL,
        )?;
        Ok(index)
    }

    pub fn artifact_meta(&mut self, index: usize, key: &str, value: &str) -> Result<&mut Self> {
        const CALL: &str = "audiocpp_request_set_artifact_meta";
        let k = ffi::cstring(CALL, "key", key)?;
        let v = ffi::cstring(CALL, "value", value)?;
        // SAFETY: valid handle and strings.
        check(
            unsafe { sys::audiocpp_request_set_artifact_meta(self.ptr.as_ptr(), index, k.as_ptr(), v.as_ptr()) },
            CALL,
        )?;
        Ok(self)
    }

    pub(crate) fn as_ptr(&self) -> *const sys::audiocpp_request {
        self.ptr.as_ptr()
    }
}

impl Default for Request {
    fn default() -> Self {
        Request::new()
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        // SAFETY: we own the handle.
        unsafe { sys::audiocpp_request_free(self.ptr.as_ptr()) }
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request").finish_non_exhaustive()
    }
}
