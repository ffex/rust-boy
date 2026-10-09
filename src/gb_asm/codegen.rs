use super::asm::Asm;
use super::instr::{Condition, Instr, JumpTarget};
use super::relax::relax_jumps;
use std::fmt;

// Code generation implementation for Asm
impl Asm {
    /// The whole program, as [`Asm::to_asm`] prints it: the instructions in the order they
    /// were emitted, with the jumps relaxed
    ///
    /// Each `jr` / `jr cc` that does not reach its target becomes a `jp` / `jp cc`: one
    /// whose target is more than 127 bytes ahead or 128 behind (counted from the end of
    /// the `jr`), in another section, or not a label of the program (an external symbol),
    /// or whose distance only RGBDS knows (a raw line, an `INCLUDE`, a `db` with a string
    /// in between). It is done on the whole program, so a `jr` that grows and pushes
    /// another one out of range makes that one grow too. A `jp` stays a `jp`.
    ///
    /// A target written from `@` (`jr nz, @+4`, also on a `jp` or a `call`) is the
    /// instruction that many bytes from the jump: its offset is written again when a jump
    /// in between, or the jump itself, grows. If that instruction cannot be found (an offset
    /// inside an instruction, a size only RGBDS knows in between), or a jump's target is
    /// another expression (`Label + 2`), or `@` appears anywhere else in the code (a raw
    /// line, data, an operand; but not the padding `ds N - @`), the program is printed as
    /// written, with no jump changed: rgbasm then reports a `jr` out of range. See
    /// `gb_asm::relax` for every rule.
    ///
    /// # Example
    /// ```
    /// use rust_boy::gb_asm::{Asm, Instr, JumpTarget};
    ///
    /// let mut asm = Asm::new();
    /// asm.label("Main").jr(".far");
    /// for _ in 0..128 {
    ///     asm.nop();
    /// }
    /// asm.label(".far").jr("Main");
    /// let program = asm.program();
    /// // 128 bytes ahead is out of reach of a jr; 133 bytes back too, once it is a jp
    /// assert_eq!(program[1], Instr::Jp { target: JumpTarget::Label(".far".into()) });
    /// assert_eq!(program[131], Instr::Jp { target: JumpTarget::Label("Main".into()) });
    /// ```
    pub fn program(&self) -> Vec<Instr> {
        relax_jumps(self.instrs())
    }

    /// The program's RGBDS assembly: [`Asm::program`], one instruction per line, each
    /// group of lines followed by a blank line ([`Asm::blank_line`])
    pub fn to_asm(&self) -> String {
        let program = self.program();
        let mut asm = String::new();
        let mut start = 0;
        for end in self.group_ends() {
            if end > start {
                for instruction in &program[start..end] {
                    asm.push_str(&format!("    {}\n", instruction));
                }
                asm.push('\n');
                start = end;
            }
        }
        asm
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
            Instr::Ds { count, fill: None } => write!(f, "ds {}", count),
            Instr::Ds {
                count,
                fill: Some(fill),
            } => write!(f, "ds {}, {}", count, fill),
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
            Instr::Section(section) => write!(f, "{}", section),
            Instr::Label { name } => write!(f, "{}:", name),
            Instr::Comment { text } => write!(f, "; {}", text),
            Instr::Db { values } => write!(f, "db {}", values),
            Instr::Dw { value } => write!(f, "dw {}", value),
            Instr::Raw { line } => write!(f, "{}", line),
        }
    }
}
