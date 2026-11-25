use std::fmt;
use std::ptr::{self, NonNull};

use audiocpp_sys as sys;

use crate::error::check;
use crate::{ffi, Error, Request, Result, Stream, StreamPolicy, TaskResult};

/// A model opened for one task on one backend. Keep it alive between requests:
/// audio.cpp reuses its graphs and caches across runs of the same session.
///
/// `Send` but not `Sync`: one thread at a time. Run several in parallel by
/// opening several sessions.
pub struct Session {
    ptr: NonNull<sys::audiocpp_session>,
}

// SAFETY: see Registry.
unsafe impl Send for Session {}

impl Session {
    pub(crate) fn from_raw(ptr: NonNull<sys::audiocpp_session>) -> Self {
        Session { ptr }
    }

    /// The family of the model this session runs.
    pub fn family(&self) -> String {
        // SAFETY: valid handle; borrowed string copied at once.
        unsafe { ffi::string(sys::audiocpp_session_family(self.ptr.as_ptr())) }
    }

    /// Allocates for a request like `request` ahead of time, so the next run of
    /// a latency-sensitive request skips that cost. Optional: [`Session::run`]
    /// always prepares.
    pub fn prepare(&mut self, request: &Request) -> Result<()> {
        // SAFETY: valid handles.
        check(unsafe { sys::audiocpp_session_prepare(self.ptr.as_ptr(), request.as_ptr()) }, "audiocpp_session_prepare")
    }

    /// Runs one request to completion.
    pub fn run(&mut self, request: &Request) -> Result<TaskResult> {
        const CALL: &str = "audiocpp_session_run";
        let mut out = ptr::null_mut();
        // SAFETY: valid handles; out receives a new result.
        check(unsafe { sys::audiocpp_session_run(self.ptr.as_ptr(), request.as_ptr(), &mut out) }, CALL)?;
        NonNull::new(out).map(TaskResult::from_raw).ok_or_else(|| Error::null_handle(CALL))
    }

    /// How a streaming session wants its input (chunk size) and gives its output.
    pub fn stream_policy(&self) -> Result<StreamPolicy> {
        StreamPolicy::query(self.ptr.as_ptr())
    }

    /// Starts a stream (the session must be [`crate::Mode::Streaming`]). Any
    /// stream already in flight is reset.
    pub fn stream(&mut self, request: Option<&Request>) -> Result<Stream<'_>> {
        Stream::start(self, request)
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::audiocpp_session {
        self.ptr.as_ptr()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: we own the handle.
        unsafe { sys::audiocpp_session_free(self.ptr.as_ptr()) }
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session").field("family", &self.family()).finish()
    }
}
