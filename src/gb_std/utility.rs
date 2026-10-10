use crate::gb_asm::{Block, Instr, Section};

pub fn header_section() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.section(Section::rom0("Header").at(0x0100));
    asm.jp("EntryPoint");
    asm.ds_fill("$150 - @", "0");

    // Entry point
    asm.into_instrs()
}
