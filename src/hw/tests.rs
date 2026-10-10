//! Tests of the `hw` module: its symbols against `include/hardware.inc` (names always,
//! values with RGBDS under `RGBDS_LINK_CHECK`) and its layout.

use std::collections::BTreeSet;

use super::*;
use crate::gb_asm::label_check::{rgbds_accepts, rgbds_rom};

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
