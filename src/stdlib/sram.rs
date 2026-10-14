//! Low-level cartridge SRAM control for MBC1/MBC5 cartridges.

use crate::asm::{Block, R8};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

/// Enable external cartridge RAM on an MBC1 or MBC5 cartridge.
///
/// SRAM in `$A000..=$BFFF` can be accessed after this routine runs. Reads and writes are
/// ordinary memory operations; callers must disable SRAM before switching cartridges or
/// entering code that expects it off. Clobbers `a` and flags.
pub fn enable() -> Routine {
    let mut code = Block::new();
    code.label("EnableCartridgeRam")
        .ld_a(0x0A)
        .ld_addr_def_a(hw::CARTRIDGE_RAM_GATE)
        .ret();
    Routine::new("EnableCartridgeRam", code).with_clobbers(Regs::A | Regs::F)
}

/// Disable external cartridge RAM on an MBC1 or MBC5 cartridge.
///
/// Clobbers `a` and flags.
pub fn disable() -> Routine {
    let mut code = Block::new();
    code.label("DisableCartridgeRam")
        .ld_a(0)
        .ld_addr_def_a(hw::CARTRIDGE_RAM_GATE)
        .ret();
    Routine::new("DisableCartridgeRam", code).with_clobbers(Regs::A | Regs::F)
}

/// Select an MBC5 cartridge RAM bank from `hl`; only the low four bits are used.
///
/// The selected 8 KiB bank is visible at `$A000..=$BFFF`. Enable SRAM separately with
/// [`enable`]. Reads and preserves `hl`; clobbers `a` and flags. MBC1 banking mode is not
/// changed by this helper.
pub fn select_mbc5_bank() -> Routine {
    let mut code = Block::new();
    code.label("SelectMbc5RamBank")
        .ld(R8::A, R8::L)
        .and(0x0F)
        .ld_addr_def_a(hw::MBC5_RAM_BANK)
        .ret();
    Routine::new("SelectMbc5RamBank", code)
        .with_reads(Regs::HL)
        .with_clobbers(Regs::A | Regs::F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;

    #[test]
    fn gate_enables_and_disables_cartridge_ram() {
        let mut cpu = TestCpu::default();
        cpu.run(enable().body());
        assert_eq!(cpu.mem.get("$0000"), Some(&0x0A));
        cpu.run(disable().body());
        assert_eq!(cpu.mem.get("$0000"), Some(&0));
    }

    #[test]
    fn mbc5_ram_bank_selection_masks_to_four_bits() {
        let mut cpu = TestCpu::default();
        [cpu.h, cpu.l] = 0xABCDu16.to_be_bytes();
        cpu.run(select_mbc5_bank().body());
        assert_eq!(cpu.mem.get("$4000"), Some(&0x0D));
        assert_eq!(u16::from_be_bytes([cpu.h, cpu.l]), 0xABCD);
    }
}
