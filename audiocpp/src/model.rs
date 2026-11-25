use std::fmt;
use std::os::raw::c_int;
use std::path::Path;
use std::ptr::{self, NonNull};

use audiocpp_sys as sys;

use crate::error::check;
use crate::options::{options_ptr, OptionsHandle};
use crate::{ffi, Backend, Error, Mode, OptionInfo, OptionScope, Options, Registry, Result, Session, Task};

/// How to pick a model inside a file or package. Every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelConfig {
    /// The family to load the path as (audio.cpp's `--family`). None = identify
    /// it from the file: a GGUF written by audio.cpp's converter names its family.
    pub family_hint: Option<String>,
    /// For packages that ship several configs (`--config`).
    pub config_id: Option<String>,
    /// For packages that ship several weight sets (`--weight`).
    pub weight_id: Option<String>,
    /// A model spec file to use instead of the built-in one (`--model-spec-override`).
    pub model_spec_override: Option<String>,
}

impl ModelConfig {
    /// A config that names the family.
    pub fn family(family: impl Into<String>) -> Self {
        ModelConfig { family_hint: Some(family.into()), ..ModelConfig::default() }
    }
}

/// A loaded model: weights in memory, ready to open sessions.
///
/// Sessions keep their model alive, so a model may be dropped while its sessions
/// run. A model is `Send` but not `Sync`: to open sessions from several threads,
/// open them on one thread and move each to its worker, or guard the model with a
/// mutex.
pub struct Model {
    ptr: NonNull<sys::audiocpp_model>,
}

// SAFETY: see Registry.
unsafe impl Send for Model {}

impl Model {
    /// Loads a model file (GGUF) or package directory. `options` are load
    /// options ([`OptionScope::Load`]).
    pub fn load(registry: &Registry, path: impl AsRef<Path>, config: &ModelConfig, options: &Options) -> Result<Model> {
        const CALL: &str = "audiocpp_model_load";
        let path_c = ffi::path_cstring(CALL, path.as_ref())?;
        let family = ffi::opt_cstring(CALL, "family_hint", config.family_hint.as_deref())?;
        let config_id = ffi::opt_cstring(CALL, "config_id", config.config_id.as_deref())?;
        let weight_id = ffi::opt_cstring(CALL, "weight_id", config.weight_id.as_deref())?;
        let spec = ffi::opt_cstring(CALL, "model_spec_override", config.model_spec_override.as_deref())?;
        let raw_config = sys::audiocpp_model_config {
            family_hint: ffi::opt_ptr(&family),
            config_id: ffi::opt_ptr(&config_id),
            weight_id: ffi::opt_ptr(&weight_id),
            model_spec_override: ffi::opt_ptr(&spec),
        };
        let options = OptionsHandle::new(options)?;
        let mut out = ptr::null_mut();
        // SAFETY: every pointer is valid for the call; out receives a new handle.
        check(
            unsafe {
                sys::audiocpp_model_load(
                    registry.as_ptr(),
                    path_c.as_ptr(),
                    &raw_config,
                    options_ptr(&options),
                    &mut out,
                )
            },
            CALL,
        )?;
        NonNull::new(out).map(|ptr| Model { ptr }).ok_or_else(|| Error::null_handle(CALL))
    }

    /// The family audio.cpp loaded the model as.
    pub fn family(&self) -> String {
        // SAFETY: valid handle; borrowed string copied at once.
        unsafe { ffi::string(sys::audiocpp_model_family(self.ptr.as_ptr())) }
    }

    pub fn description(&self) -> String {
        unsafe { ffi::string(sys::audiocpp_model_description(self.ptr.as_ptr())) }
    }

    /// Whether the model can run `task` in `mode`. (An unknown task token also
    /// answers false; compare with [`crate::tasks`] when that matters.)
    pub fn supports(&self, task: &Task, mode: Mode) -> bool {
        let (Ok(task), Ok(mode)) = (
            ffi::cstring("audiocpp_model_supports", "task", task.as_str()),
            ffi::cstring("audiocpp_model_supports", "mode", mode.as_str()),
        ) else {
            return false;
        };
        // SAFETY: valid handle and strings.
        unsafe { sys::audiocpp_model_supports(self.ptr.as_ptr(), task.as_ptr(), mode.as_ptr()) == 1 }
    }

    pub fn supports_timestamps(&self) -> bool {
        unsafe { sys::audiocpp_model_supports_timestamps(self.ptr.as_ptr()) == 1 }
    }

    /// Whether the model takes a reference voice ([`crate::Request::voice_audio`]).
    pub fn supports_speaker_reference(&self) -> bool {
        unsafe { sys::audiocpp_model_supports_speaker_reference(self.ptr.as_ptr()) == 1 }
    }

    /// Whether the model takes style conditioning (emotion, rate, pitch, ...).
    pub fn supports_style_condition(&self) -> bool {
        unsafe { sys::audiocpp_model_supports_style_condition(self.ptr.as_ptr()) == 1 }
    }

    /// The languages the model declares.
    pub fn languages(&self) -> Vec<String> {
        let n = unsafe { sys::audiocpp_model_language_count(self.ptr.as_ptr()) };
        (0..n)
            .filter_map(|i| {
                let mut lang = ptr::null();
                let st = unsafe { sys::audiocpp_model_language(self.ptr.as_ptr(), i, &mut lang) };
                (st == sys::AUDIOCPP_OK).then(|| unsafe { ffi::string(lang) })
            })
            .collect()
    }

    /// The options the model declares in one scope, straight from its spec.
    pub fn options(&self, scope: OptionScope) -> Vec<OptionInfo> {
        let n = unsafe { sys::audiocpp_model_option_count(self.ptr.as_ptr(), scope.raw()) };
        (0..n)
            .filter_map(|i| {
                let (mut name, mut value_name, mut description) = (ptr::null(), ptr::null(), ptr::null());
                let (mut default_value, mut min_value, mut max_value) = (ptr::null(), ptr::null(), ptr::null());
                let mut required: c_int = 0;
                // SAFETY: valid handle; every out pointer is valid; strings are borrowed.
                let st = unsafe {
                    sys::audiocpp_model_option(
                        self.ptr.as_ptr(),
                        scope.raw(),
                        i,
                        &mut name,
                        &mut value_name,
                        &mut description,
                        &mut default_value,
                        &mut min_value,
                        &mut max_value,
                        &mut required,
                    )
                };
                (st == sys::AUDIOCPP_OK).then(|| unsafe {
                    OptionInfo {
                        name: ffi::string(name),
                        value_name: ffi::string(value_name),
                        description: ffi::string(description),
                        default_value: ffi::string(default_value),
                        min_value: ffi::string(min_value),
                        max_value: ffi::string(max_value),
                        required: required == 1,
                    }
                })
            })
            .collect()
    }

    /// Opens a session for `task` in `mode` on `backend`. `options` are session
    /// options ([`OptionScope::Session`]).
    pub fn session(&self, task: &Task, mode: Mode, backend: &Backend, options: &Options) -> Result<Session> {
        self.open_session(task, mode, backend, None, options)
    }

    /// Like [`Model::session`], with an explicit backend implementation (as the
    /// family names them; empty = default).
    pub fn session_with_impl(
        &self,
        task: &Task,
        mode: Mode,
        backend: &Backend,
        implementation: &str,
        options: &Options,
    ) -> Result<Session> {
        self.open_session(task, mode, backend, Some(implementation), options)
    }

    fn open_session(
        &self,
        task: &Task,
        mode: Mode,
        backend: &Backend,
        implementation: Option<&str>,
        options: &Options,
    ) -> Result<Session> {
        const CALL: &str = "audiocpp_session_create";
        let task_c = ffi::cstring(CALL, "task", task.as_str())?;
        let mode_c = ffi::cstring(CALL, "mode", mode.as_str())?;
        let backend_c = ffi::cstring(CALL, "backend", backend.kind.as_str())?;
        let impl_c = ffi::opt_cstring(CALL, "implementation", implementation)?;
        let config = sys::audiocpp_backend_config {
            backend: backend_c.as_ptr(),
            device: backend.device,
            threads: backend.threads.max(1),
        };
        let options = OptionsHandle::new(options)?;
        let mut out = ptr::null_mut();
        // SAFETY: every pointer is valid for the call; out receives a new handle.
        let status = unsafe {
            match &impl_c {
                Some(i) => sys::audiocpp_session_create_with_impl(
                    self.ptr.as_ptr(),
                    task_c.as_ptr(),
                    mode_c.as_ptr(),
                    &config,
                    i.as_ptr(),
                    options_ptr(&options),
                    &mut out,
                ),
                None => sys::audiocpp_session_create(
                    self.ptr.as_ptr(),
                    task_c.as_ptr(),
                    mode_c.as_ptr(),
                    &config,
                    options_ptr(&options),
                    &mut out,
                ),
            }
        };
        check(status, CALL)?;
        NonNull::new(out).map(Session::from_raw).ok_or_else(|| Error::null_handle(CALL))
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: we own the handle; sessions keep their own reference.
        unsafe { sys::audiocpp_model_free(self.ptr.as_ptr()) }
    }
}

impl fmt::Debug for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Model").field("family", &self.family()).finish()
    }
}
