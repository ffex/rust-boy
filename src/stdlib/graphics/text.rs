//! Contiguous single-tile fonts and resumable VRAM-queue printing.
use super::vram_queue;
use crate::asm::{Block, Condition, Mem, R8, R16, R16Stack};
use crate::stdlib::routine::{Regs, Routine};

/// A contiguous range of ASCII glyphs, each occupying one background tile.
#[derive(Debug, Clone, Copy)]
pub struct Font {
    first: u8,
    count: u16,
    tile: u8,
}
impl Font {
    /// Panics for an empty or overflowing character/tile range.
    pub fn new(first: u8, count: u16, tile: u8) -> Self {
        assert!(
            count > 0 && u16::from(first) + count <= 128,
            "font must contain ASCII glyphs"
        );
        assert!(u16::from(tile) + count <= 256, "font tile range overflows");
        Self { first, count, tile }
    }
    /// The tile for a supported ASCII character; panics otherwise.
    pub fn glyph(self, ch: char) -> u8 {
        let value = ch as u32;
        assert!(
            value >= u32::from(self.first) && value < u32::from(self.first) + u32::from(self.count),
            "unsupported font character {ch:?}"
        );
        self.tile + (value as u8 - self.first)
    }
    /// Resolve text into tilemap writes. Newlines and wrapping return to the original
    /// X column. Panics before returning if a glyph falls outside the 32×32 map.
    pub fn layout(self, base: u16, x: u8, y: u8, text: &str) -> Vec<(u16, u8)> {
        assert!(
            base == crate::hw::SCRN0.value || base == crate::hw::SCRN1.value,
            "invalid tilemap base"
        );
        assert!(x < 32 && y < 32, "text origin outside tilemap");
        let (mut col, mut row) = (u16::from(x), u16::from(y));
        let mut writes = Vec::new();
        for ch in text.chars() {
            if ch == '\n' {
                col = u16::from(x);
                row += 1;
                continue;
            }
            if col == 32 {
                col = u16::from(x);
                row += 1;
            }
            assert!(row < 32, "text extends beyond tilemap");
            writes.push((base + row * 32 + col, self.glyph(ch)));
            col += 1;
        }
        writes
    }
}

/// Queue glyphs at ROM word-array `targets`, from ROM/WRAM byte-array `glyphs`.
/// `cursor` is a zero-initialized WRAM word. Carry means queue full: call again next
/// frame. Carry clear means all glyphs are queued (not necessarily flushed). Completed
/// printers stay complete until the cursor is reset. Changes all registers; main/init
/// producer only. Interrupts may safely flush the queue between instructions.
pub fn print(name: &str, targets: &str, glyphs: &str, cursor: &str, len: u16) -> Routine {
    let mut code = Block::new();
    code.label(name)
        .label(".next")
        .ld_a_addr_def(cursor)
        .ld(R8::E, R8::A)
        .ld(R8::A, Mem::addr(crate::asm::Expr::sym(cursor) + 1))
        .ld(R8::D, R8::A)
        .cp((len >> 8) as u8)
        .jr_cond(Condition::C, ".write")
        .jr_cond(Condition::NZ, ".done")
        .ld(R8::A, R8::E)
        .cp(len as u8)
        .jr_cond(Condition::NC, ".done")
        .label(".write")
        .push(R16Stack::DE)
        .ld(R16::HL, targets)
        .add_hl(R16::DE)
        .add_hl(R16::DE)
        .ld(R8::A, Mem::Hli)
        .ld(R8::C, R8::A)
        .ld(R8::B, R8::AtHl)
        .push(R16Stack::BC)
        .ld(R16::HL, glyphs)
        .add_hl(R16::DE)
        .ld(R8::A, R8::AtHl)
        .pop(R16Stack::HL)
        .call("QueueVramWrite")
        .pop(R16Stack::DE)
        .ret_cond(Condition::C)
        .inc(R16::DE)
        .ld(R8::A, R8::E)
        .ld_addr_def_a(cursor)
        .ld(R8::A, R8::D)
        .ld(Mem::addr(crate::asm::Expr::sym(cursor) + 1), R8::A)
        .jr(".next")
        .label(".done")
        .xor(R8::A)
        .ret();
    Routine::new(name, code)
        .with_dep(vram_queue::enqueue())
        .with_clobbers(Regs::ALL)
}

/// DE = unsigned number, HL = five-byte destination, C = tile for '0'. Writes five
/// decimal digits with leading zeroes. Changes AF/BC/DE/HL. Does not access VRAM.
pub fn decimal() -> Routine {
    let mut code = Block::new();
    code.label("EncodeDecimalU16");
    for divisor in [10000u16, 1000, 100, 10, 1] {
        let again = format!(".digit{divisor}");
        let done = format!(".end{divisor}");
        code.ld(R8::B, 0)
            .label(&again)
            .ld(R8::A, R8::E)
            .sub(divisor as u8)
            .ld(R8::E, R8::A)
            .ld(R8::A, R8::D)
            .sbc((divisor >> 8) as u8)
            .jr_cond(Condition::C, &done)
            .ld(R8::D, R8::A)
            .inc(R8::B)
            .jr(&again)
            .label(&done)
            .ld(R8::A, R8::E)
            .add(divisor as u8)
            .ld(R8::E, R8::A)
            .ld(R8::A, R8::B)
            .add(R8::C)
            .ld(Mem::Hli, R8::A);
    }
    code.and(R8::A).ret();
    Routine::new("EncodeDecimalU16", code)
        .with_reads(Regs::DE | Regs::HL | Regs::C)
        .with_clobbers(Regs::ALL)
}

/// DE = four packed BCD digits, HL = four-byte destination, C = tile for '0'.
/// Invalid BCD returns carry without writing; otherwise carry clears. Changes AF/HL.
pub fn bcd() -> Routine {
    let mut code = Block::new();
    code.label("EncodePackedBcd");
    for register in [R8::D, R8::E] {
        for high in [true, false] {
            code.ld(R8::A, register);
            if high {
                code.swap(R8::A);
            }
            code.and(15).cp(10).jr_cond(Condition::NC, ".invalid");
        }
    }
    for register in [R8::D, R8::E] {
        for high in [true, false] {
            code.ld(R8::A, register);
            if high {
                code.swap(R8::A);
            }
            code.and(15).add(R8::C).ld(Mem::Hli, R8::A);
        }
    }
    code.and(R8::A).ret().label(".invalid").scf().ret();
    Routine::new("EncodePackedBcd", code)
        .with_reads(Regs::DE | Regs::HL | Regs::C)
        .with_clobbers(Regs::AF | Regs::HL)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_wraps_and_checks_ranges() {
        let font = Font::new(b'0', 10, 7);
        assert_eq!(
            font.layout(crate::hw::SCRN1.value, 30, 1, "012\n9"),
            [(0x9c3e, 7), (0x9c3f, 8), (0x9c5e, 9), (0x9c7e, 16)]
        );
        for text in ["a", "é", "00"] {
            assert!(
                std::panic::catch_unwind(|| font.layout(crate::hw::SCRN0.value, 31, 31, text))
                    .is_err()
            );
        }
        assert!(font.layout(crate::hw::SCRN0.value, 0, 0, "").is_empty());
    }
}
