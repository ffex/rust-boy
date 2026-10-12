use crate::asm::{Block, Condition, Expr, Instr, Mem, R8, R16};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

//TODO
// refactor code:
// - punt in the form of builder (like cp_in_memory)

pub fn add_tiles(label: &str, tiles: &[[&str; 8]]) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.label(label);
    for tile in tiles {
        for line in tile {
            asm.dw(line);
        }
    }
    asm.label(&format!("{}End", label));
    asm.into_instrs()
}

pub fn add_tiles_2bpp(label: &str, path: &str) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.label(label);
    asm.incbin(path);
    asm.label(&format!("{}End", label));
    asm.into_instrs()
}

pub fn add_tiles_tilemap(label: &str, path: &str) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.label(label);
    asm.incbin(path);
    asm.label(&format!("{}End", label));
    asm.into_instrs()
}

pub fn add_tilemap(label: &str, tilemap: &[[u8; 32]]) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.label(label);
    for row in tilemap {
        let values: Vec<String> = row.iter().map(|&val| format!("${:02X}", val)).collect();
        asm.db(&values.join(", "));
    }
    asm.label(&format!("{}End", label));
    asm.into_instrs()
}

/// Copy the data between `label` and `{label}End` to `addr` with [`memcopy`]
///
/// `addr` is an address: a number (`Expr::hex(hw::VRAM9000.value)`, written `$9000`) or
/// a symbol (`hw::VRAM`, written `_VRAM`, or a label). Empty data (`{label}End` right after
/// `label`) copies nothing.
///
/// # Panics
/// If `label` is not a symbol name, or `addr` is text that is neither a symbol nor a
/// number (see [`Expr`]).
#[track_caller]
pub fn cp_in_memory(label: &str, addr: impl Into<Expr>) -> Vec<Instr> {
    let start = Expr::sym(label);
    let end = Expr::sym(format!("{}End", label));
    let mut asm = Block::new();
    asm.ld(R16::DE, start.clone())
        .ld(R16::HL, addr.into())
        .ld(R16::BC, end - start)
        .call("Memcopy");
    asm.into_instrs()
}

/// The `Memcopy` routine: copy `bc` bytes from `de` to `hl`
///
/// - Reads: `de` = source, `hl` = destination, `bc` = length; a length of 0 copies nothing.
/// - Returns: `de` and `hl` point after the copied bytes, `bc` = 0.
/// - Clobbers: `a` and the flags.
///
/// The length is tested before the first byte (3 bytes of code): the copy loop alone
/// copies at least one byte, so a length of 0 used to wrap to `$FFFF` and copy 64 KiB
/// over WRAM, the stack and the I/O registers (B27).
pub fn memcopy() -> Routine {
    let mut asm = Block::new();
    asm.comment("Copy bytes from one area to another");
    asm.comment("@param de: source");
    asm.comment("@param hl: destination");
    asm.comment("@param bc: length (0 copies nothing)");
    asm.label("Memcopy");
    asm.ld(R8::A, R8::B);
    asm.or(R8::C);
    asm.ret_cond(Condition::Z);
    asm.label(".copy");
    asm.ld(R8::A, Mem::De);
    asm.ld(Mem::Hli, R8::A);
    asm.inc(R16::DE);
    asm.dec(R16::BC);
    asm.ld(R8::A, R8::B);
    asm.or(R8::C);
    asm.jp_cond(Condition::NZ, ".copy");
    asm.ret();
    Routine::new("Memcopy", asm)
        .with_reads(Regs::BC | Regs::DE | Regs::HL)
        .with_returns(Regs::BC | Regs::DE | Regs::HL)
        .with_clobbers(Regs::A | Regs::F)
}
/// Turn the LCD off by writing `LCDCF_OFF`: every other LCDC setting is cleared too.
/// Emit only in VBlank. To keep the program's LCDC settings, use [`lcd_off`].
pub fn turn_off_screen() -> Vec<Instr> {
    let mut asm = Block::new();
    // Turn off LCD
    asm.ld_a(hw::LCDCF_OFF.value).ld_addr_def_a(hw::LCDC);
    asm.into_instrs()
}

/// Turn the LCD on by writing `LCDCF_ON | LCDCF_BGON | LCDCF_OBJON`: the `$9800` background map,
/// 8x8 sprites and no window, whatever the program configured. To keep them, use [`lcd_on`].
pub fn turn_on_screen() -> Vec<Instr> {
    let mut asm = Block::new();
    // Turn on LCD
    let on = Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON | hw::LCDCF_OBJON;
    asm.ld(R8::A, on).ld_addr_def_a(hw::LCDC);
    asm.into_instrs()
}

/// Turn the LCD off (LCDC bit 7), preserving every other LCDC bit: the maps, sprite size and
/// window settings come back with [`lcd_on`]. Emit only in VBlank: turning the LCD off
/// outside it can damage a DMG. Changes A and flags only.
pub fn lcd_off() -> Vec<Instr> {
    let mut code = Block::new();
    code.ldh(R8::A, Mem::addr(hw::LCDC))
        .and(Expr::low(!Expr::from(hw::LCDCF_ON)))
        .ldh(Mem::addr(hw::LCDC), R8::A);
    code.into_instrs()
}

/// Turn the LCD on (LCDC bit 7), preserving every other LCDC bit, e.g. after [`lcd_off`] and a
/// runtime tilemap copy. Changes A and flags only.
pub fn lcd_on() -> Vec<Instr> {
    let mut code = Block::new();
    code.ldh(R8::A, Mem::addr(hw::LCDC))
        .or(hw::LCDCF_ON)
        .ldh(Mem::addr(hw::LCDC), R8::A);
    code.into_instrs()
}

/// The `WaitVBlank` routine: wait until the LCD is in VBlank (`rLY` from 144 to 153)
///
/// Returns at once in VBlank. With the LCD off, `rLY` stays 0 and it never returns.
/// - Reads and returns nothing.
/// - Clobbers: `a` and the flags.
pub fn wait_vblank() -> Routine {
    let mut asm = Block::new();
    asm.label("WaitVBlank");
    asm.ld_a_addr_def(hw::LY);
    asm.cp_imm(hw::SCRN_Y.value); // VBlank: LY from 144 to 153
    asm.jp_cond(Condition::C, "WaitVBlank");
    asm.ret();
    Routine::new("WaitVBlank", asm).with_clobbers(Regs::A | Regs::F)
}

/// The `WaitNotVBlank` routine: wait until the LCD is out of VBlank (`rLY` below 144)
///
/// - Reads and returns nothing.
/// - Clobbers: `a` and the flags.
pub fn wait_not_vblank() -> Routine {
    let mut asm = Block::new();
    asm.label("WaitNotVBlank");
    asm.ld_a_addr_def(hw::LY);
    asm.cp_imm(hw::SCRN_Y.value);
    asm.jp_cond(Condition::NC, "WaitNotVBlank");
    asm.ret();
    Routine::new("WaitNotVBlank", asm).with_clobbers(Regs::A | Regs::F)
}

/// The `GetTileByPixel` routine: the background tile under a pixel
///
/// This is the only `GetTileByPixel`: `RustBoy` emits this one too
/// (`BuiltinFunction::GetTileByPixel`). Its contract:
/// - Reads: `b` = X and `c` = Y, in pixels on the background map selected by LCDC bit 3 (0 to 255;
///   the screen pixel when `rSCX` and `rSCY` are 0). `get_pivot` (of a `stdlib` `Sprite`,
///   or of the `RustBoy` sprite manager) loads them from a sprite's position.
/// - Returns: `hl` = the address of that tile in the map, `map_base + (Y / 8) * 32 + X / 8`,
///   and `a` = the tile index stored there (`[hl]`). So a caller can test the tile at
///   once (`IfConst`, `IfA`, `IfCall`), then change it through `hl` (`TileRef`).
/// - Clobbers: `bc` and the flags; `de` is kept.
///
/// It reads VRAM: call it while VRAM is accessible (in VBlank, or with the LCD off).
pub fn get_tile_by_pixel() -> Routine {
    let mut asm = Block::new();

    asm.comment("Convert a pixel position to a tilemap address and read the tile there");
    asm.comment(&format!(
        "hl = active background map + X / 8 + (Y / 8) * {}",
        hw::SCRN_VX_B.value
    ));
    asm.comment("@param b: X");
    asm.comment("@param c: Y");
    asm.comment("@return hl: tile address");
    asm.comment("@return a: tile index at that address");
    asm.comment("changes bc");
    asm.label("GetTileByPixel");

    // First, we need to divide by 8 to convert a pixel position to a tile position.
    // After this we want to multiply the Y position by 32.
    // These operations effectively cancel out so we only need to mask the Y value.
    asm.ld(R8::A, R8::C);
    asm.and(0b11111000);
    asm.ld(R8::L, R8::A);
    asm.ld(R8::H, 0);

    // Now we have the position * 8 in hl
    asm.add_hl(R16::HL); // position * 16
    asm.add_hl(R16::HL); // position * 32

    // Convert the X position to an offset.
    asm.ld(R8::A, R8::B);
    asm.srl(R8::A); // a / 2
    asm.srl(R8::A); // a / 4
    asm.srl(R8::A); // a / 8

    // Add the two offsets together.
    asm.add(R8::L);
    asm.ld(R8::L, R8::A);
    asm.adc(R8::H);
    asm.sub(R8::L);
    asm.ld(R8::H, R8::A);

    // Add the offset to the tilemap's base address
    asm.ld(R16::BC, Expr::hex(hw::SCRN0.value));
    // Read the active background map at runtime, including changes made in a handler.
    asm.ld_a_addr_def(hw::LCDC);
    asm.and(hw::LCDCF_BG9C00);
    asm.jr_cond(Condition::Z, ".map_selected");
    asm.ld(R16::BC, Expr::hex(hw::SCRN1.value));
    asm.label(".map_selected");
    asm.add_hl(R16::BC);

    // And read the tile there
    asm.ld(R8::A, R8::AtHl);
    asm.ret();

    Routine::new("GetTileByPixel", asm)
        .with_reads(Regs::BC)
        .with_returns(Regs::A | Regs::HL)
        .with_clobbers(Regs::BC | Regs::F)
}

/// A routine `label` that sets the Z flag when `a` is one of the tiles `tiles_ids`
///
/// Each tile id is a number (`"$00"`) or a constant (`"BRICK_LEFT"`), as [`Expr`] reads
/// text.
/// - Reads: `a`, the tile index (as `GetTileByPixel` returns it).
/// - Returns: the flags, Z set when `a` is one of the tiles (`IfCall::is_true`).
/// - Clobbers nothing: `a` is kept.
///
/// # Panics
/// If `label` is not an RGBDS identifier, or a tile id is neither a symbol nor a number.
#[track_caller]
pub fn is_specific_tile(label: &str, tiles_ids: &[&str]) -> Routine {
    let mut asm = Block::new();
    asm.label(label);
    for (index, tile_id) in tiles_ids.iter().enumerate() {
        asm.cp(*tile_id); //TODO understand the tile id and how to manage it!
        if index < tiles_ids.len() - 1 {
            asm.ret_cond(Condition::Z);
        }
    }
    asm.ret();
    Routine::new(label, asm)
        .with_reads(Regs::A)
        .with_returns(Regs::F)
        .with_clobbers(Regs::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::{Event, TestCpu};
    use crate::stdlib::graphics::sprites::Sprite;

    /// The tile index the tests store at map address `addr`: different for neighbours
    fn tile_at(addr: u16) -> u8 {
        (addr % 251) as u8
    }

    /// A CPU whose background maps at `$9800` and `$9C00` hold [`tile_at`], showing `$9800`
    fn cpu_with_map() -> TestCpu {
        cpu_with_lcdc(0)
    }

    /// [`cpu_with_map`] with `rLCDC` = `lcdc`
    fn cpu_with_lcdc(lcdc: u8) -> TestCpu {
        let mut cpu = TestCpu::default();
        cpu.mem.insert(hw::LCDC.name.to_string(), lcdc);
        cpu.consts
            .insert(hw::LCDCF_BG9C00.name.to_string(), hw::LCDCF_BG9C00.value);
        for addr in 0x9800..0xA000 {
            cpu.mem.insert(format!("${:04X}", addr), tile_at(addr));
        }
        cpu
    }

    #[test]
    fn lcd_off_and_on_flip_only_bit_7() {
        for code in [lcd_off(), lcd_on()] {
            assert_eq!(Regs::written_by(&code).unwrap() | Regs::AF, Regs::AF);
        }
        // Every other bit set or clear (BG map $9C00, 8x16 sprites, window on its $9C00 map)
        for others in [0x7F, 0x00, 0x6E] {
            let mut cpu = cpu_with_lcdc(others | hw::LCDCF_ON.value);
            cpu.consts
                .insert(hw::LCDCF_ON.name.to_string(), hw::LCDCF_ON.value);
            cpu.run(&lcd_off());
            assert_eq!(cpu.mem.get(hw::LCDC.name), Some(&others));
            cpu.run(&lcd_on());
            assert_eq!(
                cpu.mem.get(hw::LCDC.name),
                Some(&(others | hw::LCDCF_ON.value))
            );
        }
    }

    #[test]
    fn test_get_tile_by_pixel_returns_the_address_and_the_tile() {
        // B23: one contract. In: b = X, c = Y (pixels). Out: hl = $9800 + (Y/8)*32 + X/8,
        // a = [hl]; de kept. (The stdlib copy did not return the tile in a.)
        // The map is the one LCDC bit 3 selects: $9C00 when set, whatever the other bits
        let routine = get_tile_by_pixel();
        let positions = [0u8, 1, 7, 8, 9, 100, 143, 159, 255];
        let lcdc_on = hw::LCDCF_ON.value | hw::LCDCF_BGON.value;
        for (lcdc, map) in [
            (0, 0x9800u16),
            (lcdc_on, 0x9800),
            (hw::LCDCF_BG9C00.value, 0x9C00),
            (lcdc_on | hw::LCDCF_BG9C00.value, 0x9C00),
        ] {
            for x in positions {
                for y in positions {
                    let mut cpu = cpu_with_lcdc(lcdc);
                    (cpu.b, cpu.c, cpu.d, cpu.e) = (x, y, 0x12, 0x34);
                    cpu.run(routine.body());
                    let addr = map + u16::from(y / 8) * 32 + u16::from(x / 8);
                    let hl = u16::from_be_bytes([cpu.h, cpu.l]);
                    assert_eq!(hl, addr, "hl for LCDC {:#04x} X {} Y {}", lcdc, x, y);
                    assert_eq!(
                        cpu.a,
                        tile_at(addr),
                        "a for LCDC {:#04x} X {} Y {}",
                        lcdc,
                        x,
                        y
                    );
                    assert_eq!((cpu.d, cpu.e), (0x12, 0x34), "de kept");
                    assert!(cpu.trace.is_empty(), "no writes: {:?}", cpu.trace);
                }
            }
        }
    }

    #[test]
    fn test_get_pivot_then_get_tile_by_pixel_then_a_tile_test() {
        // The stdlib callers (unbricked_std): get_pivot, GetTileByPixel, then a routine
        // that tests the tile in a, with no `ld a, [hl]` in between
        let ball = Sprite::new(1, 0, 0, 0, 0);
        let mut code = ball.get_pivot(0, 1);
        let mut asm = Block::new();
        asm.call("GetTileByPixel").call("IsWallTile").ret();
        code.extend(asm.into_instrs());
        code.extend(get_tile_by_pixel());
        code.extend(is_specific_tile("IsWallTile", &["WALL"]));

        // The ball at OAM X 48, Y 57: the pixel above it is (40, 40), map tile (5, 5)
        let wall_addr = 0x9800 + 5 * 32 + 5;
        for (tile, is_wall) in [(3u8, true), (4, false)] {
            let mut cpu = cpu_with_map();
            cpu.mem.insert("_OAMRAM+4".to_string(), 57);
            cpu.mem.insert("_OAMRAM+5".to_string(), 48);
            cpu.mem.insert(format!("${:04X}", wall_addr), tile);
            cpu.consts.insert("WALL".to_string(), 3);
            cpu.run(&code);
            assert_eq!(cpu.zero, is_wall, "tile {}", tile);
            assert_eq!(u16::from_be_bytes([cpu.h, cpu.l]), wall_addr);
        }
    }

    #[test]
    fn test_memcopy_copies_bc_bytes() {
        let mut asm = Block::new();
        asm.emit_all(cp_in_memory("Data", "$C000")).ret();
        asm.emit_all(memcopy());
        let mut cpu = TestCpu::default();
        cpu.consts16.insert("DataEnd - Data".to_string(), 3);
        for i in 0..4 {
            cpu.mem.insert(format!("Data+{}", i), 10 + i);
        }
        cpu.run(&asm.into_instrs());
        let written: Vec<_> = ["$C000", "$C001", "$C002"]
            .iter()
            .map(|addr| cpu.mem.get(addr).copied())
            .collect();
        assert_eq!(written, [Some(10), Some(11), Some(12)]);
        assert_eq!(cpu.mem.get("$C003"), None, "3 bytes only");
        assert_eq!((cpu.b, cpu.c), (0, 0));
    }

    /// Run `cp_in_memory("Data", "$C000")` then `Memcopy` with a blob of `len` bytes;
    /// the source holds 4 bytes, `Data+0` to `Data+3`
    fn run_memcopy(len: u16) -> TestCpu {
        let mut asm = Block::new();
        asm.emit_all(cp_in_memory("Data", "$C000")).ret();
        asm.emit_all(memcopy());
        let mut cpu = TestCpu::default();
        cpu.consts16.insert("DataEnd - Data".to_string(), len);
        for i in 0..4 {
            cpu.mem.insert(format!("Data+{}", i), 10 + i);
        }
        cpu.run(&asm.into_instrs());
        cpu
    }

    #[test]
    fn test_memcopy_with_length_0_copies_nothing() {
        // B27: Memcopy was a do-while loop, so bc = 0 wrapped to $FFFF and copied
        // 64 KiB over WRAM, the stack and the I/O registers
        let cpu = run_memcopy(0);
        let writes: Vec<_> = cpu
            .trace
            .iter()
            .filter(|event| matches!(event, Event::Write(..)))
            .collect();
        assert!(writes.is_empty(), "nothing is written: {:?}", writes);
        assert_eq!((cpu.b, cpu.c), (0, 0));

        // And every length from 1 on copies exactly that many bytes
        for len in 1..=4u16 {
            let cpu = run_memcopy(len);
            for i in 0..4u16 {
                let want = (i < len).then_some(10 + i as u8);
                assert_eq!(
                    cpu.mem.get(&format!("${:04X}", 0xC000 + i)).copied(),
                    want,
                    "length {}, byte {}",
                    len,
                    i
                );
            }
            assert_eq!((cpu.b, cpu.c), (0, 0), "length {}", len);
        }
    }
}
