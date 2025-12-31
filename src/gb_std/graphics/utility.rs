use crate::gb_asm::{Asm, Condition, Instr, Register};

//TODO
// refactor code:
// - punt in the form of builder (like cp_in_memory)

pub fn add_tiles(label: &str, tiles: [[&str; 8]; 10]) -> Vec<Instr> {
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

pub fn add_tiles_2bpp(label: &str, path: &str) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(label);
    asm.incbin(path);
    asm.label(&format!("{}End", label));
    asm.get_main_instrs()
}

pub fn add_tiles_tilemap(label: &str, path: &str) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(label);
    asm.incbin(path);
    asm.label(&format!("{}End", label));
    asm.get_main_instrs()
}

pub fn add_tilemap(label: &str, tilemap: &[[u8; 32]]) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(label);
    for row in tilemap {
        let values: Vec<String> = row.iter().map(|&val| format!("${:02X}", val)).collect();
        asm.db(&values.join(", "));
    }
    asm.label(&format!("{}End", label));
    asm.get_main_instrs()
}
pub fn cp_in_memory(label: &str, addr: &str) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.ld_de_label(label)
        .ld_hl_label(addr)
        .ld_bc_label(&format!("{}End - {}", label, label))
        .call("Memcopy");
    asm.get_main_instrs()
}
// Memcopy function
pub fn memcopy() -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.comment("Copy bytes from one area to another");
    asm.comment("@param de: source");
    asm.comment("@param hl: destination");
    asm.comment("@param bc: length");
    asm.label("Memcopy");
    asm.ld_a_addr_reg(Register::DE);
    asm.ld_hli_label("a");
    asm.inc_label("de");
    asm.dec_label("bc");
    asm.ld_a_label("b");
    asm.or_label("a", "c");
    asm.jp_cond(Condition::NZ, "Memcopy");
    asm.ret();
    asm.get_main_instrs()
}
pub fn turn_off_screen() -> Vec<Instr> {
    let mut asm = Asm::new();
    // Turn off LCD
    asm.ld_a(0).ld_addr_def_a("rLCDC").get_main_instrs()
}

pub fn wait_vblank() -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label("WaitVBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::C, "WaitVBlank");
    asm.get_main_instrs()
}
