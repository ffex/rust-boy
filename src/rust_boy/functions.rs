//! Builtin function registry for auto-inclusion

use std::collections::HashSet;

use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

/// Builtin functions that can be auto-included
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinFunction {
    /// Memory copy routine
    Memcopy,
    /// Wait for VBlank
    WaitVBlank,
    /// Wait for not VBlank
    WaitNotVBlank,
    /// Update keyboard input state
    UpdateKeys,
    /// Convert pixel position to tile address
    GetTileByPixel,
}

impl BuiltinFunction {
    /// Get the label name for this function
    pub fn label(&self) -> &'static str {
        match self {
            BuiltinFunction::Memcopy => "Memcopy",
            BuiltinFunction::WaitVBlank => "WaitVBlank",
            BuiltinFunction::WaitNotVBlank => "WaitNotVBlank",
            BuiltinFunction::UpdateKeys => "UpdateKeys",
            BuiltinFunction::GetTileByPixel => "GetTileByPixel",
        }
    }

    /// Generate the assembly instructions for this function
    pub fn generate(&self) -> Vec<Instr> {
        match self {
            BuiltinFunction::Memcopy => generate_memcopy(),
            BuiltinFunction::WaitVBlank => generate_wait_vblank(),
            BuiltinFunction::WaitNotVBlank => generate_wait_not_vblank(),
            BuiltinFunction::UpdateKeys => generate_update_keys(),
            BuiltinFunction::GetTileByPixel => generate_get_tile_by_pixel(),
        }
    }
}

/// Registry for tracking which functions are used
#[derive(Debug, Default)]
pub struct FunctionRegistry {
    used: HashSet<BuiltinFunction>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a function as used
    pub fn use_function(&mut self, func: BuiltinFunction) {
        self.used.insert(func);
    }

    /// Check if a function is used
    pub fn is_used(&self, func: BuiltinFunction) -> bool {
        self.used.contains(&func)
    }

    /// Generate all used functions
    pub fn generate_all(&self) -> Vec<Instr> {
        let mut all_instrs = Vec::new();

        for func in &self.used {
            all_instrs.extend(func.generate());
        }

        all_instrs
    }
}

// Function implementations

fn generate_memcopy() -> Vec<Instr> {
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

fn generate_wait_vblank() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.label("WaitVBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::C, "WaitVBlank");
    asm.ret();

    asm.get_main_instrs()
}

fn generate_wait_not_vblank() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.label("WaitNotVBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::NC, "WaitNotVBlank");
    asm.ret();

    asm.get_main_instrs()
}

fn generate_update_keys() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.label("UpdateKeys");
    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("P1F_GET_BTN".to_string()),
    );
    asm.call(".onenibble");
    asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));

    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("P1F_GET_DPAD".to_string()),
    );
    asm.call(".onenibble");
    asm.swap(Operand::Reg(Register::A));
    asm.xor(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));

    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("P1F_GET_NONE".to_string()),
    );
    asm.ldh(
        Operand::AddrDef("rP1".to_string()),
        Operand::Reg(Register::A),
    );

    asm.ld(
        Operand::Reg(Register::A),
        Operand::AddrDef("wCurKeys".to_string()),
    );
    asm.xor(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.and(Operand::Reg(Register::B));
    asm.ld(
        Operand::AddrDef("wNewKeys".to_string()),
        Operand::Reg(Register::A),
    );
    asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.ld(
        Operand::AddrDef("wCurKeys".to_string()),
        Operand::Reg(Register::A),
    );
    asm.ret();

    asm.label(".onenibble");
    asm.ldh(
        Operand::AddrDef("rP1".to_string()),
        Operand::Reg(Register::A),
    );
    asm.call(".knowret");
    asm.ldh(
        Operand::Reg(Register::A),
        Operand::AddrDef("rP1".to_string()),
    );
    asm.ldh(
        Operand::Reg(Register::A),
        Operand::AddrDef("rP1".to_string()),
    );
    asm.ldh(
        Operand::Reg(Register::A),
        Operand::AddrDef("rP1".to_string()),
    );
    asm.or(Operand::Reg(Register::A), Operand::Imm(0xF0));

    asm.label(".knowret");
    asm.ret();

    asm.get_main_instrs()
}

fn generate_get_tile_by_pixel() -> Vec<Instr> {
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
    asm.ld_bc_label("$9800");
    asm.add(Operand::Reg(Register::HL), Operand::Reg(Register::BC));
    asm.ret();

    asm.get_main_instrs()
}
