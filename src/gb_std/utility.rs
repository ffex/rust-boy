use crate::gb_asm::{Block, Instr};

pub fn header_section() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.section("Header", "ROM0[$100]");
    asm.jp("EntryPoint");
    asm.ds("$150 - @", "0");

    // Entry point
    asm.into_instrs()
}
