use crate::gb_asm::{Asm, Instr};

pub fn header_section() -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.section("Header", "ROM0[$100]");
    asm.jp("EntryPoint");
    asm.ds("$150 - @", "0");

    // Entry point
    asm.get_main_instrs()
}
