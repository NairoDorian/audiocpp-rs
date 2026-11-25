use std::fmt;
use std::path::Path;
use std::ptr::{self, NonNull};

use audiocpp_sys as sys;

use crate::error::check;
use crate::version::ensure_compatible;
use crate::{ffi, Error, Result};

/// The set of model loaders compiled into this library: one per family selected
/// by the `model-*` features. Every model is loaded through a registry.
///
/// A registry may be freed before the models loaded from it: they keep it alive.
pub struct Registry {
    ptr: NonNull<sys::audiocpp_registry>,
}

// SAFETY: audio.cpp handles may move between threads; a handle is not used
// from two threads at once (no Sync), which is the C API's rule.
unsafe impl Send for Registry {}

impl Registry {
    /// The default loader set. Also checks, once, that the loaded audio.cpp
    /// library's ABI matches these bindings.
    pub fn new() -> Result<Self> {
        Registry::create(None)
    }

    /// A registry configured from an audio.cpp registry config file.
    pub fn with_config(path: impl AsRef<Path>) -> Result<Self> {
        Registry::create(Some(path.as_ref()))
    }

    fn create(config: Option<&Path>) -> Result<Self> {
        ensure_compatible()?;
        let config = config.map(|p| ffi::path_cstring("audiocpp_registry_create", p)).transpose()?;
        let mut out = ptr::null_mut();
        // SAFETY: valid (or NULL) path; out receives a new handle.
        check(
            unsafe { sys::audiocpp_registry_create(config.as_ref().map_or(ptr::null(), |c| c.as_ptr()), &mut out) },
            "audiocpp_registry_create",
        )?;
        NonNull::new(out).map(|ptr| Registry { ptr }).ok_or_else(|| Error::null_handle("audiocpp_registry_create"))
    }

    /// The families this library can load, as audio.cpp names them.
    pub fn families(&self) -> Vec<String> {
        // SAFETY: valid handle; the strings are borrowed and copied at once.
        let n = unsafe { sys::audiocpp_registry_family_count(self.ptr.as_ptr()) };
        (0..n)
            .filter_map(|i| {
                let mut name = ptr::null();
                let st = unsafe { sys::audiocpp_registry_family(self.ptr.as_ptr(), i, &mut name) };
                (st == sys::AUDIOCPP_OK).then(|| unsafe { ffi::string(name) })
            })
            .collect()
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::audiocpp_registry {
        self.ptr.as_ptr()
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        // SAFETY: we own the handle.
        unsafe { sys::audiocpp_registry_free(self.ptr.as_ptr()) }
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry").field("families", &self.families()).finish()
    }
}
