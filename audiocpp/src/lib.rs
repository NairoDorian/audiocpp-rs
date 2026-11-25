//! Safe Rust bindings to [audio.cpp](https://github.com/0xShug0/audio.cpp): local
//! text-to-speech, voice cloning, speech recognition and many other audio models
//! on ggml, in-process.
//!
//! The crate mirrors audio.cpp's C ABI (`audiocpp.h`) one to one, with Rust
//! ownership on top. Which model families the library contains is chosen at build
//! time with `model-<family>` features (`docs/model-features.md`); the library is
//! built from a pinned audio.cpp release by `audiocpp-sys`.
//!
//! ```no_run
//! use audiocpp::{Backend, Mode, Model, ModelConfig, Options, Registry, Request, ResultData, Task};
//!
//! # fn main() -> audiocpp::Result<()> {
//! let registry = Registry::new()?;
//! let model = Model::load(&registry, "models/tiny-voca-GGUF/tiny-voca-orig.gguf",
//!                         &ModelConfig::family("tiny_voca"), &Options::new())?;
//! let mut tts = model.session(&Task::Tts, Mode::Offline, &Backend::cpu(), &Options::new())?;
//!
//! let mut request = Request::new();
//! request.text("Hello from Rust.")?.voice_id("M2")?;
//! let result = tts.run(&request)?;
//! let audio = result.audio().expect("a TTS result carries audio");
//! println!("{} samples at {} Hz", audio.samples.len(), audio.sample_rate);
//! # Ok(()) }
//! ```
//!
//! # Rules the types encode
//! - **Ownership.** Each type owns one C handle and frees it on drop. Handles
//!   keep their parents alive (a session its model, a model its registry), so
//!   drop order does not matter.
//! - **Threads.** Every handle is `Send` and not `Sync`, the C API's rule: one
//!   thread at a time per handle, separate handles in parallel.
//! - **Borrowed audio.** [`Audio`] borrows a result's samples (no copy); text,
//!   segments and artifacts are copied out.
//! - **Errors.** [`Error`] carries the status, the failing C call and
//!   audio.cpp's own message, read at once on the failing thread.
//! - **ABI.** [`Registry::new`] checks, once, that the loaded library's ABI
//!   matches these bindings.
//!
//! # Living next to other ggml libraries
//! The library is a shared library that exports only `audiocpp_*` and keeps its
//! ggml private, so it coexists with `transcribe-cpp`, llama.cpp and others in
//! one process. See the repository's `docs/coexistence.md`.

mod backend;
mod error;
mod ffi;
pub mod logging;
mod model;
mod options;
mod registry;
mod request;
mod result;
mod session;
mod stream;
mod task;
mod version;

pub use backend::{default_threads, Backend, BackendKind};
pub use error::{Error, Result, Status};
pub use model::{Model, ModelConfig};
pub use options::{OptionInfo, OptionScope, Options};
pub use registry::Registry;
pub use request::Request;
pub use result::{
    Artifact, ArtifactKind, Audio, ResultData, ResultView, Segment, SpeakerTurn, TaskResult, Transcript, Word,
};
pub use session::Session;
pub use stream::{Event, Stream, StreamInput, StreamOutput, StreamPolicy, VoiceActivity, VoiceActivityKind};
pub use task::{tasks, Mode, Task};
pub use version::{abi_version, build_version, AbiVersion};

/// The raw C ABI, for anything the safe API does not cover.
pub use audiocpp_sys as sys;
