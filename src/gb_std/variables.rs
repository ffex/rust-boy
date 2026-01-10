use std::collections::HashMap;

use crate::gb_asm::{Asm, Instr};

pub fn def_const(name: &str, value: u8) -> Vec<Instr> {
    //TODO probabibly useful
    let mut asm = Asm::new();
    asm.def(name, value);
    asm.get_main_instrs()
}

pub fn def_var(name: &str, vartype: &str) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.raw(&format!("{}: {}", name, vartype));
    asm.get_main_instrs()
}

pub struct VariableSection {
    pub name: String,
    pub memory: String,
    pub data: HashMap<String, String>,
}

impl VariableSection {
    pub fn new(name: &str, memory: &str) -> Self {
        VariableSection {
            name: name.to_string(),
            memory: memory.to_string(),
            data: HashMap::new(),
        }
    }

    pub fn add_data(&mut self, name: &str, vartype: &str) {
        self.data.insert(name.to_string(), vartype.to_string());
    }

    pub fn generate(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.section(&self.name, &self.memory);

        for (name, vartype) in &self.data {
            asm.raw(&format!("{}: {}", name, vartype));
        }

        asm.get_main_instrs()
    }
}
