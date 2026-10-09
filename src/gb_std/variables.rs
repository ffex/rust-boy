use crate::gb_asm::{Block, Instr};

pub fn def_const(name: &str, value: u8) -> Vec<Instr> {
    //TODO probabibly useful
    let mut asm = Block::new();
    asm.def(name, value);
    asm.into_instrs()
}

pub fn def_var(name: &str, vartype: &str) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.raw(&format!("{}: {}", name, vartype));
    asm.into_instrs()
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
        let mut asm = Block::new();
        asm.section(&self.name, &self.memory);

        for (name, vartype) in &self.data {
            asm.raw(&format!("{}: {}", name, vartype));
        }

        asm.into_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_keeps_declaration_order() {
        let mut section = VariableSection::new("Vars", "WRAM0");
        // Eight names in neither alphabetical nor any hash order (1 chance in 40320)
        for name in ["wG", "wA", "wE", "wH", "wB", "wF", "wC", "wD"] {
            section.add_data(name, "db");
        }
        // Declaring a name again changes its type but keeps its position
        section.add_data("wE", "dw");

        let lines: Vec<String> = section
            .generate()
            .iter()
            .map(|instr| instr.to_string())
            .collect();
        assert_eq!(
            lines,
            [
                "SECTION \"Vars\", WRAM0",
                "wG: db",
                "wA: db",
                "wE: dw",
                "wH: db",
                "wB: db",
                "wF: db",
                "wC: db",
                "wD: db",
            ]
        );
    }
}
