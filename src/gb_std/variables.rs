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
    /// Variables as (name, type directive), in declaration order
    pub data: Vec<(String, String)>,
}

impl VariableSection {
    pub fn new(name: &str, memory: &str) -> Self {
        VariableSection {
            name: name.to_string(),
            memory: memory.to_string(),
            data: Vec::new(),
        }
    }

    /// Add a variable; declaring a name again changes its type and keeps its position
    pub fn add_data(&mut self, name: &str, vartype: &str) {
        match self.data.iter_mut().find(|(n, _)| n == name) {
            Some((_, existing)) => *existing = vartype.to_string(),
            None => self.data.push((name.to_string(), vartype.to_string())),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_keeps_declaration_order() {
        let mut section = VariableSection::new("Vars", "WRAM0");
        section.add_data("wB", "db");
        section.add_data("wA", "db");
        section.add_data("wC", "dw");
        // Declaring a name again changes its type but keeps its position
        section.add_data("wA", "dw");

        let lines: Vec<String> = section
            .generate()
            .iter()
            .map(|instr| instr.to_string())
            .collect();
        assert_eq!(
            lines,
            ["SECTION \"Vars\", WRAM0", "wB: db", "wA: dw", "wC: dw"]
        );
    }
}
