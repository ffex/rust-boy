use crate::asm::{Block, Condition, Instr, R8, R16, Section};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

/// The cartridge header: `SECTION "Header", ROM0[$100]`, `jp EntryPoint`, then padding to
/// `$150` (`ds $150 - @, 0`), which `rgbfix` fills in
pub fn header_section() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.section(Section::rom0("Header").at(hw::ROM_HEADER));
    asm.jp("EntryPoint");
    asm.ds_fill(&format!("${:X} - @", hw::ROM_HEADER_END), "0");

    // Entry point
    asm.into_instrs()
}

/// The `Delay` routine: a busy loop that counts `bc` down
///
/// It runs its loop `bc + 1` times (the loop tests `bc` before it decrements it), about
/// 7 M-cycles each.
/// - Reads: `bc`, the count (higher = longer).
/// - Returns nothing.
/// - Clobbers: `a`, `bc` (`$FFFF` on return) and the flags.
pub fn delay() -> Routine {
    let mut asm = Block::new();

    asm.comment("Delay loop using BC as counter");
    asm.comment("@param bc: delay counter (higher = longer delay)");
    asm.label("Delay");
    asm.ld(R8::A, R8::B);
    asm.or(R8::C);
    asm.dec(R16::BC);
    asm.jr_cond(Condition::NZ, "Delay");
    asm.ret();

    Routine::new("Delay", asm)
        .with_reads(Regs::BC)
        .with_clobbers(Regs::A | Regs::BC | Regs::F)
}
