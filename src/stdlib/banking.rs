//! MBC1 and MBC5 ROM bank selection routines.

use crate::asm::{Block, Condition, R8};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

/// Select an MBC1 ROM bank from `hl` (bank number 0–127).
///
/// MBC1 remaps a bank whose low five bits are zero to the following bank in the same
/// 32-bank group. Thus requests for banks `$00`, `$20`, `$40` and `$60` select `$01`, `$21`,
/// `$41` and `$61`. Use `Section::romx(...).bank(n)` to place data in a ROMX bank. This
/// routine must only be used with an MBC1 cartridge in its default ROM banking mode.
///
/// Reads and preserves `hl`; clobbers `a` and flags.
pub fn set_mbc1_rom_bank() -> Routine {
    let mut code = Block::new();
    code.label("SetMbc1RomBank")
        .ld(R8::A, R8::L)
        .and(0x1F)
        .jr_cond(Condition::NZ, ".low_ready")
        .inc(R8::A)
        .label(".low_ready")
        .ld_addr_def_a(hw::MBC1_ROM_BANK_LOW)
        .ld(R8::A, R8::L)
        .swap(R8::A)
        .srl(R8::A)
        .and(0x03)
        .ld_addr_def_a(hw::MBC1_ROM_BANK_HIGH)
        .ret();
    Routine::new("SetMbc1RomBank", code)
        .with_reads(Regs::HL)
        .with_clobbers(Regs::A | Regs::F)
}

/// Select an MBC5 ROM bank from `hl` (bank number 0–511).
///
/// Bank zero is valid on MBC5. Use `Section::romx(...).bank(n)` to place data in a ROMX
/// bank. Reads and preserves `hl`; clobbers `a` and flags.
pub fn set_mbc5_rom_bank() -> Routine {
    let mut code = Block::new();
    code.label("SetMbc5RomBank")
        .ld(R8::A, R8::L)
        .ld_addr_def_a(hw::MBC5_ROM_BANK_LOW)
        .ld(R8::A, R8::H)
        .and(1)
        .ld_addr_def_a(hw::MBC5_ROM_BANK_HIGH)
        .ret();
    Routine::new("SetMbc5RomBank", code)
        .with_reads(Regs::HL)
        .with_clobbers(Regs::A | Regs::F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;

    fn run(routine: Routine, bank: u16) -> TestCpu {
        let mut cpu = TestCpu::default();
        [cpu.h, cpu.l] = bank.to_be_bytes();
        cpu.run(routine.body());
        cpu
    }

    #[test]
    fn mbc1_splits_bits_and_avoids_forbidden_low_banks() {
        for (bank, low, high) in [(0, 1, 0), (0x20, 1, 1), (0x41, 1, 2), (0x7F, 0x1F, 3)] {
            let cpu = run(set_mbc1_rom_bank(), bank);
            assert_eq!(cpu.mem.get("$2000"), Some(&low));
            assert_eq!(cpu.mem.get("$4000"), Some(&high));
            assert_eq!(u16::from_be_bytes([cpu.h, cpu.l]), bank);
        }
    }

    #[test]
    fn mbc5_writes_nine_bank_bits_including_bank_zero() {
        for (bank, low, high) in [(0, 0, 0), (0xFF, 0xFF, 0), (0x100, 0, 1), (0x1FF, 0xFF, 1)] {
            let cpu = run(set_mbc5_rom_bank(), bank);
            assert_eq!(cpu.mem.get("$2000"), Some(&low));
            assert_eq!(cpu.mem.get("$3000"), Some(&high));
            assert_eq!(u16::from_be_bytes([cpu.h, cpu.l]), bank);
        }
    }
}
