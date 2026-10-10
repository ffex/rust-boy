//! Tests of the `hw` module: its symbols against `include/hardware.inc` (names always,
//! values with RGBDS under `RGBDS_LINK_CHECK`), its layout, and the guard that keeps
//! hardware names and addresses out of strings in the other layers.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::*;
use crate::asm::label_check::{rgbds_accepts, rgbds_rom};

const HARDWARE_INC: &str = include_str!("../../include/hardware.inc");

/// Every name `hardware.inc` defines (`DEF name ...`)
fn hardware_inc_names() -> BTreeSet<&'static str> {
    HARDWARE_INC
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("DEF "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect()
}

#[test]
fn test_every_symbol_is_a_hardware_inc_name() {
    let defined = hardware_inc_names();
    let missing: Vec<&str> = SYMBOLS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !defined.contains(name))
        .collect();
    assert!(missing.is_empty(), "not in hardware.inc: {:?}", missing);

    let unique: BTreeSet<&str> = SYMBOLS.iter().map(|(name, _)| *name).collect();
    assert_eq!(unique.len(), SYMBOLS.len(), "a name is listed twice");
}

/// The program that asserts, with RGBDS, that each (name, value) is the value
/// `hardware.inc` gives the name
fn assert_values(symbols: &[(&str, u16)]) -> String {
    let mut asm = String::from("INCLUDE \"hardware.inc\"\n");
    for (name, value) in symbols {
        asm.push_str(&format!(
            "ASSERT {0} == ${1:04X}, \"hw gives {0} the value ${1:04X}\"\n",
            name, value
        ));
    }
    asm.push_str("SECTION \"Main\", ROM0\nnop\n");
    asm
}

/// Each symbol's value is the one `hardware.inc` gives it: a file that includes it
/// asserts `name == value` for every symbol, and RGBDS assembles it (with
/// `RGBDS_LINK_CHECK`; the names alone are checked without RGBDS above)
#[test]
fn test_every_symbol_has_its_hardware_inc_value() {
    assert!(SYMBOLS.len() > 100, "{} symbols", SYMBOLS.len());
    // Panics with the failed assertions, if any
    let Some(_) = rgbds_rom(&assert_values(SYMBOLS)) else {
        return; // RGBDS_LINK_CHECK not set
    };
    // The check is not vacuous: one wrong value, and RGBDS rejects the file
    for wrong in [
        ("rLCDC", 0xFF41),
        ("LCDCF_OBJ16", 0x02),
        ("sizeof_OAM_ATTRS", 5),
    ] {
        assert_eq!(
            rgbds_accepts(&assert_values(&[wrong])),
            Some(false),
            "{:?} was accepted",
            wrong
        );
    }
}

#[test]
fn test_oam_offset() {
    assert_eq!(oam_offset(0, OAMA_Y), 0);
    assert_eq!(oam_offset(1, OAMA_X), 5);
    assert_eq!(oam_offset(2, OAMA_FLAGS), 11);
    assert_eq!(oam_offset(39, OAMA_TILEID), 158);
    assert_eq!(OAMRAM.value + oam_offset(39, OAMA_FLAGS), OAM_END - 1);
}

#[test]
#[should_panic(expected = "OAM entry 40 does not exist: OAM has 40 entries, 0 to 39")]
fn test_oam_offset_rejects_an_entry_past_oam() {
    oam_offset(40, OAMA_Y);
}

#[test]
#[should_panic(expected = "OAMF_PRI (128) is not a byte of an OAM entry")]
fn test_oam_offset_rejects_a_flag_for_a_byte() {
    oam_offset(0, OAMF_PRI);
}

/// Only the four `OAMA_*` symbols are bytes of an entry: another `u8` symbol panics, even
/// one whose value is below 4
#[test]
fn test_oam_offset_takes_only_the_oama_symbols() {
    for symbol in [OAMB_BANK1, PADB_A, LCDCF_OFF, P1F_0, STATF_LCD] {
        let message = crate::engine::panic_message(|| oam_offset(0, symbol));
        assert!(
            message.contains(&format!(
                "{} ({}) is not a byte of an OAM entry",
                symbol.name, symbol.value
            )),
            "{}: {}",
            symbol.name,
            message
        );
    }
    for (offset, byte) in OAM_ENTRY_BYTES.into_iter().enumerate() {
        assert_eq!(usize::from(oam_offset(0, byte)), offset);
    }
    assert_eq!(OAM_ENTRY_BYTES.len(), usize::from(OAM_ENTRY_SIZE.value));
}

#[test]
fn test_regions_are_consistent() {
    assert_eq!((VRAM_OBJ_TILES_END - VRAM_OBJ_TILES) / TILE_SIZE, 256);
    assert_eq!((VRAM_BG_TILES_END - VRAM_BG_TILES) / TILE_SIZE, 128);
    assert_eq!(VRAM8800.value - VRAM8000.value, 128 * TILE_SIZE);
    assert_eq!(VRAM_BG_TILES_END, SCRN0.value);
    assert_eq!(SCRN0.value + SCRN_SIZE, SCRN1.value);
    assert_eq!(SCRN1.value + SCRN_SIZE, VRAM_END);
    assert_eq!(VRAM.value, VRAM8000.value);
    assert_eq!(
        SCRN_SIZE,
        u16::from(SCRN_VX_B.value) * u16::from(SCRN_VY_B.value)
    );
    assert_eq!(SCRN_VX.value, u16::from(SCRN_VX_B.value) * 8);
    assert_eq!(SCRN_X.value, SCRN_X_B.value * TILE_WIDTH);
    assert_eq!(SCRN_Y.value, SCRN_Y_B.value * TILE_WIDTH);
    assert_eq!(u16::from(TILE_WIDTH) * 2, TILE_SIZE, "8 rows of 2 bytes");
    assert_eq!(WRAM0_END - RAM.value, 0x1000);
    assert_eq!(WRAM_END - RAM.value, 0x2000);
    assert_eq!(OAM_END - OAMRAM.value, u16::from(OAM_SIZE));
    assert_eq!(OAM_SIZE, OAM_COUNT.value * OAM_ENTRY_SIZE.value);
    assert_eq!(HRAM_END, IE.value);
    assert_eq!(IO.value, P1.value);
    assert_eq!(ROM_HEADER_END - ROM_HEADER, 0x50);
    assert_eq!(P1F_GET_NONE.value, P1F_GET_BTN.value | P1F_GET_DPAD.value);
    for (flag, bit) in [
        (PADF_DOWN, PADB_DOWN),
        (PADF_UP, PADB_UP),
        (PADF_LEFT, PADB_LEFT),
        (PADF_RIGHT, PADB_RIGHT),
        (PADF_START, PADB_START),
        (PADF_SELECT, PADB_SELECT),
        (PADF_B, PADB_B),
        (PADF_A, PADB_A),
    ] {
        assert_eq!(flag.value, 1 << bit.value, "{}", flag.name);
    }
}

// === The guard: no hardware strings outside `hw` ===

/// Things found in Rust source, each with its line: (line, text)
type Findings = Vec<(usize, String)>;

/// The hardware names and addresses that `text`, Rust source, writes in its string
/// literals, and the hardware addresses it writes as hexadecimal integers, outside
/// comments and outside every item marked `#[cfg(test)]` (tests spell out the text they
/// expect)
///
/// What it flags:
/// - in a string literal, any name `hardware.inc` defines (`names`), as a whole word
///   (`"rLCDC"`, `"_OAMRAM+4"`, `"LCDCF_ON | LCDCF_BGON"`), and any hexadecimal number from
///   `$8000` on, written `$9800` or `0x9800` (VRAM, cartridge RAM, WRAM, OAM, I/O, HRAM);
/// - in code, any hexadecimal integer from `0x8000` on (`Expr::hex(0x9800)`). This
///   includes values that are not addresses, such as a mask `0xFF00` or the two's
///   complement `0xFFE0` (-32): write those from `hw` or as arithmetic
///   (`u16::from(hw::SCRN_VX_B.value).wrapping_neg()`), or in decimal.
///
/// What it does not see: decimal, octal and binary numbers (`144`, `0o177`, `0b1000_0000`,
/// `65344`), short hex strings below `$8000` (`"$41"` for an `ldh` offset, `"$FF"`), text
/// built at run time (`format!` pieces, `concat!`, a `char` pushed onto a `String`), and
/// comments, doc comments included. Those are left to review.
fn hardware_literals(text: &str, names: &BTreeSet<&str>) -> Findings {
    let (strings, integers) = scan(text);
    let mut findings = Vec::new();
    for (line, literal) in strings {
        for word in literal.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if names.contains(word) {
                findings.push((line, format!("\"{}\" names {}", literal, word)));
            }
        }
        for number in hex_numbers(&literal, &["$", "0x", "0X"]) {
            findings.push((
                line,
                format!("\"{}\" holds the address {}", literal, number),
            ));
        }
    }
    for (line, number) in integers {
        findings.push((line, format!("the integer {} is an address", number)));
    }
    findings
}

/// The hexadecimal numbers of `text` written after one of `prefixes` whose value is
/// `$8000` or more
fn hex_numbers(text: &str, prefixes: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for prefix in prefixes {
        for (start, _) in text.match_indices(prefix) {
            let digits: String = text[start + prefix.len()..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit() || *c == '_')
                .filter(|c| *c != '_')
                .collect();
            if u32::from_str_radix(&digits, 16).is_ok_and(|value| value >= 0x8000) {
                found.push(format!("{}{}", prefix, digits));
            }
        }
    }
    found
}

/// Where the scan is in a `#[cfg(test)]` item: the brace depth of the attribute, and
/// whether the item's `{ … }` body has opened
#[derive(Clone, Copy)]
struct TestItem {
    depth: usize,
    in_body: bool,
}

/// Read Rust source: (string literals, hexadecimal integers from `0x8000` on), each with
/// its line, outside comments and outside `#[cfg(test)]` items
///
/// Comments are skipped (`//` to the end of the line, `/* */`, nested too), strings
/// (`"…"` with escapes, raw `r#"…"#`) and character literals are read whole, so a quote,
/// a brace or a `//` in them does not confuse it. The item after a `#[cfg(test)]` (a
/// `mod tests { … }`, a helper `fn … { … }`, a `mod tests;`, a `use …;`) is skipped up to
/// its matching `}`, or its `;` when it has no body; the code after it is read again.
fn scan(code: &str) -> (Findings, Findings) {
    let cfg_test: Vec<char> = "#[cfg(test)]".chars().collect();
    let chars: Vec<char> = code.chars().collect();
    let (mut strings, mut integers) = (Vec::new(), Vec::new());
    let (mut line, mut i, mut depth) = (1, 0, 0usize);
    let mut test_item: Option<TestItem> = None;
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let after_ident = i > 0 && is_ident(chars[i - 1]);
        let recording = test_item.is_none();
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            // Block comments nest: `/* /* */ */` is one comment
            let mut nesting = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    nesting += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    nesting -= 1;
                    i += 2;
                    if nesting == 0 {
                        break;
                    }
                } else {
                    line += usize::from(chars[i] == '\n');
                    i += 1;
                }
            }
        } else if c == '#' && chars[i..].starts_with(&cfg_test) {
            if recording {
                test_item = Some(TestItem {
                    depth,
                    in_body: false,
                });
            }
            i += cfg_test.len();
        } else if c == '{' {
            depth += 1;
            if let Some(item) = test_item.as_mut() {
                if !item.in_body && depth == item.depth + 1 {
                    item.in_body = true;
                }
            }
            i += 1;
        } else if c == '}' {
            depth = depth.saturating_sub(1);
            if test_item.is_some_and(|item| item.in_body && depth == item.depth) {
                test_item = None;
            }
            i += 1;
        } else if c == ';' {
            if test_item.is_some_and(|item| !item.in_body && depth == item.depth) {
                test_item = None;
            }
            i += 1;
        } else if c == 'r' && !after_ident && matches!(next, Some('"' | '#')) {
            // A raw string, r"…" or r#"…"#
            let hashes = chars[i + 1..].iter().take_while(|c| **c == '#').count();
            let open = i + 1 + hashes;
            if chars.get(open) != Some(&'"') {
                i += 1; // `r#ident`
                continue;
            }
            let (start_line, mut j) = (line, open + 1);
            let closing: Vec<char> = std::iter::once('"')
                .chain("#".repeat(hashes).chars())
                .collect();
            while j < chars.len() && !chars[j..].starts_with(&closing) {
                line += usize::from(chars[j] == '\n');
                j += 1;
            }
            if recording {
                strings.push((start_line, chars[open + 1..j].iter().collect()));
            }
            i = j + closing.len();
        } else if c == '"' {
            let (start_line, mut j) = (line, i + 1);
            while j < chars.len() && chars[j] != '"' {
                // An escape is two characters, `\"` or `\` and a line break
                let step = if chars[j] == '\\' { 2 } else { 1 };
                let end = (j + step).min(chars.len());
                line += chars[j..end].iter().filter(|c| **c == '\n').count();
                j = end;
            }
            if recording {
                strings.push((
                    start_line,
                    chars[i + 1..j.min(chars.len())].iter().collect(),
                ));
            }
            i = j + 1;
        } else if c == '\'' {
            // A character literal ('"', '\'', '{'), or a lifetime ('a)
            if next == Some('\\') {
                // `'\''`, `'\n'`, `'\u{..}'`: past the escaped character, to the quote
                i += 3;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if chars.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if c.is_ascii_digit() && !after_ident {
            let token: String = chars[i..].iter().take_while(|c| is_ident(**c)).collect();
            i += token.len();
            if recording && token.starts_with("0x") {
                integers.extend(
                    hex_numbers(&token, &["0x"])
                        .into_iter()
                        .map(|number| (line, number)),
                );
            }
        } else {
            i += 1;
        }
    }
    (strings, integers)
}

/// Every `.rs` file under `dir`, sorted
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {}", dir.display(), error))
        .map(|entry| entry.expect("a directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}

/// No `hardware.inc` name and no hardware address is written as text (or as a hex
/// integer) in `stdlib`, `engine` or the examples: they take them from `hw`. Items
/// marked `#[cfg(test)]` are not checked (tests spell out the text they expect), nor is the
/// raw-`asm` tutorial `src/bin/unbricked.rs`, written by hand on purpose (CLAUDE.md).
/// What it flags, and what it does not see: [`hardware_literals`].
#[test]
fn test_no_hardware_strings_outside_hw() {
    let names = hardware_inc_names();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let exempt = root.join("src/bin/unbricked.rs");
    let mut files = Vec::new();
    for dir in ["src/stdlib", "src/engine", "src/bin"] {
        files.extend(rust_files(&root.join(dir)));
    }
    assert!(files.len() > 20, "{} files", files.len());
    let mut errors = Vec::new();
    for file in files.iter().filter(|file| **file != exempt) {
        let text = std::fs::read_to_string(file).expect("a source file");
        for (line, finding) in hardware_literals(&text, &names) {
            errors.push(format!(
                "{}:{}: {}",
                file.strip_prefix(root).unwrap_or(file).display(),
                line,
                finding
            ));
        }
    }
    assert!(
        errors.is_empty(),
        "hardware names or addresses written by hand; take them from `crate::hw` \
         (`hw::LCDC`, `Expr::from(hw::OAMRAM) + 4`, `hw::SCRN0.value`):\n{}",
        errors.join("\n")
    );
}

/// The guard finds what it should, where it should, and nothing in comments (nested
/// too), `#[cfg(test)]` items (a module, a helper in the middle of a file, a method, a
/// `use`, a `mod x;`) or other text
#[test]
fn test_the_guard_finds_hardware_literals() {
    let names = hardware_inc_names();
    let source = r##"
use x; // "rLCDC" in a comment
/// "_OAMRAM+4" in a doc comment
fn f() {
    asm.ld_addr_def_a("rLCDC");
    asm.ld(R16::HL, Expr::sym("_OAMRAM") + 4);
    asm.ld(R16::BC, Expr::hex(0x9800));
    cp_in_memory("Tiles", "$9000");
    let s = "a \" quote, then PADF_LEFT";
    let c = '"'; let d = "wCurKeys"; /* "rLY" */
    let raw = r#"LCDCF_ON | "x""#;
    let fine = ("$150 - @", 0x0100, 0xF0, "${:04X}", "Delay", "rLCDCX", 'a', '\'', '\n', "\\", b'x');
    let not_a_number = id_0x9800;
    let message = "a message \
                   on two lines, rSCX";
    let after = "_HRAM";
    /* nested /* "rLY" */ still a comment "rWX" */ let braces = ("}", '{', '}');
}
#[cfg(test)]
mod tests {
    fn g() { assert_eq!(text, "ld [rLCDC], a"); }
}
#[cfg(test)]
fn helper() -> &'static str { if x { "rLY" } else { "_SCRN0" } }
fn after_the_helper() { let x = "rSTAT"; }
#[cfg(test)]
use y::{a, b};
#[cfg(test)]
mod more;
const Z: u16 = 0xFF41;
impl S {
    #[cfg(test)]
    pub(crate) fn get(&self) -> u8 { "rIE" }
    fn real(&self) { "rIF" }
}
"##;
    let found: Findings = hardware_literals(source, &names);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    // Strings first, then integers: only the test items are skipped, not the code after
    assert_eq!(
        lines,
        [5, 6, 8, 9, 11, 14, 16, 25, 34, 7, 30],
        "{:#?}",
        found
    );
    assert!(found[7].1.contains("names rSTAT"), "{:?}", found[7]);
    assert!(found[8].1.contains("names rIF"), "{:?}", found[8]);
    assert!(
        found[10].1.contains("the integer 0xFF41"),
        "{:?}",
        found[10]
    );
    assert!(found[0].1.contains("names rLCDC"), "{:?}", found[0]);
    assert!(
        found[2].1.contains("holds the address $9000"),
        "{:?}",
        found[2]
    );
    assert!(found[9].1.contains("the integer 0x9800"), "{:?}", found[9]);
}
