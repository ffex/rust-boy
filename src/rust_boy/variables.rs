//! Variable management with automatic WRAM allocation

use std::collections::HashMap;

use crate::gb_asm::{Asm, Instr};

/// Unique identifier for a variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub(crate) usize);

/// A handle to a variable that provides convenient operations.
///
/// This allows writing:
/// ```ignore
/// let ball_momentum_y = gb.vars.create_i8("wBallMomentumY", -1);
///
/// // Set value
/// gb.add_to_main_loop(ball_momentum_y.set(-1));
///
/// // Get value (loads into A)
/// gb.add_to_main_loop(ball_momentum_y.get());
/// ```
#[derive(Debug, Clone)]
pub struct Var {
    name: String,
    var_type: VarType,
}

impl Var {
    /// Set the variable to an immediate value (returns instructions)
    pub fn set(&self, value: i8) -> Vec<Instr> {
        let mut asm = Asm::new();
        if value < 0 {
            asm.ld_a_label(&format!("{}", value));
        } else {
            asm.ld_a(value as u8);
        }
        asm.ld_addr_def_a(&self.name);
        asm.get_main_instrs()
    }

    /// Get the variable value into register A (returns instructions)
    pub fn get(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&self.name);
        asm.get_main_instrs()
    }

    /// Get the variable name/label
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Variable type and size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    /// 1 byte unsigned (db)
    U8,
    /// 2 bytes unsigned (dw)
    U16,
    /// 1 byte signed (db, interpreted as signed)
    I8,
    /// 2 bytes signed (dw, interpreted as signed)
    I16,
}

impl VarType {
    /// Get size in bytes
    pub fn size(&self) -> u16 {
        match self {
            VarType::U8 | VarType::I8 => 1,
            VarType::U16 | VarType::I16 => 2,
        }
    }

    /// Get the assembly directive for this type
    pub fn directive(&self) -> &'static str {
        match self {
            VarType::U8 | VarType::I8 => "db",
            VarType::U16 | VarType::I16 => "dw",
        }
    }
}

/// Internal variable data
#[derive(Debug, Clone)]
pub(crate) struct Variable {
    pub name: String,
    pub var_type: VarType,
    pub initial_value: i32,
    pub wram_address: u16,
    pub section: String, // Section name for grouping
}

/// Manages variables with automatic WRAM allocation
#[derive(Debug)]
pub struct VariableManager {
    variables: HashMap<VarId, Variable>,
    next_id: usize,
    next_wram_addr: u16,
    sections: HashMap<String, Vec<VarId>>,
}

impl VariableManager {
    pub(crate) fn new() -> Self {
        Self {
            variables: HashMap::new(),
            next_id: 0,
            next_wram_addr: 0xC000,
            sections: HashMap::new(),
        }
    }

    /// Create an unsigned 8-bit variable
    pub fn create_u8(&mut self, name: &str, initial: u8) -> Var {
        self.create_var(name, VarType::U8, initial as i32, "Variables")
    }

    /// Create an unsigned 16-bit variable
    pub fn create_u16(&mut self, name: &str, initial: u16) -> Var {
        self.create_var(name, VarType::U16, initial as i32, "Variables")
    }

    /// Create a signed 8-bit variable
    pub fn create_i8(&mut self, name: &str, initial: i8) -> Var {
        self.create_var(name, VarType::I8, initial as i32, "Variables")
    }

    /// Create a signed 16-bit variable
    pub fn create_i16(&mut self, name: &str, initial: i16) -> Var {
        self.create_var(name, VarType::I16, initial as i32, "Variables")
    }

    /// Create a variable in a specific section
    pub fn create_in_section(
        &mut self,
        name: &str,
        var_type: VarType,
        initial: i32,
        section: &str,
    ) -> Var {
        self.create_var(name, var_type, initial, section)
    }

    fn create_var(&mut self, name: &str, var_type: VarType, initial: i32, section: &str) -> Var {
        let addr = self.next_wram_addr;
        self.next_wram_addr += var_type.size();

        let id = VarId(self.next_id);
        self.next_id += 1;

        let var = Variable {
            name: name.to_string(),
            var_type,
            initial_value: initial,
            wram_address: addr,
            section: section.to_string(),
        };

        self.variables.insert(id, var);
        self.sections
            .entry(section.to_string())
            .or_default()
            .push(id);

        Var {
            name: name.to_string(),
            var_type,
        }
    }

    /// Get the assembly label name for a variable
    pub fn get_label(&self, id: VarId) -> Option<&str> {
        self.variables.get(&id).map(|v| v.name.as_str())
    }

    /// Get the WRAM address for a variable
    pub fn get_address(&self, id: VarId) -> Option<u16> {
        self.variables.get(&id).map(|v| v.wram_address)
    }

    /// Get the variable type
    pub fn get_type(&self, id: VarId) -> Option<VarType> {
        self.variables.get(&id).map(|v| v.var_type)
    }

    /// Generate variable section instructions for the Data chunk
    pub(crate) fn generate_sections(&self) -> Vec<Instr> {
        use crate::gb_asm::Asm;

        let mut asm = Asm::new();

        for (section_name, var_ids) in &self.sections {
            asm.section(section_name, "WRAM0");

            for id in var_ids {
                if let Some(var) = self.variables.get(id) {
                    // Format: varName: db or varName: dw
                    asm.raw(&format!("{}: {}", var.name, var.var_type.directive()));
                }
            }
        }

        asm.get_main_instrs()
    }

    /// Generate initialization code for variables with non-zero initial values
    pub(crate) fn generate_init_code(&self) -> Vec<Instr> {
        use crate::gb_asm::Asm;

        let mut asm = Asm::new();

        for var in self.variables.values() {
            // Always initialize variables (even to 0, for clarity)
            match var.var_type {
                VarType::U8 => {
                    let val = var.initial_value as u8;
                    asm.ld_a(val);
                    asm.ld_addr_def_a(&var.name);
                }
                VarType::I8 => {
                    // For signed values, emit negative numbers directly
                    if var.initial_value < 0 {
                        asm.ld_a_label(&format!("{}", var.initial_value));
                    } else {
                        asm.ld_a(var.initial_value as u8);
                    }
                    asm.ld_addr_def_a(&var.name);
                }
                VarType::U16 => {
                    let val = var.initial_value as u16;
                    // Load low byte
                    asm.ld_a((val & 0xFF) as u8);
                    asm.ld_addr_def_a(&var.name);
                    // Load high byte
                    asm.ld_a((val >> 8) as u8);
                    asm.ld_addr_def_a(&format!("{}+1", var.name));
                }
                VarType::I16 => {
                    let val = var.initial_value as i16;
                    let as_u16 = val as u16;
                    // Load low byte
                    let low_byte = (as_u16 & 0xFF) as i8;
                    if low_byte < 0 {
                        asm.ld_a_label(&format!("{}", low_byte));
                    } else {
                        asm.ld_a(low_byte as u8);
                    }
                    asm.ld_addr_def_a(&var.name);
                    // Load high byte
                    let high_byte = (as_u16 >> 8) as i8;
                    if high_byte < 0 {
                        asm.ld_a_label(&format!("{}", high_byte));
                    } else {
                        asm.ld_a(high_byte as u8);
                    }
                    asm.ld_addr_def_a(&format!("{}+1", var.name));
                }
            }
        }

        asm.get_main_instrs()
    }

    /// Check if any variables have been created
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8_variable() {
        let mut vm = VariableManager::new();

        let id = vm.create_u8("wScore", 0);

        assert_eq!(vm.get_label(id), Some("wScore"));
        assert_eq!(vm.get_address(id), Some(0xC000));
        assert_eq!(vm.get_type(id), Some(VarType::U8));
    }

    #[test]
    fn test_multiple_variables() {
        let mut vm = VariableManager::new();

        let id1 = vm.create_u8("wVar1", 0);
        let id2 = vm.create_u16("wVar2", 0);
        let id3 = vm.create_u8("wVar3", 0);

        assert_eq!(vm.get_address(id1), Some(0xC000));
        assert_eq!(vm.get_address(id2), Some(0xC001)); // After 1 byte
        assert_eq!(vm.get_address(id3), Some(0xC003)); // After 2 bytes
    }

    #[test]
    fn test_i8_variable() {
        let mut vm = VariableManager::new();

        let id = vm.create_i8("wMomentum", -1);

        assert_eq!(vm.get_label(id), Some("wMomentum"));
        assert_eq!(vm.get_type(id), Some(VarType::I8));
    }
}
