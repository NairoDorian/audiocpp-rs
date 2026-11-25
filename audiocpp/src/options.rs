use std::ptr::{self, NonNull};

use audiocpp_sys as sys;

use crate::error::check;
use crate::ffi;
use crate::{Error, Result};

/// Options for a load, a session or a request: audio.cpp's option maps are
/// string → string, and each model declares the ones it accepts
/// ([`crate::Model::options`]). Applied in order; a later key wins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options(Vec<(String, String)>);

impl Options {
    pub fn new() -> Self {
        Options(Vec::new())
    }

    /// Adds `key = value` and returns the options (builder style).
    pub fn with(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.0.push((key.into(), value.to_string()));
        self
    }

    pub fn push(&mut self, key: impl Into<String>, value: impl ToString) {
        self.0.push((key.into(), value.to_string()));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl<K: Into<String>, V: ToString> FromIterator<(K, V)> for Options {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Options(iter.into_iter().map(|(k, v)| (k.into(), v.to_string())).collect())
    }
}

/// An `audiocpp_options` handle filled from [`Options`]; None for no options
/// (the C API then gets NULL).
pub(crate) struct OptionsHandle(NonNull<sys::audiocpp_options>);

impl OptionsHandle {
    pub(crate) fn new(options: &Options) -> Result<Option<OptionsHandle>> {
        if options.is_empty() {
            return Ok(None);
        }
        // SAFETY: returns a new handle or NULL.
        let raw = unsafe { sys::audiocpp_options_create() };
        let handle = OptionsHandle(NonNull::new(raw).ok_or_else(|| Error::null_handle("audiocpp_options_create"))?);
        for (key, value) in options.iter() {
            let k = ffi::cstring("audiocpp_options_set", "option key", key)?;
            let v = ffi::cstring("audiocpp_options_set", "option value", value)?;
            // SAFETY: valid handle and NUL-terminated strings, copied by the call.
            check(
                unsafe { sys::audiocpp_options_set(handle.0.as_ptr(), k.as_ptr(), v.as_ptr()) },
                "audiocpp_options_set",
            )?;
        }
        Ok(Some(handle))
    }
}

pub(crate) fn options_ptr(handle: &Option<OptionsHandle>) -> *const sys::audiocpp_options {
    handle.as_ref().map_or(ptr::null(), |h| h.0.as_ptr())
}

impl Drop for OptionsHandle {
    fn drop(&mut self) {
        // SAFETY: we own the handle.
        unsafe { sys::audiocpp_options_free(self.0.as_ptr()) }
    }
}

/// Which option map an option belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionScope {
    /// Per request ([`crate::Request::option`]).
    Request,
    /// Per session ([`crate::Model::session`]'s options).
    Session,
    /// At load time ([`crate::Model::load`]'s options).
    Load,
}

impl OptionScope {
    pub(crate) fn raw(self) -> sys::audiocpp_option_scope {
        match self {
            OptionScope::Request => sys::AUDIOCPP_OPTION_SCOPE_REQUEST,
            OptionScope::Session => sys::AUDIOCPP_OPTION_SCOPE_SESSION,
            OptionScope::Load => sys::AUDIOCPP_OPTION_SCOPE_LOAD,
        }
    }
}

/// One option a model declares: enough to build a settings UI without
/// hardcoding anything per family. Bounds are hints, not enforced here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionInfo {
    pub name: String,
    /// The value's placeholder name, e.g. "seconds".
    pub value_name: String,
    pub description: String,
    pub default_value: String,
    pub min_value: String,
    pub max_value: String,
    pub required: bool,
}
