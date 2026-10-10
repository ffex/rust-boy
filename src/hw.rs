//! Game Boy hardware facts, as pure data: the I/O registers and their flags, the memory
//! map, the OAM layout and the screen sizes, each with the `hardware.inc` name the
//! generated code writes for it.
//!
//! Every value that `include/hardware.inc` (v4.x) defines is a [`Symbol`]: its name, exactly
//! as `hardware.inc` spells it, and its value. The Rust name is the `hardware.inc` name
//! without its `r` or `_` prefix: [`LCDC`] is `rLCDC`, [`OAMRAM`] is `_OAMRAM`, [`SCRN0`] is
//! `_SCRN0`, and the flags keep their whole name ([`LCDCF_ON`], [`PADF_LEFT`]); the one
//! exception is [`OAM_ENTRY_SIZE`], `sizeof_OAM_ATTRS`. The facts `hardware.inc` has no
//! name for (where the sprite tiles end, the OAM coordinate offsets, the cartridge header)
//! are plain numbers ([`TILE_SIZE`], [`OAM_X_OFFSET`], [`ROM_HEADER_END`], ...).
//!
//! The code writes a symbol by its name and computes with its value:
//!
//! ```
//! use rust_boy::gb_asm::{Block, Expr, R8};
//! use rust_boy::hw;
//!
//! let mut asm = Block::new();
//! asm.ld(R8::A, Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON) // a symbol is an `Expr`
//!     .ld_addr_def_a(hw::LCDC)
//!     .ld_a_addr_def(Expr::from(hw::OAMRAM) + 5)
//!     .cp_imm(hw::SCRN_Y.value); // or its value, a number
//! let text: Vec<String> = asm.iter().map(|i| i.to_string()).collect();
//! assert_eq!(
//!     text,
//!     ["ld a, LCDCF_ON | LCDCF_BGON", "ld [rLCDC], a", "ld a, [_OAMRAM+5]", "cp a, 144"]
//! );
//! assert_eq!(hw::LCDC.value, 0xFF40);
//! ```
//!
//! Layering: this module depends on nothing. `gb_std` and `rust_boy` use it, and `gb_std`
//! turns a [`Symbol`] into an [`Expr`](crate::gb_asm::Expr) (or an operand) by its name;
//! `gb_asm` does not know it. A unit test assembles every symbol with RGBDS against
//! `include/hardware.inc` and asserts that its value is the one there; another fails if a
//! `hardware.inc` name or a hardware address is written as a string in `gb_std`, `rust_boy`
//! or the examples (the raw-`gb_asm` tutorial `src/bin/unbricked.rs` excepted) instead of
//! coming from here.

#[cfg(test)]
mod tests;

/// A symbol of `hardware.inc`: the `name` the generated code writes, and its `value`
/// (`u16` for an address, `u8` for a flag or a small count)
///
/// It does not implement `Display` on purpose: a message writes `.name` or `.value`, so
/// it never prints the one it did not mean. `gb_std` makes it an [`Expr`](crate::gb_asm::Expr), an
/// [`Operand`](crate::gb_asm::Operand) or an [`AluOperand`](crate::gb_asm::AluOperand)
/// (the symbol by its name), so it can be passed wherever the builders take one:
/// `ld_addr_def_a(hw::LCDC)` writes `ld [rLCDC], a`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Symbol<T> {
    /// The name in `hardware.inc`, which the generated code writes: `"rLCDC"`
    pub name: &'static str,
    /// The value `hardware.inc` gives it: `0xFF40`
    pub value: T,
}

/// Define each symbol as a `pub const`, and list them all in [`SYMBOLS`]
macro_rules! symbols {
    ($($(#[$doc:meta])* $id:ident: $t:ty = $name:literal, $value:expr;)*) => {
        $(
            $(#[$doc])*
            pub const $id: Symbol<$t> = Symbol { name: $name, value: $value };
        )*

        /// Every [`Symbol`] of this module, as (name, value): what the tests check
        /// against `include/hardware.inc`
        #[allow(clippy::unnecessary_cast)]
        pub const SYMBOLS: &[(&str, u16)] = &[$(($id.name, $id.value as u16)),*];
    };
}

symbols! {
    // === Memory map ===
    /// Start of VRAM, `_VRAM`: tile data from `$8000`, then the two tilemaps
    VRAM: u16 = "_VRAM", 0x8000;
    /// The first tile block, `_VRAM8000`: the sprite tiles 0 to 127 (and the background
    /// tiles with `LCDCF_BG8000`)
    VRAM8000: u16 = "_VRAM8000", 0x8000;
    /// The second tile block, `_VRAM8800`: the sprite tiles 128 to 255, shared with the
    /// background tiles 128 to 255
    VRAM8800: u16 = "_VRAM8800", 0x8800;
    /// The third tile block, `_VRAM9000`: the background tiles 0 to 127 with `LCDCF_BG8800`
    /// (as `RustBoy` uses them)
    VRAM9000: u16 = "_VRAM9000", 0x9000;
    /// The first tilemap, `_SCRN0` (`$9800`-`$9BFF`)
    SCRN0: u16 = "_SCRN0", 0x9800;
    /// The second tilemap, `_SCRN1` (`$9C00`-`$9FFF`)
    SCRN1: u16 = "_SCRN1", 0x9C00;
    /// Start of the cartridge RAM, `_SRAM` (`$A000`-`$BFFF`)
    SRAM: u16 = "_SRAM", 0xA000;
    /// Start of WRAM, `_RAM`: bank 0 (`$C000`-`$CFFF`), then bank 1
    RAM: u16 = "_RAM", 0xC000;
    /// Start of WRAM bank 1 (switchable on the Game Boy Color), `_RAMBANK` (`$D000`-`$DFFF`)
    RAMBANK: u16 = "_RAMBANK", 0xD000;
    /// Start of OAM, the sprite attribute table, `_OAMRAM` (`$FE00`-`$FE9F`)
    OAMRAM: u16 = "_OAMRAM", 0xFE00;
    /// Start of the I/O registers, `_IO` (`$FF00`-`$FF7F`, and `rIE` at `$FFFF`)
    IO: u16 = "_IO", 0xFF00;
    /// Start of HRAM, `_HRAM` (`$FF80`-`$FFFE`)
    HRAM: u16 = "_HRAM", 0xFF80;

    // === I/O registers ===
    /// Joypad, `rP1`: select the buttons or the D-pad (`P1F_GET_*`), then read 4 bits
    /// (0 = pressed)
    P1: u16 = "rP1", 0xFF00;
    /// Interrupt flags, `rIF` (bits `IEF_*`)
    IF: u16 = "rIF", 0xFF0F;
    /// LCD control, `rLCDC` (flags `LCDCF_*`)
    LCDC: u16 = "rLCDC", 0xFF40;
    /// LCD status, `rSTAT` (flags `STATF_*`)
    STAT: u16 = "rSTAT", 0xFF41;
    /// Background scroll Y, `rSCY`
    SCY: u16 = "rSCY", 0xFF42;
    /// Background scroll X, `rSCX`
    SCX: u16 = "rSCX", 0xFF43;
    /// The line the LCD is drawing, `rLY`: 0 to 143 on screen, 144 ([`SCRN_Y`]) to 153 in
    /// VBlank
    LY: u16 = "rLY", 0xFF44;
    /// LY compare, `rLYC`
    LYC: u16 = "rLYC", 0xFF45;
    /// OAM DMA source and start, `rDMA`
    DMA: u16 = "rDMA", 0xFF46;
    /// Background palette, `rBGP`
    BGP: u16 = "rBGP", 0xFF47;
    /// Object palette 0, `rOBP0`
    OBP0: u16 = "rOBP0", 0xFF48;
    /// Object palette 1, `rOBP1`
    OBP1: u16 = "rOBP1", 0xFF49;
    /// Window Y, `rWY`
    WY: u16 = "rWY", 0xFF4A;
    /// Window X + 7, `rWX`
    WX: u16 = "rWX", 0xFF4B;
    /// Interrupt enable, `rIE` (bits `IEF_*`)
    IE: u16 = "rIE", 0xFFFF;

    // === rP1 flags ===
    /// `rP1` bit 5: 0 selects the buttons
    P1F_5: u8 = "P1F_5", 0b0010_0000;
    /// `rP1` bit 4: 0 selects the D-pad
    P1F_4: u8 = "P1F_4", 0b0001_0000;
    /// `rP1` bit 3, an input: Down or Start
    P1F_3: u8 = "P1F_3", 0b0000_1000;
    /// `rP1` bit 2, an input: Up or Select
    P1F_2: u8 = "P1F_2", 0b0000_0100;
    /// `rP1` bit 1, an input: Left or B
    P1F_1: u8 = "P1F_1", 0b0000_0010;
    /// `rP1` bit 0, an input: Right or A
    P1F_0: u8 = "P1F_0", 0b0000_0001;
    /// Value for `rP1`: select the D-pad
    P1F_GET_DPAD: u8 = "P1F_GET_DPAD", 0b0010_0000;
    /// Value for `rP1`: select the buttons (A, B, Select, Start)
    P1F_GET_BTN: u8 = "P1F_GET_BTN", 0b0001_0000;
    /// Value for `rP1`: select nothing
    P1F_GET_NONE: u8 = "P1F_GET_NONE", 0b0011_0000;

    // === rLCDC flags ===
    /// LCDC: the LCD is off (0: no bit set)
    LCDCF_OFF: u8 = "LCDCF_OFF", 0;
    /// LCDC: the LCD is on
    LCDCF_ON: u8 = "LCDCF_ON", 0b1000_0000;
    /// LCDC: the window shows the tilemap at `$9800` (0)
    LCDCF_WIN9800: u8 = "LCDCF_WIN9800", 0;
    /// LCDC: the window shows the tilemap at `$9C00`
    LCDCF_WIN9C00: u8 = "LCDCF_WIN9C00", 0b0100_0000;
    /// LCDC: the window is hidden (0)
    LCDCF_WINOFF: u8 = "LCDCF_WINOFF", 0;
    /// LCDC: the window is shown
    LCDCF_WINON: u8 = "LCDCF_WINON", 0b0010_0000;
    /// LCDC: background and window tiles 0 to 127 at `$9000`, 128 to 255 at `$8800` (0)
    LCDCF_BG8800: u8 = "LCDCF_BG8800", 0;
    /// LCDC: background and window tiles at `$8000`, like the objects
    LCDCF_BG8000: u8 = "LCDCF_BG8000", 0b0001_0000;
    /// LCDC: the background shows the tilemap at `$9800` (0)
    LCDCF_BG9800: u8 = "LCDCF_BG9800", 0;
    /// LCDC: the background shows the tilemap at `$9C00`
    LCDCF_BG9C00: u8 = "LCDCF_BG9C00", 0b0000_1000;
    /// LCDC: 8x8 objects (0)
    LCDCF_OBJ8: u8 = "LCDCF_OBJ8", 0;
    /// LCDC: 8x16 objects
    LCDCF_OBJ16: u8 = "LCDCF_OBJ16", 0b0000_0100;
    /// LCDC: the objects (sprites) are hidden (0)
    LCDCF_OBJOFF: u8 = "LCDCF_OBJOFF", 0;
    /// LCDC: the objects (sprites) are shown
    LCDCF_OBJON: u8 = "LCDCF_OBJON", 0b0000_0010;
    /// LCDC: the background is hidden (0)
    LCDCF_BGOFF: u8 = "LCDCF_BGOFF", 0;
    /// LCDC: the background is shown
    LCDCF_BGON: u8 = "LCDCF_BGON", 0b0000_0001;

    // === rSTAT flags ===
    /// STAT: interrupt on LY = LYC
    STATF_LYC: u8 = "STATF_LYC", 0b0100_0000;
    /// STAT: interrupt on mode 2 (OAM scan)
    STATF_MODE10: u8 = "STATF_MODE10", 0b0010_0000;
    /// STAT: interrupt on mode 1 (VBlank)
    STATF_MODE01: u8 = "STATF_MODE01", 0b0001_0000;
    /// STAT: interrupt on mode 0 (HBlank)
    STATF_MODE00: u8 = "STATF_MODE00", 0b0000_1000;
    /// STAT: LY = LYC now
    STATF_LYCF: u8 = "STATF_LYCF", 0b0000_0100;
    /// STAT mode 0: HBlank
    STATF_HBL: u8 = "STATF_HBL", 0;
    /// STAT mode 1: VBlank
    STATF_VBL: u8 = "STATF_VBL", 0b0000_0001;
    /// STAT mode 2: the LCD reads OAM
    STATF_OAM: u8 = "STATF_OAM", 0b0000_0010;
    /// STAT mode 3: the LCD reads OAM and VRAM
    STATF_LCD: u8 = "STATF_LCD", 0b0000_0011;
    /// STAT: VRAM is busy (modes 2 and 3)
    STATF_BUSY: u8 = "STATF_BUSY", 0b0000_0010;

    // === rIE / rIF bits ===
    /// Interrupt: a joypad line went from high to low
    IEF_HILO: u8 = "IEF_HILO", 0b0001_0000;
    /// Interrupt: serial transfer done
    IEF_SERIAL: u8 = "IEF_SERIAL", 0b0000_1000;
    /// Interrupt: timer overflow
    IEF_TIMER: u8 = "IEF_TIMER", 0b0000_0100;
    /// Interrupt: STAT (`rSTAT` conditions)
    IEF_STAT: u8 = "IEF_STAT", 0b0000_0010;
    /// Interrupt: VBlank
    IEF_VBLANK: u8 = "IEF_VBLANK", 0b0000_0001;

    // === Joypad: the key bits as `UpdateKeys` stores them (`wCurKeys`, `wNewKeys`) ===
    /// Key flag: Down
    PADF_DOWN: u8 = "PADF_DOWN", 0x80;
    /// Key flag: Up
    PADF_UP: u8 = "PADF_UP", 0x40;
    /// Key flag: Left
    PADF_LEFT: u8 = "PADF_LEFT", 0x20;
    /// Key flag: Right
    PADF_RIGHT: u8 = "PADF_RIGHT", 0x10;
    /// Key flag: Start
    PADF_START: u8 = "PADF_START", 0x08;
    /// Key flag: Select
    PADF_SELECT: u8 = "PADF_SELECT", 0x04;
    /// Key flag: B
    PADF_B: u8 = "PADF_B", 0x02;
    /// Key flag: A
    PADF_A: u8 = "PADF_A", 0x01;
    /// Key bit number: Down
    PADB_DOWN: u8 = "PADB_DOWN", 7;
    /// Key bit number: Up
    PADB_UP: u8 = "PADB_UP", 6;
    /// Key bit number: Left
    PADB_LEFT: u8 = "PADB_LEFT", 5;
    /// Key bit number: Right
    PADB_RIGHT: u8 = "PADB_RIGHT", 4;
    /// Key bit number: Start
    PADB_START: u8 = "PADB_START", 3;
    /// Key bit number: Select
    PADB_SELECT: u8 = "PADB_SELECT", 2;
    /// Key bit number: B
    PADB_B: u8 = "PADB_B", 1;
    /// Key bit number: A
    PADB_A: u8 = "PADB_A", 0;

    // === Screen ===
    /// Screen width in pixels, `SCRN_X`
    SCRN_X: u8 = "SCRN_X", 160;
    /// Screen height in pixels, `SCRN_Y`: also the first `rLY` line of VBlank
    SCRN_Y: u8 = "SCRN_Y", 144;
    /// Screen width in tiles, `SCRN_X_B`
    SCRN_X_B: u8 = "SCRN_X_B", 20;
    /// Screen height in tiles, `SCRN_Y_B`
    SCRN_Y_B: u8 = "SCRN_Y_B", 18;
    /// Background map width in pixels, `SCRN_VX`
    SCRN_VX: u16 = "SCRN_VX", 256;
    /// Background map height in pixels, `SCRN_VY`
    SCRN_VY: u16 = "SCRN_VY", 256;
    /// Background map width in tiles (bytes in one tilemap row), `SCRN_VX_B`
    SCRN_VX_B: u8 = "SCRN_VX_B", 32;
    /// Background map height in tiles (rows in a tilemap), `SCRN_VY_B`
    SCRN_VY_B: u8 = "SCRN_VY_B", 32;

    // === OAM ===
    /// Sprites in OAM, `OAM_COUNT`
    OAM_COUNT: u8 = "OAM_COUNT", 40;
    /// Bytes in one OAM entry, `sizeof_OAM_ATTRS`: Y, X, tile, flags
    OAM_ENTRY_SIZE: u8 = "sizeof_OAM_ATTRS", 4;
    /// Byte of an OAM entry holding the Y coordinate, `OAMA_Y`
    OAMA_Y: u8 = "OAMA_Y", 0;
    /// Byte of an OAM entry holding the X coordinate, `OAMA_X`
    OAMA_X: u8 = "OAMA_X", 1;
    /// Byte of an OAM entry holding the tile index, `OAMA_TILEID`
    OAMA_TILEID: u8 = "OAMA_TILEID", 2;
    /// Byte of an OAM entry holding the attribute flags (`OAMF_*`), `OAMA_FLAGS`
    OAMA_FLAGS: u8 = "OAMA_FLAGS", 3;
    /// OAM flag: the background colours 1 to 3 are drawn over the sprite
    OAMF_PRI: u8 = "OAMF_PRI", 0b1000_0000;
    /// OAM flag: flip the sprite vertically
    OAMF_YFLIP: u8 = "OAMF_YFLIP", 0b0100_0000;
    /// OAM flag: flip the sprite horizontally
    OAMF_XFLIP: u8 = "OAMF_XFLIP", 0b0010_0000;
    /// OAM flag: palette `rOBP0` (0)
    OAMF_PAL0: u8 = "OAMF_PAL0", 0;
    /// OAM flag: palette `rOBP1`
    OAMF_PAL1: u8 = "OAMF_PAL1", 0b0001_0000;
    /// OAM flag (Game Boy Color): tiles from VRAM bank 0 (0)
    OAMF_BANK0: u8 = "OAMF_BANK0", 0;
    /// OAM flag (Game Boy Color): tiles from VRAM bank 1
    OAMF_BANK1: u8 = "OAMF_BANK1", 0b0000_1000;
    /// OAM flags mask (Game Boy Color): the palette number
    OAMF_PALMASK: u8 = "OAMF_PALMASK", 0b0000_0111;
    /// OAM flag bit number: priority
    OAMB_PRI: u8 = "OAMB_PRI", 7;
    /// OAM flag bit number: Y flip
    OAMB_YFLIP: u8 = "OAMB_YFLIP", 6;
    /// OAM flag bit number: X flip
    OAMB_XFLIP: u8 = "OAMB_XFLIP", 5;
    /// OAM flag bit number: palette `rOBP1`
    OAMB_PAL1: u8 = "OAMB_PAL1", 4;
    /// OAM flag bit number (Game Boy Color): VRAM bank 1
    OAMB_BANK1: u8 = "OAMB_BANK1", 3;
}

// === Facts `hardware.inc` has no name for ===

/// Start of the cartridge header, where execution starts (`SECTION "Header", ROM0[$100]`)
pub const ROM_HEADER: u16 = 0x0100;
/// End (exclusive) of the cartridge header: `rgbfix` fills `$104`-`$14F`, so code goes
/// from `$150` on (`ds $150 - @, 0` pads the header)
pub const ROM_HEADER_END: u16 = 0x0150;

/// Bytes in one 8x8 tile (2 bits per pixel)
pub const TILE_SIZE: u16 = 16;
/// Pixels in one row of a tile: a sprite is one tile wide
pub const TILE_WIDTH: u8 = 8;

/// Start of the sprite (object) tiles in VRAM, `_VRAM8000`: tile index `n` is at
/// `$8000 + n * 16`, for `n` from 0 to 255
pub const VRAM_OBJ_TILES: u16 = VRAM8000.value;
/// End (exclusive) of the sprite tiles: 256 tiles
pub const VRAM_OBJ_TILES_END: u16 = VRAM9000.value;
/// Start of the background tiles as `RustBoy` uses them, `_VRAM9000` (LCDC bit 4 off:
/// indices 0 to 127 are here)
pub const VRAM_BG_TILES: u16 = VRAM9000.value;
/// End (exclusive) of the background tiles: 128 tiles, then the tilemaps
pub const VRAM_BG_TILES_END: u16 = SCRN0.value;
/// End (exclusive) of VRAM
pub const VRAM_END: u16 = 0xA000;
/// Bytes in one tilemap: 32 x 32 tiles
pub const SCRN_SIZE: u16 = 0x400;

/// End (exclusive) of WRAM bank 0: a `WRAM0` section must fit in 4 KiB
pub const WRAM0_END: u16 = RAMBANK.value;
/// End (exclusive) of all WRAM
pub const WRAM_END: u16 = 0xE000;

/// End (exclusive) of OAM
pub const OAM_END: u16 = 0xFEA0;
/// Bytes in OAM: 40 entries of 4 bytes
pub const OAM_SIZE: u8 = 160;
/// OAM X of the screen's left edge: OAM X = screen x + 8
pub const OAM_X_OFFSET: u8 = 8;
/// OAM Y of the screen's top edge: OAM Y = screen y + 16
pub const OAM_Y_OFFSET: u8 = 16;

/// End (exclusive) of HRAM: `rIE` is at `$FFFF`
pub const HRAM_END: u16 = 0xFFFF;

/// The offset from [`OAMRAM`] of byte `byte` ([`OAMA_Y`], [`OAMA_X`], [`OAMA_TILEID`] or
/// [`OAMA_FLAGS`]) of OAM entry `index`: 5 for entry 1, X (the generated code writes
/// `_OAMRAM+5`)
///
/// # Panics
/// If `index` is not an OAM entry (0 to 39), or `byte` is not a byte of one (0 to 3): the
/// address would be past the entry, or past OAM.
#[track_caller]
pub fn oam_offset(index: u8, byte: Symbol<u8>) -> u16 {
    assert!(
        index < OAM_COUNT.value,
        "OAM entry {} does not exist: OAM has {} entries, 0 to {}",
        index,
        OAM_COUNT.value,
        OAM_COUNT.value - 1
    );
    assert!(
        byte.value < OAM_ENTRY_SIZE.value,
        "{} ({}) is not a byte of an OAM entry: use OAMA_Y, OAMA_X, OAMA_TILEID or OAMA_FLAGS",
        byte.name,
        byte.value
    );
    u16::from(index) * u16::from(OAM_ENTRY_SIZE.value) + u16::from(byte.value)
}
