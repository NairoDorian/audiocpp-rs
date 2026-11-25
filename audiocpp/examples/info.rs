//! What this build of audio.cpp contains: versions, tasks, compiled families.
//!
//!   cargo run --example info --features model-tiny_voca

use audiocpp::{abi_version, build_version, tasks, AbiVersion, Registry};

fn main() -> audiocpp::Result<()> {
    println!("audio.cpp {} (C ABI {}, bindings written for {})", build_version(), abi_version(), AbiVersion::BINDINGS);
    let names: Vec<String> = tasks().iter().map(ToString::to_string).collect();
    println!("tasks: {}", names.join(" "));

    let registry = Registry::new()?;
    let families = registry.families();
    println!("{} families compiled in:", families.len());
    for family in families {
        println!("  {family}");
    }
    Ok(())
}
