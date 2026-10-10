//! A [`hw::Symbol`] in generated code: the conversions that let the builders take one
//!
//! `hw` is pure data and `gb_asm` does not know it, so the conversions live here, in the
//! first layer that depends on both. A symbol becomes the [`Expr`] of its `hardware.inc`
//! name ([`Expr::sym`]), never of its value: `ld_addr_def_a(hw::LCDC)` writes
//! `ld [rLCDC], a`, and `Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON` writes
//! `LCDCF_ON | LCDCF_BGON`. Code that wants the number writes `hw::SCRN_Y.value`.

use crate::gb_asm::{AluOperand, Expr, Operand};
use crate::hw::Symbol;

impl<T> From<Symbol<T>> for Expr {
    fn from(symbol: Symbol<T>) -> Expr {
        Expr::sym(symbol.name)
    }
}

impl<T> From<Symbol<T>> for Operand {
    fn from(symbol: Symbol<T>) -> Operand {
        Operand::from(Expr::from(symbol))
    }
}

impl<T> From<Symbol<T>> for AluOperand {
    fn from(symbol: Symbol<T>) -> AluOperand {
        AluOperand::from(Expr::from(symbol))
    }
}

#[cfg(test)]
mod tests {
    use crate::gb_asm::test_cpu::TestCpu;
    use crate::gb_asm::{Block, Expr, Mem, R8, R16};
    use crate::hw;

    #[test]
    fn test_a_symbol_is_written_by_its_name() {
        let mut asm = Block::new();
        asm.ld_addr_def_a(hw::LCDC)
            .ldh(R8::A, Mem::addr(hw::P1))
            .ld(R8::A, hw::P1F_GET_BTN)
            .and(hw::PADF_LEFT)
            .ld(R16::HL, Expr::from(hw::OAMRAM) + 4)
            .ld(
                R8::A,
                Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON | hw::LCDCF_OBJ16,
            );
        let text: Vec<String> = asm.iter().map(|i| i.to_string()).collect();
        assert_eq!(
            text,
            [
                "ld [rLCDC], a",
                "ldh a, [rP1]",
                "ld a, P1F_GET_BTN",
                "and a, PADF_LEFT",
                "ld hl, _OAMRAM+4",
                "ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJ16",
            ]
        );
    }

    /// On the test CPU, a symbol is the same memory, or constant, as its name
    #[test]
    fn test_a_symbol_runs_as_its_name() {
        let mut asm = Block::new();
        asm.ld(R8::A, Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON)
            .ld_addr_def_a(hw::LCDC)
            .ld(R16::HL, hw::OAMRAM)
            .ld(Mem::Hli, R8::A);
        let mut cpu = TestCpu::default();
        for flag in [hw::LCDCF_ON, hw::LCDCF_BGON] {
            cpu.consts.insert(flag.name.to_string(), flag.value);
        }
        cpu.run(&asm.into_instrs());
        assert_eq!(cpu.mem["rLCDC"], 0x81);
        assert_eq!(cpu.mem["_OAMRAM+0"], 0x81);
    }
}
