use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

//TODO
// refactor code:
// - punt in the form of builder (like cp_in_memory)

pub fn add_tiles(label: &str, tiles: &[[&str; 8]]) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(label);
    for tile in tiles {
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

pub fn turn_on_screen() -> Vec<Instr> {
    let mut asm = Asm::new();
    // Turn on LCD
    asm.ld_a_label("LCDCF_ON | LCDCF_BGON | LCDCF_OBJON")
        .ld_addr_def_a("rLCDC")
        .get_main_instrs()
}

pub fn wait_vblank() -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label("WaitVBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::C, "WaitVBlank");
    asm.ret();
    asm.get_main_instrs()
}
pub fn wait_not_vblank() -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label("WaitNotVBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::NC, "WaitNotVBlank");
    asm.ret();
    asm.get_main_instrs()
}

// Convert a pixel position to a tilemap address
// hl = $9800 + X + Y * 32
// @param b: X
// @param c: Y
// @return hl: tile address
pub fn get_tile_by_pixel() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.comment("Convert a pixel position to a tilemap address");
    asm.comment("hl = $9800 + X + Y * 32");
    asm.comment("@param b: X");
    asm.comment("@param c: Y");
    asm.comment("@return hl: tile address");
    asm.label("GetTileByPixel");

    // First, we need to divide by 8 to convert a pixel position to a tile position.
    // After this we want to multiply the Y position by 32.
    // These operations effectively cancel out so we only need to mask the Y value.
    asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::C));
    asm.and(Operand::Imm(0b11111000));
    asm.ld(Operand::Reg(Register::L), Operand::Reg(Register::A));
    asm.ld(Operand::Reg(Register::H), Operand::Imm(0));

    // Now we have the position * 8 in hl
    asm.add(Operand::Reg(Register::HL), Operand::Reg(Register::HL)); // position * 16
    asm.add(Operand::Reg(Register::HL), Operand::Reg(Register::HL)); // position * 32

    // Convert the X position to an offset.
    asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.srl(Operand::Reg(Register::A)); // a / 2
    asm.srl(Operand::Reg(Register::A)); // a / 4
    asm.srl(Operand::Reg(Register::A)); // a / 8

    // Add the two offsets together.
    asm.add(Operand::Reg(Register::A), Operand::Reg(Register::L));
    asm.ld(Operand::Reg(Register::L), Operand::Reg(Register::A));
    asm.adc(Operand::Reg(Register::A), Operand::Reg(Register::H));
    asm.sub(Operand::Reg(Register::A), Operand::Reg(Register::L));
    asm.ld(Operand::Reg(Register::H), Operand::Reg(Register::A));

    // Add the offset to the tilemap's base address, and we are done!
    asm.ld_bc(0x9800);
    asm.add(Operand::Reg(Register::HL), Operand::Reg(Register::BC));
    asm.ret();

    asm.get_main_instrs()
}

pub fn is_specific_tile(label: &str, tiles_ids: &[&str]) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(label);
    for tile_id in tiles_ids {
        asm.cp_label(tile_id); //TODO understand the tile id and how to manage it!
        asm.ret_cond(Condition::Z);
    }
    asm.ret();
    asm.get_main_instrs()
}
