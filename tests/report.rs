//! On-demand ROM size and frame-budget report.
//!
//! Run through `scripts/report.sh`. Without `RGBDS_LINK_CHECK` the live report returns
//! early, like the emulator tests; CI sets it in the RGBDS job.

mod support;

use support::gameboy::{FRAME_MCYCLES, GameBoy};
use support::rom::{Rom, rgbds_enabled};

const VBLANK_MCYCLES: u64 = 10 * 114;
const EXAMPLES: [&str; 8] = [
    "basic_usage",
    "unbricked",
    "unbricked_std",
    "unbricked_rustboy",
    "fosdem",
    "coin-anim",
    "music",
    "olive-empire",
];

fn rom_used_bytes(map: &str) -> usize {
    let mut rom = false;
    map.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.ends_with(':') && trimmed.contains("bank #") {
                rom = trimmed.starts_with("ROM0 bank #") || trimmed.starts_with("ROMX bank #");
                return None;
            }
            if !rom || !trimmed.starts_with("SECTION: ") {
                return None;
            }
            let (_, size) = trimmed.rsplit_once("($")?;
            let (hex, _) = size.split_once(" bytes)")?;
            usize::from_str_radix(hex, 16).ok()
        })
        .sum()
}

fn measure_vblank_handler(gb: &mut GameBoy, entry: u16, end_opcode: u8, runs: usize) -> Vec<u64> {
    let mut result = Vec::with_capacity(runs);
    for _ in 0..runs {
        assert!(gb.run_until_pc(entry, 2), "${entry:04X} did not run");
        let start = gb.cycles();
        while gb.read(gb.registers().pc) != end_opcode {
            gb.step();
        }
        gb.step();
        // Entry at VBlankHandler is after the CPU's 5-M-cycle interrupt dispatch
        // and the 4-M-cycle JP at the $40 vector. Include both in the VBlank share.
        result.push(gb.cycles() - start + 9);
    }
    result
}

fn range(values: &[u64]) -> (u64, u64) {
    (*values.iter().min().unwrap(), *values.iter().max().unwrap())
}

fn report(bin: &str) {
    let rom = Rom::example(bin);
    let used = rom_used_bytes(&rom.map);
    println!("{bin}");
    println!(
        "  ROM: {used} / {} bytes ({:.1}%)",
        rom.bytes.len(),
        used as f64 * 100.0 / rom.bytes.len() as f64
    );

    if let Some(&handler) = rom.symbols.get("VBlankHandler") {
        let mut gb = rom.boot();
        gb.run_frames(5);
        let samples = measure_vblank_handler(&mut gb, handler, 0xD9, 10);
        let (min, max) = range(&samples);
        println!(
            "  VBlank handler (including dispatch): {min}..{max} / {VBLANK_MCYCLES} M-cycles ({:.1}% max)",
            max as f64 * 100.0 / VBLANK_MCYCLES as f64
        );
    } else {
        println!("  VBlank handler: n/a (no VBlankHandler symbol)");
    }

    if let (Some(&main), Some(_)) = (rom.symbols.get("Main"), rom.symbols.get("WaitFrame")) {
        let mut gb = rom.boot();
        // Engine programs begin Main with the three-byte `call WaitFrame`. Measuring from
        // its return address to the next Main excludes sleeping and includes game logic.
        assert!(
            gb.run_until_pc(main.wrapping_add(3), 10),
            "WaitFrame did not return"
        );
        let mut samples = Vec::new();
        for _ in 0..10 {
            let start = gb.cycles();
            assert!(gb.run_until_pc(main, 2), "Main did not repeat");
            samples.push(gb.cycles() - start);
            assert!(
                gb.run_until_pc(main.wrapping_add(3), 2),
                "WaitFrame did not return"
            );
        }
        let (min, max) = range(&samples);
        println!(
            "  Main-loop work: {min}..{max} / {FRAME_MCYCLES} M-cycles ({:.1}% max)",
            max as f64 * 100.0 / FRAME_MCYCLES as f64
        );
    } else {
        println!("  Main-loop work: n/a (not an interrupt-driven engine program)");
    }
}

#[test]
fn rom_size_and_cycle_budget() {
    if !rgbds_enabled() {
        return;
    }
    match std::env::var("REPORT_BIN") {
        Ok(bin) => {
            assert!(
                EXAMPLES.contains(&bin.as_str()),
                "unknown example binary {bin}"
            );
            report(&bin);
        }
        Err(_) => {
            for bin in EXAMPLES {
                report(bin);
            }
        }
    }
}

#[test]
fn parses_rom_section_sizes() {
    let map = r#"
ROM0 bank #0:
    SECTION: $0100-$0103 ($0004 bytes) ["Header"]
    SECTION: $0150-$015F ($0010 bytes) ["Main"]
    SLACK: $3EAC bytes
WRAM0 bank #0:
    SECTION: $C000-$C0FF ($0100 bytes) ["Variables"]
"#;
    assert_eq!(rom_used_bytes(map), 20);
}

#[test]
fn empty_vblank_handler_budget_includes_dispatch_and_vector() {
    if !rgbds_enabled() {
        return;
    }
    let game = rust_boy::engine::RustBoy::new();
    let rom = Rom::build(&game.build().unwrap(), &[]);
    let handler = rom.symbols["VBlankHandler"];
    let mut gb = rom.boot();
    gb.run_frames(5);
    // push af (4), ld a,1 (2), ld [wVBlankFlag],a (4), pop af (3),
    // reti (4), CPU interrupt dispatch (5), and vector JP (4).
    assert_eq!(measure_vblank_handler(&mut gb, handler, 0xD9, 3), [26; 3]);
}
