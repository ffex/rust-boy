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
            Chunk::Header,    // INCLUDE, SECTION Header
            Chunk::Constants, // DEF statements
            Chunk::Init,      // Initialization code
            Chunk::MainLoop,  // Main game loop
            Chunk::Main,      // Legacy (backwards compatibility)
            Chunk::Functions, // Function definitions
            Chunk::Tiles,     // Tile data
            Chunk::Tilemap,   // Tilemap data
            Chunk::Data,      // Variables (WRAM sections)
        ];

        for chunk in &chunk_order {
            if let Some(instructions) = self.chunks.get(chunk).filter(|i| !i.is_empty()) {
                // Write instructions with indentation
                for instruction in instructions {
                    asm.push_str(&format!("    {}\n", instruction));
                }

                // Add blank line between chunks
                asm.push('\n');
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
            Operand::AddrRegDec(reg) => write!(f, "[{}d]", reg),
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

/// `sp + offset` / `sp - offset`, as `ld hl, sp + e8` writes it
fn sp_offset(offset: i8) -> String {
    if offset < 0 {
        format!("sp - {}", offset.unsigned_abs())
    } else {
        format!("sp + {}", offset)
    }
}

// Display implementation for Instr: the RGBDS syntax of gbz80(7), the 8-bit ALU
// instructions with their explicit `a` (`cp a, 5`). Panics on an instruction that
// `Instr::check` rejects.
impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Err(error) = self.check() {
            panic!("invalid instruction: {}", error);
        }
        match self {
            // Load instructions
            Instr::Ld { dst, src } => write!(f, "ld {}, {}", dst, src),
            Instr::Ldh { dst, src } => write!(f, "ldh {}, {}", dst, src),
            Instr::LdHlSp { offset } => write!(f, "ld hl, {}", sp_offset(*offset)),
            Instr::Push { pair } => write!(f, "push {}", pair),
            Instr::Pop { pair } => write!(f, "pop {}", pair),

            // 8-bit arithmetic and logic
            Instr::Add { src } => write!(f, "add a, {}", src),
            Instr::Adc { src } => write!(f, "adc a, {}", src),
            Instr::Sub { src } => write!(f, "sub a, {}", src),
            Instr::Sbc { src } => write!(f, "sbc a, {}", src),
            Instr::And { src } => write!(f, "and a, {}", src),
            Instr::Xor { src } => write!(f, "xor a, {}", src),
            Instr::Or { src } => write!(f, "or a, {}", src),
            Instr::Cp { src } => write!(f, "cp a, {}", src),
            Instr::Inc { operand } => write!(f, "inc {}", operand),
            Instr::Dec { operand } => write!(f, "dec {}", operand),

            // 16-bit arithmetic
            Instr::AddHl { src } => write!(f, "add hl, {}", src),
            Instr::AddSp { offset } => write!(f, "add sp, {}", offset),

            // Rotates and shifts
            Instr::Rlca => write!(f, "rlca"),
            Instr::Rrca => write!(f, "rrca"),
            Instr::Rla => write!(f, "rla"),
            Instr::Rra => write!(f, "rra"),
            Instr::Rlc { operand } => write!(f, "rlc {}", operand),
            Instr::Rrc { operand } => write!(f, "rrc {}", operand),
            Instr::Rl { operand } => write!(f, "rl {}", operand),
            Instr::Rr { operand } => write!(f, "rr {}", operand),
            Instr::Sla { operand } => write!(f, "sla {}", operand),
            Instr::Sra { operand } => write!(f, "sra {}", operand),
            Instr::Swap { operand } => write!(f, "swap {}", operand),
            Instr::Srl { operand } => write!(f, "srl {}", operand),

            // Bit instructions
            Instr::Bit { bit, operand } => write!(f, "bit {}, {}", bit, operand),
            Instr::Set { bit, operand } => write!(f, "set {}, {}", bit, operand),
            Instr::Res { bit, operand } => write!(f, "res {}, {}", bit, operand),

            // Flags and accumulator
            Instr::Daa => write!(f, "daa"),
            Instr::Cpl => write!(f, "cpl"),
            Instr::Scf => write!(f, "scf"),
            Instr::Ccf => write!(f, "ccf"),

            // CPU control
            Instr::Nop => write!(f, "nop"),
            Instr::Halt => write!(f, "halt"),
            Instr::Stop => write!(f, "stop"),
            Instr::Di => write!(f, "di"),
            Instr::Ei => write!(f, "ei"),

            // Jump instructions
            Instr::Jp { target } => write!(f, "jp {}", target),
            Instr::JpCond { condition, target } => write!(f, "jp {}, {}", condition, target),
            Instr::JpHl => write!(f, "jp hl"),
            Instr::Jr { target } => write!(f, "jr {}", target),
            Instr::JrCond { condition, target } => write!(f, "jr {}, {}", condition, target),
            Instr::Call { target } => write!(f, "call {}", target),
            Instr::CallCond { condition, target } => {
                write!(f, "call {}, {}", condition, target)
            }
            Instr::Ret => write!(f, "ret"),
            Instr::RetCond { condition } => write!(f, "ret {}", condition),
            Instr::Reti => write!(f, "reti"),
            Instr::Rst { vector } => write!(f, "rst ${:02x}", vector),

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
