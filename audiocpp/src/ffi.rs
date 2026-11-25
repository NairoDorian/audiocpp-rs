//! Small helpers for crossing the C boundary.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

use crate::{Error, Result};

/// Copies a borrowed C string ("" for NULL; invalid UTF-8 is replaced).
///
/// # Safety
/// `p` must be NULL or point to a NUL-terminated string valid for this call.
pub(crate) unsafe fn string(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

pub(crate) fn cstring(call: &'static str, what: &str, value: &str) -> Result<CString> {
    CString::new(value).map_err(|_| Error::invalid(call, format!("{what} contains a NUL byte")))
}

pub(crate) fn opt_cstring(call: &'static str, what: &str, value: Option<&str>) -> Result<Option<CString>> {
    value.map(|v| cstring(call, what, v)).transpose()
}

pub(crate) fn opt_ptr(value: &Option<CString>) -> *const c_char {
    value.as_ref().map_or(ptr::null(), |c| c.as_ptr())
}

/// audio.cpp takes UTF-8 paths. (On Windows a non-ASCII path also needs the
/// host executable to run with the UTF-8 code page: see docs/api.md.)
pub(crate) fn path_cstring(call: &'static str, path: &Path) -> Result<CString> {
    let s =
        path.to_str().ok_or_else(|| Error::invalid(call, format!("path is not valid UTF-8: {}", path.display())))?;
    cstring(call, "path", s)
}
