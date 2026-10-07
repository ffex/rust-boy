//! Builtin function registry for auto-inclusion

use std::collections::BTreeSet;

use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

/// Builtin functions that can be auto-included
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// Delay loop using BC as counter
    Delay,
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
            BuiltinFunction::Delay => "Delay",
        }
    }

    /// Try to get a BuiltinFunction from its label name
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Memcopy" => Some(BuiltinFunction::Memcopy),
            "WaitVBlank" => Some(BuiltinFunction::WaitVBlank),
            "WaitNotVBlank" => Some(BuiltinFunction::WaitNotVBlank),
            "UpdateKeys" => Some(BuiltinFunction::UpdateKeys),
            "GetTileByPixel" => Some(BuiltinFunction::GetTileByPixel),
            "Delay" => Some(BuiltinFunction::Delay),
            _ => None,
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
            BuiltinFunction::Delay => generate_delay(),
        }
    }
}

/// Registry for tracking which functions are used (both builtin and user-defined)
#[derive(Default)]
pub struct FunctionRegistry {
    /// Builtin functions that have been marked as used (emitted in enum order)
    used_builtins: BTreeSet<BuiltinFunction>,
    /// User-defined functions as (name, instructions), in registration order
    user_functions: Vec<(String, Vec<Instr>)>,
    /// User functions that have been called (for validation)
    used_user_functions: BTreeSet<String>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a builtin function as used
    pub fn use_function(&mut self, func: BuiltinFunction) {
        self.used_builtins.insert(func);
    }

    /// Check if a builtin function is used
    pub fn is_used(&self, func: BuiltinFunction) -> bool {
        self.used_builtins.contains(&func)
    }

    /// Register a user-defined function
    ///
    /// Registering the same name again replaces the body and keeps its position.
    pub fn register_user_function(&mut self, name: &str, body: Vec<Instr>) {
        match self.user_functions.iter_mut().find(|(n, _)| n == name) {
            Some((_, existing)) => *existing = body,
            None => self.user_functions.push((name.to_string(), body)),
        }
    }

    fn has_user_function(&self, name: &str) -> bool {
        self.user_functions.iter().any(|(n, _)| n == name)
    }

    /// Check if a function exists (builtin or user-defined)
    pub fn function_exists(&self, name: &str) -> bool {
        BuiltinFunction::from_name(name).is_some() || self.has_user_function(name)
    }

    /// Mark a function as called and auto-register if builtin
    /// Returns true if the function exists, false otherwise
    pub fn call_function(&mut self, name: &str) -> bool {
        // Check if it's a builtin function
        if let Some(builtin) = BuiltinFunction::from_name(name) {
            self.used_builtins.insert(builtin);
            return true;
        }

        // Check if it's a user-defined function
        if self.has_user_function(name) {
            self.used_user_functions.insert(name.to_string());
            return true;
        }

        false
    }

    /// Get list of all registered function names (for error messages)
    pub fn available_functions(&self) -> Vec<String> {
        let mut names: Vec<String> = vec![
            "Memcopy".to_string(),
            "WaitVBlank".to_string(),
            "WaitNotVBlank".to_string(),
            "UpdateKeys".to_string(),
            "GetTileByPixel".to_string(),
            "Delay".to_string(),
        ];
        names.extend(self.user_functions.iter().map(|(n, _)| n.clone()));
        names.sort();
        names
    }

    /// Generate all used functions (builtin and user-defined)
    pub fn generate_all(&self) -> Vec<Instr> {
        let mut all_instrs = Vec::new();

        // Generate builtin functions
        for func in &self.used_builtins {
            all_instrs.extend(func.generate());
        }

        // Generate user-defined functions
        for (_, body) in &self.user_functions {
            all_instrs.extend(body.clone());
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

    asm.ld_a_addr_reg(Register::HL); //done to help, fit good in unbreaked
    asm.ret();

    asm.get_main_instrs()
}

fn generate_delay() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.comment("Delay loop using BC as counter");
    asm.comment("@param bc: delay counter (higher = longer delay)");
    asm.label("Delay");
    asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.or(Operand::Reg(Register::A), Operand::Reg(Register::C));
    asm.dec(Operand::Reg(Register::BC));
    asm.jr_cond(Condition::NZ, "Delay");
    asm.ret();

    asm.get_main_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_body(label: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.label(label).ret();
        asm.get_main_instrs()
    }

    #[test]
    fn test_user_functions_keep_registration_order() {
        let mut registry = FunctionRegistry::new();
        registry.register_user_function("Second", function_body("Second"));
        registry.register_user_function("First", function_body("First"));
        // Registering a name again replaces the body but keeps its position
        registry.register_user_function("Second", function_body("SecondV2"));

        let lines: Vec<String> = registry
            .generate_all()
            .iter()
            .map(|instr| instr.to_string())
            .collect();
        assert_eq!(lines, ["SecondV2:", "ret", "First:", "ret"]);
        assert!(registry.function_exists("First"));
        assert!(!registry.function_exists("Missing"));
    }
}
