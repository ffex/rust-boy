//! The example programs: their names, their directories and the assembly they print

use std::path::{Path, PathBuf};
use std::process::Command;

/// Every example binary, in the order of the README
pub const EXAMPLES: [&str; 6] = [
    "basic_usage",
    "unbricked",
    "unbricked_std",
    "unbricked_rustboy",
    "fosdem",
    "coin-anim",
];

/// The repository's root
pub fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The directory of example `bin`: its committed assembly, `main.asm` (the snapshot
/// `tests/snapshots.rs` checks), and its assets (`.2bpp` files), if any
pub fn example_dir(bin: &str) -> PathBuf {
    root().join("examples").join(bin)
}

/// The assembly example `bin` prints: it runs the binary Cargo built for the tests
pub fn example_asm(bin: &str) -> String {
    let exe = match bin {
        "basic_usage" => env!("CARGO_BIN_EXE_basic_usage"),
        "unbricked" => env!("CARGO_BIN_EXE_unbricked"),
        "unbricked_std" => env!("CARGO_BIN_EXE_unbricked_std"),
        "unbricked_rustboy" => env!("CARGO_BIN_EXE_unbricked_rustboy"),
        "fosdem" => env!("CARGO_BIN_EXE_fosdem"),
        "coin-anim" => env!("CARGO_BIN_EXE_coin-anim"),
        other => panic!("no example binary {:?}", other),
    };
    let output = Command::new(exe)
        .current_dir(root())
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {}", exe, error));
    assert!(
        output.status.success(),
        "{} failed:\n{}",
        bin,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the assembly is UTF-8")
}
