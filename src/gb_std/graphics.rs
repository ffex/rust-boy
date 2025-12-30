use crate::gb_asm::{Asm, Instr};

pub fn add_tiles(tiles: [[&str; 8]; 10], label: &str) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(label);
    for tile in &tiles {
        for line in tile {
            asm.dw(line);
        }
    }
    asm.label(&format!("{}End", label));
    asm.get_main_instrs()
}
