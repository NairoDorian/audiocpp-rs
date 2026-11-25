use std::fmt;
use std::sync::OnceLock;

use audiocpp_sys as sys;

use crate::{Error, Result, Status};

/// `(major, minor, patch)` of audio.cpp's C ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl AbiVersion {
    /// The ABI these bindings were written against.
    pub const BINDINGS: AbiVersion = AbiVersion {
        major: sys::AUDIOCPP_ABI_VERSION_MAJOR,
        minor: sys::AUDIOCPP_ABI_VERSION_MINOR,
        patch: sys::AUDIOCPP_ABI_VERSION_PATCH,
    };
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The ABI of the audio.cpp library actually loaded.
pub fn abi_version() -> AbiVersion {
    // SAFETY: no arguments, no state.
    let v = unsafe { sys::audiocpp_abi_version() };
    AbiVersion { major: v >> 16, minor: (v >> 8) & 0xff, patch: v & 0xff }
}

/// audio.cpp's build version, e.g. "v0.9.6".
pub fn build_version() -> String {
    // SAFETY: a static string.
    unsafe { crate::ffi::string(sys::audiocpp_build_version()) }
}

/// Fails once and for all when the loaded library cannot serve these bindings:
/// another major, or an older minor (a function they declare would be missing).
pub(crate) fn ensure_compatible() -> Result<()> {
    static CHECK: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    CHECK
        .get_or_init(|| {
            let lib = abi_version();
            let want = AbiVersion::BINDINGS;
            if lib.major != want.major || lib.minor < want.minor {
                Err(format!(
                    "the loaded audio.cpp library has C ABI {lib}; these bindings need {}.{}.x or a later minor",
                    want.major, want.minor
                ))
            } else {
                Ok(())
            }
        })
        .clone()
        .map_err(|message| Error::new(Status::AbiMismatch, "audiocpp_abi_version", message))
}
