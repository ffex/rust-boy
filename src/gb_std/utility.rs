use crate::gb_asm::{Block, Instr, Section};
use crate::hw;

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
