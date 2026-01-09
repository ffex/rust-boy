use super::asm::{Asm, Chunk};
use super::instr::{Condition, Instr, JumpTarget, Operand, Register};
use std::fmt;

// Code generation implementation for Asm
impl Asm {
    /// Get the instructions from the Main chunk as an owned vector
    /// Returns an empty vector if the Main chunk has no instructions
    pub fn get_main_instrs(&self) -> Vec<Instr> {
        self.chunks.get(&Chunk::Main).cloned().unwrap_or_default()
    }

    /// Generate assembly code from the instruction chunks
    pub fn to_asm(&self) -> String {
        let mut asm = String::new();

        // Define the order in which chunks should appear in the output
        let chunk_order = [
            Chunk::Main,
            Chunk::Functions,
            Chunk::Data,
            Chunk::Tiles,
            Chunk::Tilemap,
        ];

        for chunk in &chunk_order {
            if let Some(instructions) = self.chunks.get(chunk) {
                if !instructions.is_empty() {
                    // Write instructions with indentation
                    for instruction in instructions {
                        asm.push_str(&format!("    {}\n", instruction));
                    }

                    // Add blank line between chunks
                    asm.push('\n');
                }
            }
        }

        asm
    }
}
// Display implementation for Register
impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Register::A => write!(f, "a"),
            Register::B => write!(f, "b"),
            Register::C => write!(f, "c"),
            Register::D => write!(f, "d"),
            Register::E => write!(f, "e"),
            Register::H => write!(f, "h"),
            Register::L => write!(f, "l"),
            Register::SP => write!(f, "sp"),
            Register::PC => write!(f, "pc"),
            Register::AF => write!(f, "af"),
            Register::BC => write!(f, "bc"),
            Register::DE => write!(f, "de"),
            Register::HL => write!(f, "hl"),
        }
    }
}

// Display implementation for Operand
impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Reg(reg) => write!(f, "{}", reg),
            Operand::Imm(val) => write!(f, "{}", val),
            Operand::Imm16(val) => write!(f, "{}", val),
            Operand::Addr(addr) => write!(f, "[${:04x}]", addr),
            Operand::AddrDef(const_name) => write!(f, "[{}]", const_name),
            Operand::AddrReg(reg) => write!(f, "[{}]", reg),
            Operand::AddrRegInc(reg) => write!(f, "[{}i]", reg),
            Operand::Label(label) => write!(f, "{}", label),
        }
    }
}

// Display implementation for JumpTarget
impl fmt::Display for JumpTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JumpTarget::Label(label) => write!(f, "{}", label),
            JumpTarget::Addr(addr) => write!(f, "${:04x}", addr),
        }
    }
}

// Display implementation for Condition
impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Condition::Z => write!(f, "z"),
            Condition::NZ => write!(f, "nz"),
            Condition::C => write!(f, "c"),
            Condition::NC => write!(f, "nc"),
        }
    }
}

// Display implementation for Instr
impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Load instructions
            Instr::Ld { dst, src } => write!(f, "ld {}, {}", dst, src),
            Instr::Ldh { dst, src } => write!(f, "ldh {}, {}", dst, src),

            // Arithmetic instructions
            Instr::Add { dst, src } => write!(f, "add {}, {}", dst, src),
            Instr::AdcA { operand } => write!(f, "adc {}", operand),
            Instr::Adc { dst, src } => write!(f, "adc {}, {}", dst, src),
            Instr::Sub { dst, src } => write!(f, "sub {}, {}", dst, src),
            Instr::Inc { operand } => write!(f, "inc {}", operand),
            Instr::Dec { operand } => write!(f, "dec {}", operand),

            // Logical instructions
            Instr::And { operand } => write!(f, "and a, {}", operand),
            Instr::Or { dst, src } => write!(f, "or {}, {}", dst, src),
            Instr::Xor { dst, src } => write!(f, "xor {}, {}", dst, src),
            Instr::Cp { operand } => write!(f, "cp {}", operand),

            // Bit shift instructions
            Instr::Srl { operand } => write!(f, "srl {}", operand),
            Instr::Swap { operand } => write!(f, "swap {}", operand),

            // Misc instructions
            Instr::Daa => write!(f, "daa"),

            // Jump instructions
            Instr::Jp { target } => write!(f, "jp {}", target),
            Instr::JpCond { condition, target } => write!(f, "jp {}, {}", condition, target),
            Instr::Jr { target } => write!(f, "jr {}", target),
            Instr::JrCond { condition, target } => write!(f, "jr {}, {}", condition, target),
            Instr::Call { target } => write!(f, "call {}", target),
            Instr::Ret => write!(f, "ret"),
            Instr::RetCond { condition } => write!(f, "ret {}", condition),

            // Assembler directives
            Instr::Ds {
                num_bytes,
                starter_point,
            } => write!(f, "ds {}, {}", num_bytes, starter_point),
            Instr::Include { file } => write!(f, "INCLUDE \"{}\"", file),
            Instr::Incbin {
                file,
                offset,
                length,
            } => match (offset, length) {
                (Some(off), Some(len)) => write!(f, "INCBIN \"{}\",{},{}", file, off, len),
                (Some(off), None) => write!(f, "INCBIN \"{}\",{}", file, off),
                (None, Some(len)) => write!(f, "INCBIN \"{}\",0,{}", file, len),
                (None, None) => write!(f, "INCBIN \"{}\"", file),
            },
            Instr::Def { label, value } => write!(f, "DEF {} EQU {}", label, value),
            Instr::Section { name, mem_type } => write!(f, "SECTION \"{}\", {}", name, mem_type),
            Instr::Label { name } => write!(f, "{}:", name),
            Instr::Comment { text } => write!(f, "; {}", text),
            Instr::Db { values } => write!(f, "db {}", values),
            Instr::Dw { value } => write!(f, "dw {}", value),
            Instr::Raw { line } => write!(f, "{}", line),
        }
    }
}
