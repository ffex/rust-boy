//! Building a ROM with RGBDS, to run it on [`GameBoy`]
//!
//! The tests that need RGBDS run only when the environment variable `RGBDS_LINK_CHECK`
//! is set (with `rgbasm`, `rgblink` and `rgbfix` on the `PATH`), as the library's link
//! checks do: [`rgbds_enabled`]. CI sets it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::examples::{example_asm, example_dir, root};
use super::gameboy::GameBoy;

/// Whether the tests that need RGBDS run: `RGBDS_LINK_CHECK` is set. When it is not, it
/// says so on stderr (shown with `--nocapture`), and the test returns early.
pub fn rgbds_enabled() -> bool {
    let enabled = std::env::var_os("RGBDS_LINK_CHECK").is_some();
    if !enabled {
        eprintln!("skipped: set RGBDS_LINK_CHECK=1 (RGBDS on the PATH) to build and run ROMs");
    }
    enabled
}

/// A ROM RGBDS built, with its symbols and its map file
pub struct Rom {
    pub bytes: Vec<u8>,
    /// The address of each label (`Scope.local` for a local one), from `rgblink -n`
    pub symbols: BTreeMap<String, u16>,
    /// The map file, from `rgblink -m`
    pub map: String,
}

impl Rom {
    /// Assemble `asm` and link it into a ROM, as `scripts/run.sh` does:
    /// `rgbasm -I include -I <dir>…`, `rgblink -n -m`, `rgbfix -v -p 0xFF`. Panics with
    /// the RGBDS errors if a step fails.
    pub fn build(asm: &str, include_dirs: &[PathBuf]) -> Rom {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "rom-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("cannot create a temporary directory");
        std::fs::write(dir.join("main.asm"), asm).expect("cannot write main.asm");

        let mut rgbasm = vec!["-I".into(), root().join("include").into_os_string()];
        for include in include_dirs {
            rgbasm.push("-I".into());
            rgbasm.push(include.clone().into_os_string());
        }
        rgbasm.extend(["-o", "main.o", "main.asm"].map(Into::into));
        let steps: [(&str, Vec<std::ffi::OsString>); 3] = [
            ("rgbasm", rgbasm),
            (
                "rgblink",
                [
                    "-n", "main.sym", "-m", "main.map", "-o", "main.gb", "main.o",
                ]
                .map(Into::into)
                .to_vec(),
            ),
            (
                "rgbfix",
                ["-v", "-p", "0xFF", "main.gb"].map(Into::into).to_vec(),
            ),
        ];
        for (tool, args) in steps {
            let output = Command::new(tool)
                .args(&args)
                .current_dir(&dir)
                .output()
                .unwrap_or_else(|error| {
                    panic!("cannot run {} (is RGBDS on the PATH?): {}", tool, error)
                });
            if !output.status.success() {
                panic!(
                    "{} failed:\n{}\n\nin {}:\n{}",
                    tool,
                    String::from_utf8_lossy(&output.stderr),
                    dir.display(),
                    asm
                );
            }
        }
        let bytes = std::fs::read(dir.join("main.gb")).expect("cannot read main.gb");
        let sym = std::fs::read_to_string(dir.join("main.sym")).expect("cannot read main.sym");
        let map = std::fs::read_to_string(dir.join("main.map")).expect("cannot read main.map");
        let _ = std::fs::remove_dir_all(&dir);
        Rom {
            bytes,
            symbols: parse_symbols(&sym),
            map,
        }
    }

    /// The ROM of example `bin`: its assembly, with `examples/<bin>/` on the include path
    /// for its assets
    pub fn example(bin: &str) -> Rom {
        Rom::build(&example_asm(bin), &[example_dir(bin)])
    }

    /// A Game Boy that runs this ROM from power-on (`$0100`)
    pub fn boot(&self) -> GameBoy {
        GameBoy::with_symbols(self.bytes.clone(), self.symbols.clone())
    }
}

/// The labels of a `.sym` file (`bank:address name` lines), by name
fn parse_symbols(sym: &str) -> BTreeMap<String, u16> {
    sym.lines()
        .filter(|line| !line.starts_with(';') && !line.trim().is_empty())
        .map(|line| {
            let (place, name) = line.split_once(' ').expect("a line `bank:address name`");
            let (_, address) = place.split_once(':').expect("`bank:address`");
            let address = u16::from_str_radix(address, 16).expect("a hexadecimal address");
            (name.trim().to_string(), address)
        })
        .collect()
}
