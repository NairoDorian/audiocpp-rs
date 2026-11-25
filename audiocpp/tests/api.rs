//! The safe API's contract, without any model file.

use audiocpp::{
    abi_version, build_version, tasks, AbiVersion, Backend, BackendKind, Model, ModelConfig, Options, Registry,
    Request, Status, Task,
};

#[test]
fn loaded_library_matches_the_bindings() {
    let lib = abi_version();
    assert_eq!(lib.major, AbiVersion::BINDINGS.major, "library ABI {lib}");
    assert!(lib.minor >= AbiVersion::BINDINGS.minor, "library ABI {lib}");
    assert!(!build_version().is_empty());
}

#[test]
fn task_vocabulary() {
    let all = tasks();
    assert!(all.contains(&Task::Tts), "{all:?}");
    assert!(all.contains(&Task::Asr), "{all:?}");
    for task in &all {
        assert_eq!(&Task::from_token(task.as_str()), task, "tokens round-trip");
    }
    assert_eq!(Task::from_token("clon"), Task::VoiceClone);
    assert_eq!(Task::from_spec_name("music"), Some(Task::Generation));
    assert_eq!(Task::from_spec_name("no-such-task"), None);
}

#[test]
fn registry_lists_the_compiled_families() {
    let registry = Registry::new().expect("registry");
    let families = registry.families();
    println!("compiled families: {families:?}");
    #[cfg(feature = "model-tiny_voca")]
    assert!(families.iter().any(|f| f == "tiny_voca"), "{families:?}");
    #[cfg(feature = "model-sense_asr")]
    assert!(!families.is_empty());
}

#[test]
fn loading_a_missing_file_fails_with_a_message() {
    let registry = Registry::new().expect("registry");
    let err = Model::load(&registry, "this/file/does/not/exist.gguf", &ModelConfig::default(), &Options::new())
        .expect_err("a missing file cannot load");
    assert_eq!(err.call(), "audiocpp_model_load");
    assert!(!err.to_string().is_empty());
    println!("{err}");
}

#[test]
fn request_setters_validate_before_calling_c() {
    let mut request = Request::new();
    assert!(request.text("hello").is_ok());
    assert_eq!(request.text("nul\0inside").unwrap_err().status(), Status::InvalidArgument);
    assert_eq!(request.audio(&[0.0; 3], 16_000, 2).unwrap_err().status(), Status::InvalidArgument);
    assert_eq!(request.audio(&[0.0; 4], 0, 1).unwrap_err().status(), Status::InvalidArgument);
    assert!(request.audio(&[0.0; 4], 16_000, 2).is_ok());
    assert!(request.option("speed", 1.25).is_ok());
    assert!(request.option_array("phonemes", &["a", "b"]).is_ok());
    assert!(request.seed(7).is_ok());
}

#[test]
fn backend_defaults() {
    let cpu = Backend::default();
    assert_eq!(cpu.kind, BackendKind::Cpu);
    assert!(cpu.threads >= 1);
    assert_eq!(Backend::best().device(1).threads(0).threads, 1);
    assert_eq!(BackendKind::Vulkan.as_str(), "vulkan");
}

#[test]
fn options_build_in_order() {
    let options = Options::new().with("a", 1).with("b", "two");
    let pairs: Vec<(&str, &str)> = options.iter().collect();
    assert_eq!(pairs, vec![("a", "1"), ("b", "two")]);
    let collected: Options = [("x", "1")].into_iter().collect();
    assert!(!collected.is_empty());
}
