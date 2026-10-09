//! Game Boy hardware facts, as pure data: addresses, sizes, and the `hardware.inc`
//! names the generated code uses for them.
//!
//! The start of the Phase 2 `hw` module (Task.md): code written since then takes its
//! hardware facts from here instead of writing them as strings or bare numbers; the
//! older code moves here in Phase 2. `gb_std` and `rust_boy` use it; `gb_asm` does not.

/// Bytes in one 8x8 tile (2 bits per pixel)
pub const TILE_SIZE: u16 = 16;

/// Start of the sprite (object) tiles in VRAM, `_VRAM8000`: tile index `n` is at
/// `$8000 + n * 16`, for `n` from 0 to 255
pub const VRAM_OBJ_TILES: u16 = 0x8000;
/// End (exclusive) of the sprite tiles: 256 tiles
pub const VRAM_OBJ_TILES_END: u16 = 0x9000;
/// Start of the background tiles as `RustBoy` uses them, `_VRAM9000` (LCDC bit 4 off:
/// indices 0 to 127 are here)
pub const VRAM_BG_TILES: u16 = 0x9000;
/// End (exclusive) of the background tiles: 128 tiles, then the tilemaps
pub const VRAM_BG_TILES_END: u16 = 0x9800;

/// The first background tilemap, `_SCRN0`
pub const SCRN0: u16 = 0x9800;
/// The second background tilemap, `_SCRN1`
pub const SCRN1: u16 = 0x9C00;
/// Bytes in one tilemap: 32 x 32 tiles
pub const SCRN_SIZE: u16 = 0x400;
/// Tiles in one row of a tilemap, `SCRN_VX_B`
pub const SCRN_ROW_TILES: usize = 32;
/// Rows in a tilemap, `SCRN_VY_B`
pub const SCRN_ROWS: usize = 32;

/// `hardware.inc` LCDC flag: the background shows the tilemap at `$9800`
pub const LCDCF_BG9800: &str = "LCDCF_BG9800";
/// `hardware.inc` LCDC flag: the background shows the tilemap at `$9C00`
pub const LCDCF_BG9C00: &str = "LCDCF_BG9C00";

/// Start of WRAM bank 0, `_RAM`
pub const WRAM0: u16 = 0xC000;
/// End (exclusive) of WRAM bank 0: a `WRAM0` section must fit in 4 KiB
pub const WRAM0_END: u16 = 0xD000;
/// End (exclusive) of all WRAM
pub const WRAM_END: u16 = 0xE000;

/// Start of OAM, the sprite attribute table (`_OAMRAM` in `hardware.inc`)
pub const OAM_START: u16 = 0xFE00;
/// End (exclusive) of OAM
pub const OAM_END: u16 = 0xFEA0;
/// `hardware.inc` name of the start of OAM
pub const OAMRAM: &str = "_OAMRAM";
/// Sprites in OAM, `OAM_COUNT`
pub const OAM_COUNT: u8 = 40;
/// Bytes in one OAM entry
pub const OAM_ENTRY_SIZE: u8 = 4;
/// Byte of an OAM entry holding the Y coordinate, `OAMA_Y`
pub const OAMA_Y: u8 = 0;
/// Byte of an OAM entry holding the X coordinate, `OAMA_X`
pub const OAMA_X: u8 = 1;
/// Byte of an OAM entry holding the tile index, `OAMA_TILEID`
pub const OAMA_TILEID: u8 = 2;
/// OAM X of the screen's left edge: OAM X = screen x + 8
pub const OAM_X_OFFSET: u8 = 8;
/// OAM Y of the screen's top edge: OAM Y = screen y + 16
pub const OAM_Y_OFFSET: u8 = 16;

/// The address of byte `byte` (`OAMA_Y`, `OAMA_X`, ...) of OAM entry `index`, as the
/// generated code writes it: `_OAMRAM+5` for entry 1, X
pub fn oam_address(index: u8, byte: u8) -> String {
    format!(
        "{}+{}",
        OAMRAM,
        u16::from(index) * u16::from(OAM_ENTRY_SIZE) + u16::from(byte)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oam_address() {
        assert_eq!(oam_address(0, OAMA_Y), "_OAMRAM+0");
        assert_eq!(oam_address(1, OAMA_X), "_OAMRAM+5");
        assert_eq!(oam_address(39, OAMA_TILEID), "_OAMRAM+158");
    }

    #[test]
    fn test_regions_are_consistent() {
        assert_eq!((VRAM_OBJ_TILES_END - VRAM_OBJ_TILES) / TILE_SIZE, 256);
        assert_eq!((VRAM_BG_TILES_END - VRAM_BG_TILES) / TILE_SIZE, 128);
        assert_eq!(VRAM_BG_TILES_END, SCRN0);
        assert_eq!(SCRN0 + SCRN_SIZE, SCRN1);
        assert_eq!(usize::from(SCRN_SIZE), SCRN_ROW_TILES * SCRN_ROWS);
        assert_eq!(
            OAM_END - OAM_START,
            u16::from(OAM_COUNT) * u16::from(OAM_ENTRY_SIZE)
        );
    }
}
