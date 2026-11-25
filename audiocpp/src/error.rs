use std::fmt;

use audiocpp_sys as sys;

/// What kind of failure a call reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Status {
    InvalidArgument,
    /// The model's family is not compiled into this library (enable its `model-*` feature).
    UnsupportedFamily,
    LoadFailed,
    Runtime,
    OutOfMemory,
    OutOfRange,
    /// The result has no output of that kind, or the model cannot do that.
    NotAvailable,
    /// The loaded library's ABI does not match these bindings.
    AbiMismatch,
    /// A status code this crate does not know (a newer library).
    Unknown(i32),
}

impl Status {
    pub(crate) fn from_raw(code: sys::audiocpp_status) -> Self {
        match code {
            sys::AUDIOCPP_ERR_INVALID_ARGUMENT => Status::InvalidArgument,
            sys::AUDIOCPP_ERR_UNSUPPORTED_FAMILY => Status::UnsupportedFamily,
            sys::AUDIOCPP_ERR_LOAD_FAILED => Status::LoadFailed,
            sys::AUDIOCPP_ERR_RUNTIME => Status::Runtime,
            sys::AUDIOCPP_ERR_OUT_OF_MEMORY => Status::OutOfMemory,
            sys::AUDIOCPP_ERR_OUT_OF_RANGE => Status::OutOfRange,
            sys::AUDIOCPP_ERR_NOT_AVAILABLE => Status::NotAvailable,
            other => Status::Unknown(other),
        }
    }

    fn describe(self) -> String {
        match self {
            Status::InvalidArgument => "invalid argument".into(),
            Status::UnsupportedFamily => "unsupported family".into(),
            Status::LoadFailed => "load failed".into(),
            Status::Runtime => "runtime error".into(),
            Status::OutOfMemory => "out of memory".into(),
            Status::OutOfRange => "out of range".into(),
            Status::NotAvailable => "not available".into(),
            Status::AbiMismatch => "ABI mismatch".into(),
            Status::Unknown(code) => format!("unknown status {code}"),
        }
    }
}

/// An audio.cpp error: the status, the C call that failed, and audio.cpp's own
/// explanation (`audiocpp_last_error()`, read right after the failing call).
#[derive(Debug, Clone)]
pub struct Error {
    status: Status,
    call: &'static str,
    message: String,
}

impl Error {
    /// The error of a failing C call. `audiocpp_last_error()` is per thread and
    /// a later call may clear it, so it is read here, at once.
    pub(crate) fn from_call(code: sys::audiocpp_status, call: &'static str) -> Self {
        // SAFETY: audiocpp_last_error never returns a dangling pointer; the string
        // is copied before any other call on this thread.
        let message = unsafe { crate::ffi::string(sys::audiocpp_last_error()) };
        Error { status: Status::from_raw(code), call, message }
    }

    pub(crate) fn new(status: Status, call: &'static str, message: impl Into<String>) -> Self {
        Error { status, call, message: message.into() }
    }

    pub(crate) fn invalid(call: &'static str, message: impl Into<String>) -> Self {
        Error::new(Status::InvalidArgument, call, message)
    }

    pub(crate) fn null_handle(call: &'static str) -> Self {
        Error::new(Status::Runtime, call, "the call succeeded but returned no handle")
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// The C function (or crate operation) that failed.
    pub fn call(&self) -> &str {
        self.call
    }

    /// audio.cpp's explanation; may be empty.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.call, self.status.describe())?;
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn check(code: sys::audiocpp_status, call: &'static str) -> Result<()> {
    if code == sys::AUDIOCPP_OK {
        Ok(())
    } else {
        Err(Error::from_call(code, call))
    }
}
