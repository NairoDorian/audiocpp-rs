//! audio.cpp's log lines (its own and ggml's).
//!
//! By default they go to stderr. [`set_log_callback`] routes them to your code,
//! [`route_to_log`] (feature `log`) to the `log` crate under target `audiocpp`,
//! [`silence`] drops them and [`reset`] restores stderr. The setting is
//! process-wide, like audio.cpp's own `audiocpp_log_set`.

use std::cell::Cell;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use audiocpp_sys as sys;

/// The severity of a log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

type Callback = Arc<dyn Fn(LogLevel, &str) + Send + Sync>;

static CALLBACK: Mutex<Option<Callback>> = Mutex::new(None);

thread_local! {
    // ggml continues a line with a CONT fragment: it keeps the previous level.
    static LAST_LEVEL: Cell<LogLevel> = const { Cell::new(LogLevel::Info) };
}

/// Sends every audio.cpp and ggml log line to `callback`. It may be called from
/// audio.cpp's worker threads; `text` may end with a newline. A panic inside it
/// is caught (it never unwinds into C).
pub fn set_log_callback<F>(callback: F)
where
    F: Fn(LogLevel, &str) + Send + Sync + 'static,
{
    *CALLBACK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(callback));
    // SAFETY: `trampoline` is a valid callback for the whole process lifetime.
    unsafe { sys::audiocpp_log_set(Some(trampoline), std::ptr::null_mut()) }
}

/// Drops every log line.
pub fn silence() {
    set_log_callback(|_, _| {});
}

/// Restores audio.cpp's default: log lines go to stderr.
pub fn reset() {
    // SAFETY: NULL restores the default sink.
    unsafe { sys::audiocpp_log_set(None, std::ptr::null_mut()) }
    *CALLBACK.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Routes the log lines to the `log` crate, target `audiocpp`.
#[cfg(feature = "log")]
pub fn route_to_log() {
    set_log_callback(|level, text| {
        let text = text.trim_end();
        if text.is_empty() {
            return;
        }
        match level {
            LogLevel::Debug => log::debug!(target: "audiocpp", "{text}"),
            LogLevel::Info => log::info!(target: "audiocpp", "{text}"),
            LogLevel::Warn => log::warn!(target: "audiocpp", "{text}"),
            LogLevel::Error => log::error!(target: "audiocpp", "{text}"),
        }
    });
}

unsafe extern "C" fn trampoline(level: sys::audiocpp_log_level, text: *const c_char, _user_data: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let level = match level {
            sys::AUDIOCPP_LOG_LEVEL_NONE => return,
            sys::AUDIOCPP_LOG_LEVEL_CONT => LAST_LEVEL.with(Cell::get),
            sys::AUDIOCPP_LOG_LEVEL_DEBUG => LogLevel::Debug,
            sys::AUDIOCPP_LOG_LEVEL_WARN => LogLevel::Warn,
            sys::AUDIOCPP_LOG_LEVEL_ERROR => LogLevel::Error,
            _ => LogLevel::Info,
        };
        LAST_LEVEL.with(|l| l.set(level));
        let text = if text.is_null() {
            String::new()
        } else {
            // SAFETY: audio.cpp passes a NUL-terminated string valid for this call.
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
        };
        // Clone the callback out so the lock is not held while it runs.
        let callback = CALLBACK.lock().ok().and_then(|guard| guard.clone());
        if let Some(callback) = callback {
            callback(level, &text);
        }
    }));
}
