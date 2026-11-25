//! Raw FFI bindings to audio.cpp's C ABI, `include/audiocpp.h`.
//!
//! These declarations mirror the header of the pinned audio.cpp (`audio.cpp/`
//! submodule) one to one. They are written by hand, and `cargo xtask
//! check-abi` proves them against the header: every function, the ABI
//! version, and the header's hash recorded in `abi.lock`. Any change to the
//! header fails that check until the declarations are reviewed.
//!
//! Use the safe `audiocpp` crate unless you need the raw surface. The contract
//! these functions follow (from the header and audio.cpp's docs/c_api.md):
//! - every fallible call returns an `audiocpp_status`; `audiocpp_last_error()`
//!   holds the detail of the last failing call **on the calling thread**;
//! - returned `const char *` / `const float *` are borrowed from the handle that
//!   produced them; returned strings are never NULL;
//! - handles keep their parents alive (a session holds its model, a model its
//!   registry), so they may be freed in any order; freeing NULL is a no-op;
//! - handles are not individually thread-safe; separate handles may be used
//!   from separate threads.

#![allow(non_camel_case_types)]

use std::marker::{PhantomData, PhantomPinned};
use std::os::raw::{c_char, c_float, c_int, c_void};

/// ABI version of the header these declarations were written against.
pub const AUDIOCPP_ABI_VERSION_MAJOR: u32 = 0;
pub const AUDIOCPP_ABI_VERSION_MINOR: u32 = 4;
pub const AUDIOCPP_ABI_VERSION_PATCH: u32 = 0;

// ---------------------------------------------------------------------------
// Status and logging
// ---------------------------------------------------------------------------
pub type audiocpp_status = c_int;
pub const AUDIOCPP_OK: audiocpp_status = 0;
pub const AUDIOCPP_ERR_INVALID_ARGUMENT: audiocpp_status = 1;
pub const AUDIOCPP_ERR_UNSUPPORTED_FAMILY: audiocpp_status = 2;
pub const AUDIOCPP_ERR_LOAD_FAILED: audiocpp_status = 3;
pub const AUDIOCPP_ERR_RUNTIME: audiocpp_status = 4;
pub const AUDIOCPP_ERR_OUT_OF_MEMORY: audiocpp_status = 5;
pub const AUDIOCPP_ERR_OUT_OF_RANGE: audiocpp_status = 6;
pub const AUDIOCPP_ERR_NOT_AVAILABLE: audiocpp_status = 7;

pub type audiocpp_log_level = c_int;
pub const AUDIOCPP_LOG_LEVEL_NONE: audiocpp_log_level = 0;
pub const AUDIOCPP_LOG_LEVEL_DEBUG: audiocpp_log_level = 1;
pub const AUDIOCPP_LOG_LEVEL_INFO: audiocpp_log_level = 2;
pub const AUDIOCPP_LOG_LEVEL_WARN: audiocpp_log_level = 3;
pub const AUDIOCPP_LOG_LEVEL_ERROR: audiocpp_log_level = 4;
pub const AUDIOCPP_LOG_LEVEL_CONT: audiocpp_log_level = 5;

pub type audiocpp_log_callback =
    Option<unsafe extern "C" fn(level: audiocpp_log_level, text: *const c_char, user_data: *mut c_void)>;

// ---------------------------------------------------------------------------
// Opaque handles
// ---------------------------------------------------------------------------
macro_rules! opaque {
    ($($name:ident),* $(,)?) => {$(
        #[repr(C)]
        pub struct $name {
            _data: [u8; 0],
            _marker: PhantomData<(*mut u8, PhantomPinned)>,
        }
    )*};
}
opaque!(
    audiocpp_registry,
    audiocpp_model,
    audiocpp_session,
    audiocpp_options,
    audiocpp_request,
    audiocpp_result,
    audiocpp_event,
);

// ---------------------------------------------------------------------------
// Plain structs and enums
// ---------------------------------------------------------------------------
/// Model selection; every field may be NULL.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct audiocpp_model_config {
    pub family_hint: *const c_char,
    pub config_id: *const c_char,
    pub weight_id: *const c_char,
    pub model_spec_override: *const c_char,
}

/// `backend`: "cpu", "cuda", "hip"/"rocm", "vulkan", "metal", "webgpu", "sycl"
/// or "best"; NULL means "cpu".
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct audiocpp_backend_config {
    pub backend: *const c_char,
    pub device: c_int,
    pub threads: c_int,
}

pub type audiocpp_option_scope = c_int;
pub const AUDIOCPP_OPTION_SCOPE_REQUEST: audiocpp_option_scope = 0;
pub const AUDIOCPP_OPTION_SCOPE_SESSION: audiocpp_option_scope = 1;
pub const AUDIOCPP_OPTION_SCOPE_LOAD: audiocpp_option_scope = 2;

pub type audiocpp_artifact_kind = c_int;
pub const AUDIOCPP_ARTIFACT_SPEAKER_EMBEDDING: audiocpp_artifact_kind = 0;
pub const AUDIOCPP_ARTIFACT_STYLE_EMBEDDING: audiocpp_artifact_kind = 1;
pub const AUDIOCPP_ARTIFACT_PROMPT_EMBEDDING: audiocpp_artifact_kind = 2;
pub const AUDIOCPP_ARTIFACT_ACOUSTIC_TOKENS: audiocpp_artifact_kind = 3;
pub const AUDIOCPP_ARTIFACT_MIDI: audiocpp_artifact_kind = 4;
pub const AUDIOCPP_ARTIFACT_TRANSCRIPT_ALIGNMENT: audiocpp_artifact_kind = 5;
pub const AUDIOCPP_ARTIFACT_DIARIZATION_STATE: audiocpp_artifact_kind = 6;
pub const AUDIOCPP_ARTIFACT_VAD_STATE: audiocpp_artifact_kind = 7;
pub const AUDIOCPP_ARTIFACT_CUSTOM: audiocpp_artifact_kind = 8;

pub type audiocpp_stream_input_kind = c_int;
pub const AUDIOCPP_STREAM_INPUT_NONE: audiocpp_stream_input_kind = 0;
pub const AUDIOCPP_STREAM_INPUT_AUDIO_CHUNKS: audiocpp_stream_input_kind = 1;

pub type audiocpp_stream_output_kind = c_int;
pub const AUDIOCPP_STREAM_OUTPUT_FINAL_RESULT: audiocpp_stream_output_kind = 0;
pub const AUDIOCPP_STREAM_OUTPUT_PULL_EVENTS: audiocpp_stream_output_kind = 1;

pub type audiocpp_voice_activity_kind = c_int;
pub const AUDIOCPP_VOICE_ACTIVITY_SPEECH_START: audiocpp_voice_activity_kind = 0;
pub const AUDIOCPP_VOICE_ACTIVITY_SPEECH_END: audiocpp_voice_activity_kind = 1;
pub const AUDIOCPP_VOICE_ACTIVITY_SPEECH_SEGMENT: audiocpp_voice_activity_kind = 2;

// ---------------------------------------------------------------------------
// Functions (77, in header order)
// ---------------------------------------------------------------------------
extern "C" {
    // Versioning
    pub fn audiocpp_abi_version() -> u32;
    pub fn audiocpp_build_version() -> *const c_char;

    // Status and errors
    pub fn audiocpp_last_error() -> *const c_char;
    pub fn audiocpp_status_string(status: audiocpp_status) -> *const c_char;

    // Logging
    pub fn audiocpp_log_set(log_callback: audiocpp_log_callback, user_data: *mut c_void);

    // Options
    pub fn audiocpp_options_create() -> *mut audiocpp_options;
    pub fn audiocpp_options_set(
        options: *mut audiocpp_options,
        key: *const c_char,
        value: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_options_free(options: *mut audiocpp_options);

    // Registry
    pub fn audiocpp_registry_create(
        config_path: *const c_char,
        out_registry: *mut *mut audiocpp_registry,
    ) -> audiocpp_status;
    pub fn audiocpp_registry_free(registry: *mut audiocpp_registry);
    pub fn audiocpp_registry_family_count(registry: *const audiocpp_registry) -> usize;
    pub fn audiocpp_registry_family(
        registry: *const audiocpp_registry,
        index: usize,
        out_family: *mut *const c_char,
    ) -> audiocpp_status;

    // Task vocabulary
    pub fn audiocpp_task_count() -> usize;
    pub fn audiocpp_task_name(index: usize) -> *const c_char;
    pub fn audiocpp_task_from_spec_name(spec_task: *const c_char) -> *const c_char;

    // Model
    pub fn audiocpp_model_load(
        registry: *mut audiocpp_registry,
        model_path: *const c_char,
        config: *const audiocpp_model_config,
        options: *const audiocpp_options,
        out_model: *mut *mut audiocpp_model,
    ) -> audiocpp_status;
    pub fn audiocpp_model_free(model: *mut audiocpp_model);
    pub fn audiocpp_model_family(model: *const audiocpp_model) -> *const c_char;
    pub fn audiocpp_model_description(model: *const audiocpp_model) -> *const c_char;
    pub fn audiocpp_model_supports(model: *const audiocpp_model, task: *const c_char, mode: *const c_char) -> c_int;
    pub fn audiocpp_model_supports_timestamps(model: *const audiocpp_model) -> c_int;
    pub fn audiocpp_model_supports_speaker_reference(model: *const audiocpp_model) -> c_int;
    pub fn audiocpp_model_supports_style_condition(model: *const audiocpp_model) -> c_int;
    pub fn audiocpp_model_language_count(model: *const audiocpp_model) -> usize;
    pub fn audiocpp_model_language(
        model: *const audiocpp_model,
        index: usize,
        out_language: *mut *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_model_option_count(model: *const audiocpp_model, scope: audiocpp_option_scope) -> usize;
    pub fn audiocpp_model_option(
        model: *const audiocpp_model,
        scope: audiocpp_option_scope,
        index: usize,
        out_name: *mut *const c_char,
        out_value_name: *mut *const c_char,
        out_description: *mut *const c_char,
        out_default_value: *mut *const c_char,
        out_min_value: *mut *const c_char,
        out_max_value: *mut *const c_char,
        out_required: *mut c_int,
    ) -> audiocpp_status;

    // Session
    pub fn audiocpp_session_create(
        model: *const audiocpp_model,
        task: *const c_char,
        mode: *const c_char,
        backend_config: *const audiocpp_backend_config,
        options: *const audiocpp_options,
        out_session: *mut *mut audiocpp_session,
    ) -> audiocpp_status;
    pub fn audiocpp_session_create_with_impl(
        model: *const audiocpp_model,
        task: *const c_char,
        mode: *const c_char,
        backend_config: *const audiocpp_backend_config,
        impl_: *const c_char,
        options: *const audiocpp_options,
        out_session: *mut *mut audiocpp_session,
    ) -> audiocpp_status;
    pub fn audiocpp_session_free(session: *mut audiocpp_session);
    pub fn audiocpp_session_family(session: *const audiocpp_session) -> *const c_char;
    pub fn audiocpp_session_prepare(
        session: *mut audiocpp_session,
        request: *const audiocpp_request,
    ) -> audiocpp_status;
    pub fn audiocpp_session_run(
        session: *mut audiocpp_session,
        request: *const audiocpp_request,
        out_result: *mut *mut audiocpp_result,
    ) -> audiocpp_status;

    // Request
    pub fn audiocpp_request_create() -> *mut audiocpp_request;
    pub fn audiocpp_request_free(request: *mut audiocpp_request);
    pub fn audiocpp_request_set_text(
        request: *mut audiocpp_request,
        text: *const c_char,
        language: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_text_language(
        request: *mut audiocpp_request,
        language: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_audio(
        request: *mut audiocpp_request,
        samples: *const c_float,
        frames: usize,
        sample_rate: c_int,
        channels: c_int,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_voice_audio(
        request: *mut audiocpp_request,
        samples: *const c_float,
        frames: usize,
        sample_rate: c_int,
        channels: c_int,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_voice_id(
        request: *mut audiocpp_request,
        cached_voice_id: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_style_language(
        request: *mut audiocpp_request,
        language: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_emotion(request: *mut audiocpp_request, emotion: *const c_char) -> audiocpp_status;
    pub fn audiocpp_request_set_speaking_rate(
        request: *mut audiocpp_request,
        speaking_rate: c_float,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_pitch_shift(request: *mut audiocpp_request, pitch_shift: c_float) -> audiocpp_status;
    pub fn audiocpp_request_set_energy_scale(request: *mut audiocpp_request, energy_scale: c_float) -> audiocpp_status;
    pub fn audiocpp_request_set_style_tag(
        request: *mut audiocpp_request,
        key: *const c_char,
        value: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_add_artifact(
        request: *mut audiocpp_request,
        kind: audiocpp_artifact_kind,
        id: *const c_char,
        payload: *const c_void,
        payload_bytes: usize,
        out_index: *mut usize,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_artifact_meta(
        request: *mut audiocpp_request,
        index: usize,
        key: *const c_char,
        value: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_option(
        request: *mut audiocpp_request,
        key: *const c_char,
        value: *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_request_set_option_array(
        request: *mut audiocpp_request,
        key: *const c_char,
        values: *const *const c_char,
        count: usize,
    ) -> audiocpp_status;

    // Result
    pub fn audiocpp_result_free(result: *mut audiocpp_result);
    pub fn audiocpp_result_audio(
        result: *const audiocpp_result,
        out_samples: *mut *const c_float,
        out_frames: *mut usize,
        out_sample_rate: *mut c_int,
        out_channels: *mut c_int,
    ) -> audiocpp_status;
    pub fn audiocpp_result_text(
        result: *const audiocpp_result,
        out_text: *mut *const c_char,
        out_language: *mut *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_result_custom_schema(
        result: *const audiocpp_result,
        out_schema: *mut *const c_char,
        out_data: *mut *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_result_segment_count(result: *const audiocpp_result) -> usize;
    pub fn audiocpp_result_segment(
        result: *const audiocpp_result,
        index: usize,
        out_start_sample: *mut i64,
        out_end_sample: *mut i64,
        out_confidence: *mut c_float,
        out_text: *mut *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_result_speaker_turn_count(result: *const audiocpp_result) -> usize;
    pub fn audiocpp_result_speaker_turn(
        result: *const audiocpp_result,
        index: usize,
        out_start_sample: *mut i64,
        out_end_sample: *mut i64,
        out_speaker_id: *mut *const c_char,
        out_confidence: *mut c_float,
        out_text: *mut *const c_char,
    ) -> audiocpp_status;
    pub fn audiocpp_result_word_count(result: *const audiocpp_result) -> usize;
    pub fn audiocpp_result_word(
        result: *const audiocpp_result,
        index: usize,
        out_word: *mut *const c_char,
        out_start_sample: *mut i64,
        out_end_sample: *mut i64,
        out_confidence: *mut c_float,
    ) -> audiocpp_status;
    pub fn audiocpp_result_named_audio_count(result: *const audiocpp_result) -> usize;
    pub fn audiocpp_result_named_audio(
        result: *const audiocpp_result,
        index: usize,
        out_id: *mut *const c_char,
        out_samples: *mut *const c_float,
        out_frames: *mut usize,
        out_sample_rate: *mut c_int,
        out_channels: *mut c_int,
    ) -> audiocpp_status;
    pub fn audiocpp_result_artifact_count(result: *const audiocpp_result) -> usize;
    pub fn audiocpp_result_artifact(
        result: *const audiocpp_result,
        index: usize,
        out_kind: *mut audiocpp_artifact_kind,
        out_id: *mut *const c_char,
        out_payload: *mut *const c_void,
        out_payload_bytes: *mut usize,
    ) -> audiocpp_status;
    pub fn audiocpp_result_artifact_meta_count(result: *const audiocpp_result, index: usize) -> usize;
    pub fn audiocpp_result_artifact_meta(
        result: *const audiocpp_result,
        index: usize,
        meta_index: usize,
        out_key: *mut *const c_char,
        out_value: *mut *const c_char,
    ) -> audiocpp_status;

    // Streaming
    pub fn audiocpp_stream_policy(
        session: *const audiocpp_session,
        out_input: *mut audiocpp_stream_input_kind,
        out_output: *mut audiocpp_stream_output_kind,
        out_preferred_chunk_samples: *mut i64,
        out_preferred_chunk_seconds: *mut f64,
    ) -> audiocpp_status;
    pub fn audiocpp_stream_start(session: *mut audiocpp_session, request: *const audiocpp_request) -> audiocpp_status;
    pub fn audiocpp_stream_push(
        session: *mut audiocpp_session,
        samples: *const c_float,
        frames: usize,
        sample_rate: c_int,
        channels: c_int,
        start_sample: i64,
        out_event: *mut *mut audiocpp_event,
    ) -> audiocpp_status;
    pub fn audiocpp_stream_next_event(
        session: *mut audiocpp_session,
        out_event: *mut *mut audiocpp_event,
    ) -> audiocpp_status;
    pub fn audiocpp_stream_finish(
        session: *mut audiocpp_session,
        out_result: *mut *mut audiocpp_result,
    ) -> audiocpp_status;
    pub fn audiocpp_stream_reset(session: *mut audiocpp_session) -> audiocpp_status;
    pub fn audiocpp_event_free(event: *mut audiocpp_event);
    pub fn audiocpp_event_is_final(event: *const audiocpp_event) -> c_int;
    pub fn audiocpp_event_as_result(event: *const audiocpp_event) -> *const audiocpp_result;
    pub fn audiocpp_event_voice_activity_count(event: *const audiocpp_event) -> usize;
    pub fn audiocpp_event_voice_activity(
        event: *const audiocpp_event,
        index: usize,
        out_kind: *mut audiocpp_voice_activity_kind,
        out_sample: *mut i64,
        out_probability: *mut c_float,
    ) -> audiocpp_status;
}
