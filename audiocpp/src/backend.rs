use std::fmt;

/// The compute backend a session runs on. A backend must be compiled in
/// (`vulkan`, `cuda`, `metal`, `hip` features); the CPU always is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Cpu,
    Cuda,
    /// AMD ROCm/HIP.
    Hip,
    Vulkan,
    Metal,
    WebGpu,
    Sycl,
    /// The best device audio.cpp finds (a GPU when one is compiled in and present).
    Best,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BackendKind::Cpu => "cpu",
            BackendKind::Cuda => "cuda",
            BackendKind::Hip => "hip",
            BackendKind::Vulkan => "vulkan",
            BackendKind::Metal => "metal",
            BackendKind::WebGpu => "webgpu",
            BackendKind::Sycl => "sycl",
            BackendKind::Best => "best",
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where and with how many threads a session runs.
///
/// audio.cpp's own default (no config) is the CPU with **one** thread; this
/// type's default is the CPU with [`default_threads`] threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Backend {
    pub kind: BackendKind,
    /// Device index within the backend (0 = the first).
    pub device: i32,
    /// CPU threads for the work that runs on the CPU.
    pub threads: i32,
}

impl Backend {
    pub fn new(kind: BackendKind) -> Self {
        Backend { kind, device: 0, threads: default_threads() }
    }

    pub fn cpu() -> Self {
        Backend::new(BackendKind::Cpu)
    }

    /// A GPU when one is compiled in and present, else the CPU.
    pub fn best() -> Self {
        Backend::new(BackendKind::Best)
    }

    pub fn device(mut self, device: i32) -> Self {
        self.device = device;
        self
    }

    pub fn threads(mut self, threads: i32) -> Self {
        self.threads = threads.max(1);
        self
    }
}

impl Default for Backend {
    fn default() -> Self {
        Backend::cpu()
    }
}

/// The machine's parallelism, capped at 8 (more threads rarely help a single
/// audio model, and they compete with the rest of the app).
pub fn default_threads() -> i32 {
    std::thread::available_parallelism().map(|n| n.get().min(8) as i32).unwrap_or(4)
}
