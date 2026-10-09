//! The SM83 (Game Boy CPU) instructions and the RGBDS directives, as data.
//!
//! Each instruction family has one shape:
//! - the 8-bit ALU instructions work on `a` and take one source, [`Operand`]:
//!   [`Instr::Add`], [`Instr::Adc`], [`Instr::Sub`], [`Instr::Sbc`], [`Instr::And`],
//!   [`Instr::Xor`], [`Instr::Or`], [`Instr::Cp`] (`cp a, src`); the 16-bit additions are
//!   [`Instr::AddHl`] (`add hl, r16`) and [`Instr::AddSp`] (`add sp, e8`);
//! - the rotates, shifts and `swap` take one [`R8`] (an 8-bit register or `[hl]`):
//!   [`Instr::Rlc`], [`Instr::Rrc`], [`Instr::Rl`], [`Instr::Rr`], [`Instr::Sla`],
//!   [`Instr::Sra`], [`Instr::Swap`], [`Instr::Srl`]; the faster forms on `a` take nothing
//!   ([`Instr::Rlca`], [`Instr::Rrca`], [`Instr::Rla`], [`Instr::Rra`]);
//! - the bit instructions take a bit number (0 to 7) and an [`R8`]: [`Instr::Bit`],
//!   [`Instr::Set`], [`Instr::Res`];
//! - `push` and `pop` take an [`R16Stack`].
//!
//! An operand that the type cannot rule out (a bit number above 7, an `rst` vector that is
//! not a multiple of 8 up to `$38`, an ALU source that is a 16-bit register) is rejected by
//! [`Instr::check`]: [`Asm::emit`](super::Asm::emit) and the RGBDS output panic on it with
//! a clear message.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instr {
    // Load instructions
    Ld {
        dst: Operand,
        src: Operand,
    },
    Ldh {
        dst: Operand,
        src: Operand,
    },
    /// `ld hl, sp + offset`
    LdHlSp {
        offset: i8,
    },
    /// `push r16`
    Push {
        pair: R16Stack,
    },
    /// `pop r16`
    Pop {
        pair: R16Stack,
    },

    // 8-bit arithmetic and logic: `op a, src`
    /// `add a, src`
    Add {
        src: Operand,
    },
    /// `adc a, src`: `a + src + carry`
    Adc {
        src: Operand,
    },
    /// `sub a, src`
    Sub {
        src: Operand,
    },
    /// `sbc a, src`: `a - src - carry`
    Sbc {
        src: Operand,
    },
    /// `and a, src`
    And {
        src: Operand,
    },
    /// `xor a, src`
    Xor {
        src: Operand,
    },
    /// `or a, src`
    Or {
        src: Operand,
    },
    /// `cp a, src`: the flags of `a - src`, `a` unchanged
    Cp {
        src: Operand,
    },
    Inc {
        operand: Operand,
    },
    Dec {
        operand: Operand,
    },

    // 16-bit arithmetic
    /// `add hl, r16`
    AddHl {
        src: R16,
    },
    /// `add sp, offset`
    AddSp {
        offset: i8,
    },

    // Rotates and shifts on `a` (Z is always reset)
    Rlca,
    Rrca,
    Rla,
    Rra,

    // Rotates, shifts and swap (prefixed by `$CB`)
    /// Rotate left, bit 7 into the carry and bit 0
    Rlc {
        operand: R8,
    },
    /// Rotate right, bit 0 into the carry and bit 7
    Rrc {
        operand: R8,
    },
    /// Rotate left through the carry
    Rl {
        operand: R8,
    },
    /// Rotate right through the carry
    Rr {
        operand: R8,
    },
    /// Shift left, bit 7 into the carry, bit 0 reset
    Sla {
        operand: R8,
    },
    /// Shift right, bit 0 into the carry, bit 7 unchanged
    Sra {
        operand: R8,
    },
    /// Swap the two nibbles
    Swap {
        operand: R8,
    },
    /// Shift right, bit 0 into the carry, bit 7 reset
    Srl {
        operand: R8,
    },

    // Bit instructions (prefixed by `$CB`); `bit` is 0 to 7
    /// `bit n, r8`: Z set when the bit is 0
    Bit {
        bit: u8,
        operand: R8,
    },
    /// `set n, r8`
    Set {
        bit: u8,
        operand: R8,
    },
    /// `res n, r8`
    Res {
        bit: u8,
        operand: R8,
    },

    // Flags and accumulator
    Daa,
    /// `cpl`: `a = !a`
    Cpl,
    /// `scf`: set the carry
    Scf,
    /// `ccf`: complement the carry
    Ccf,

    // CPU control
    Nop,
    Halt,
    Stop,
    Di,
    Ei,

    // Jump instructions
    Jp {
        target: JumpTarget,
    },
    JpCond {
        condition: Condition,
        target: JumpTarget,
    },
    /// `jp hl`
    JpHl,
    Jr {
        target: JumpTarget,
    },
    JrCond {
        condition: Condition,
        target: JumpTarget,
    },
    Call {
        target: JumpTarget,
    },
    CallCond {
        condition: Condition,
        target: JumpTarget,
    },
    Ret,
    RetCond {
        condition: Condition,
    },
    Reti,
    /// `rst vector`: a call to `vector`, one of `$00`, `$08`, …, `$38`
    Rst {
        vector: u8,
    },

    // Assembler directives
    Ds {
        num_bytes: String,
        starter_point: String,
    },
    Include {
        file: String,
    },
    Incbin {
        file: String,
        offset: Option<u32>,
        length: Option<u32>,
    },
    Def {
        label: String,
        value: String,
    },
    Section {
        name: String,
        mem_type: String,
    },
    Label {
        name: String,
    },
    Comment {
        text: String,
    },
    Db {
        values: String,
    },
    Dw {
        value: String,
    },
    Raw {
        line: String,
    },
}

impl Instr {
    /// `Ok` if every operand is one the instruction takes, else what is wrong
    ///
    /// It checks what the operand types cannot rule out: the source of an 8-bit ALU
    /// instruction (an 8-bit register, `[hl]`, a number or an expression; not a 16-bit
    /// register, an address or `[hli]`), the bit number of `bit` / `set` / `res` (0 to 7),
    /// the vector of `rst` (`$00`, `$08`, …, `$38`), and the register of `[hli]` / `[hld]`
    /// (`hl` only). The other operands of `ld`, `ldh`, `inc` and `dec` are not checked yet.
    pub fn check(&self) -> Result<(), String> {
        match self {
            Instr::Add { src }
            | Instr::Adc { src }
            | Instr::Sub { src }
            | Instr::Sbc { src }
            | Instr::And { src }
            | Instr::Xor { src }
            | Instr::Or { src }
            | Instr::Cp { src } => match src {
                Operand::Reg(
                    Register::A
                    | Register::B
                    | Register::C
                    | Register::D
                    | Register::E
                    | Register::H
                    | Register::L,
                )
                | Operand::AddrReg(Register::HL)
                | Operand::Imm(_)
                | Operand::Label(_) => Ok(()),
                other => Err(format!(
                    "{} a, {}: the source of an 8-bit ALU instruction must be an 8-bit \
                     register, [hl], or an 8-bit value",
                    self.mnemonic(),
                    other
                )),
            },
            Instr::Bit { bit, .. } | Instr::Set { bit, .. } | Instr::Res { bit, .. }
                if *bit > 7 =>
            {
                Err(format!(
                    "{} {}: the bit number must be 0 to 7",
                    self.mnemonic(),
                    bit
                ))
            }
            Instr::Rst { vector } if vector % 8 != 0 || *vector > 0x38 => Err(format!(
                "rst ${:02x}: the vector must be one of $00, $08, $10, $18, $20, $28, $30, $38",
                vector
            )),
            Instr::Ld { dst, src } | Instr::Ldh { dst, src } => {
                for operand in [dst, src] {
                    if let Operand::AddrRegInc(reg) | Operand::AddrRegDec(reg) = operand {
                        if *reg != Register::HL {
                            return Err(format!(
                                "{} {}, {}: only hl can be incremented or decremented in a \
                                 load ([hli], [hld])",
                                self.mnemonic(),
                                dst,
                                src
                            ));
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// The mnemonic of an instruction with operands, for messages
    fn mnemonic(&self) -> &'static str {
        match self {
            Instr::Add { .. } => "add",
            Instr::Adc { .. } => "adc",
            Instr::Sub { .. } => "sub",
            Instr::Sbc { .. } => "sbc",
            Instr::And { .. } => "and",
            Instr::Xor { .. } => "xor",
            Instr::Or { .. } => "or",
            Instr::Cp { .. } => "cp",
            Instr::Bit { .. } => "bit",
            Instr::Set { .. } => "set",
            Instr::Res { .. } => "res",
            Instr::Ld { .. } => "ld",
            Instr::Ldh { .. } => "ldh",
            _ => "instruction",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Register {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    SP,
    PC,
    AF,
    BC,
    DE,
    HL,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    Reg(Register),
    Imm(u8),
    Imm16(u16),
    Addr(u16),
    AddrDef(String),
    AddrReg(Register),
    /// `[hli]`: the byte at `hl`, then `hl` is incremented (`hl` only)
    AddrRegInc(Register),
    /// `[hld]`: the byte at `hl`, then `hl` is decremented (`hl` only)
    AddrRegDec(Register),
    Label(String),
}

/// The operand of the instructions that take an 8-bit register or the byte at `[hl]`:
/// the rotates, shifts, `swap`, `bit`, `set` and `res`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    /// `[hl]`, the byte at the address in `hl`
    AtHl,
}

impl R8 {
    /// Every value, in the order of the SM83 encoding: `b`, `c`, `d`, `e`, `h`, `l`,
    /// `[hl]`, `a`
    pub const ALL: [R8; 8] = [R8::B, R8::C, R8::D, R8::E, R8::H, R8::L, R8::AtHl, R8::A];

    /// The `R8` written `name` (`a`, `b`, …, `[hl]`, any case); panics on anything else
    #[track_caller]
    pub(crate) fn from_name(name: &str) -> R8 {
        let name = name.trim();
        R8::ALL
            .into_iter()
            .find(|r8| r8.to_string().eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                panic!(
                    "{:?} is not an 8-bit register or [hl] (a, b, c, d, e, h, l, [hl])",
                    name
                )
            })
    }
}

impl From<R8> for Operand {
    fn from(r8: R8) -> Operand {
        match r8 {
            R8::A => Operand::Reg(Register::A),
            R8::B => Operand::Reg(Register::B),
            R8::C => Operand::Reg(Register::C),
            R8::D => Operand::Reg(Register::D),
            R8::E => Operand::Reg(Register::E),
            R8::H => Operand::Reg(Register::H),
            R8::L => Operand::Reg(Register::L),
            R8::AtHl => Operand::AddrReg(Register::HL),
        }
    }
}

/// A 16-bit register that `add hl, r16` adds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R16 {
    BC,
    DE,
    HL,
    SP,
}

impl R16 {
    /// The `R16` written `name` (`bc`, `de`, `hl`, `sp`, any case); panics on anything else
    #[track_caller]
    pub(crate) fn from_name(name: &str) -> R16 {
        let name = name.trim();
        [R16::BC, R16::DE, R16::HL, R16::SP]
            .into_iter()
            .find(|r16| r16.to_string().eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("{:?} is not a 16-bit register (bc, de, hl, sp)", name))
    }
}

impl From<R16> for Register {
    fn from(r16: R16) -> Register {
        match r16 {
            R16::BC => Register::BC,
            R16::DE => Register::DE,
            R16::HL => Register::HL,
            R16::SP => Register::SP,
        }
    }
}

/// A register pair that `push` and `pop` move
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R16Stack {
    BC,
    DE,
    HL,
    /// `a` and the flags
    AF,
}

impl From<R16Stack> for Register {
    fn from(pair: R16Stack) -> Register {
        match pair {
            R16Stack::BC => Register::BC,
            R16Stack::DE => Register::DE,
            R16Stack::HL => Register::HL,
            R16Stack::AF => Register::AF,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JumpTarget {
    Label(String),
    Addr(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    Z,  // Zero
    NZ, // Not Zero
    C,  // Carry
    NC, // Not Carry
}

impl fmt::Display for R8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            R8::AtHl => write!(f, "[hl]"),
            other => write!(f, "{}", Operand::from(*other)),
        }
    }
}

impl fmt::Display for R16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Register::from(*self))
    }
}

impl fmt::Display for R16Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Register::from(*self))
    }
}
