//! The SM83 (Game Boy CPU) instructions and the RGBDS directives, as data.
//!
//! Each instruction family has one shape, with typed operands:
//! - the loads, [`Instr::Ld`] and [`Instr::Ldh`], take a destination [`Dst`] (a register or
//!   memory, never a value: `ld 1, 2` cannot be written) and a source [`Operand`] (a
//!   register, memory, or a value [`Expr`]);
//! - the 8-bit ALU instructions work on `a` and take one source, an [`AluOperand`] (an
//!   8-bit register, `[hl]` or a value): [`Instr::Add`], [`Instr::Adc`], [`Instr::Sub`],
//!   [`Instr::Sbc`], [`Instr::And`], [`Instr::Xor`], [`Instr::Or`], [`Instr::Cp`]
//!   (`cp a, src`); the 16-bit additions are [`Instr::AddHl`] (`add hl, r16`) and
//!   [`Instr::AddSp`] (`add sp, e8`);
//! - `inc` and `dec` take an [`IncDec`]: an 8-bit register, `[hl]`, or a 16-bit register;
//! - the rotates, shifts and `swap` take one [`R8`] (an 8-bit register or `[hl]`):
//!   [`Instr::Rlc`], [`Instr::Rrc`], [`Instr::Rl`], [`Instr::Rr`], [`Instr::Sla`],
//!   [`Instr::Sra`], [`Instr::Swap`], [`Instr::Srl`]; the faster forms on `a` take nothing
//!   ([`Instr::Rlca`], [`Instr::Rrca`], [`Instr::Rla`], [`Instr::Rra`]);
//! - the bit instructions take a bit number (0 to 7) and an [`R8`]: [`Instr::Bit`],
//!   [`Instr::Set`], [`Instr::Res`];
//! - `push` and `pop` take an [`R16Stack`].
//!
//! What the types cannot rule out is rejected by [`Instr::check`]: a load whose two
//! operands do not make an SM83 instruction (`ld [hl], [hl]`, `ld b, [de]`, `ld bc, de`), a
//! constant value that does not fit its operand (`ld a, 300`), a bit number above 7, an
//! `rst` vector that is not a multiple of 8 up to `$38`. [`Asm::emit`](super::Asm::emit)
//! and the RGBDS output panic on it with a clear message.

use std::fmt;

use super::expr::{Expr, parse_number};
use super::labels::code_lines;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instr {
    // Load instructions
    /// `ld dst, src`: every form but `ld hl, sp + e` ([`Instr::LdHlSp`]); see
    /// [`Instr::check`] for the pairs it takes
    Ld {
        dst: Dst,
        src: Operand,
    },
    /// `ldh dst, src`: `ldh [n8], a`, `ldh a, [n8]`, `ldh [c], a`, `ldh a, [c]`, the
    /// address `$FF00` to `$FFFF` (`Mem::Addr`) or `$FF00 + c` (`Mem::C`)
    Ldh {
        dst: Dst,
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
        src: AluOperand,
    },
    /// `adc a, src`: `a + src + carry`
    Adc {
        src: AluOperand,
    },
    /// `sub a, src`
    Sub {
        src: AluOperand,
    },
    /// `sbc a, src`: `a - src - carry`
    Sbc {
        src: AluOperand,
    },
    /// `and a, src`
    And {
        src: AluOperand,
    },
    /// `xor a, src`
    Xor {
        src: AluOperand,
    },
    /// `or a, src`
    Or {
        src: AluOperand,
    },
    /// `cp a, src`: the flags of `a - src`, `a` unchanged
    Cp {
        src: AluOperand,
    },
    /// `inc r8` (Z set from the result, the carry unchanged) or `inc r16` (no flags)
    Inc {
        operand: IncDec,
    },
    /// `dec r8` (Z set from the result, the carry unchanged) or `dec r16` (no flags)
    Dec {
        operand: IncDec,
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
    /// `Ok` if the operands make an SM83 instruction, else what is wrong
    ///
    /// It checks what the operand types cannot rule out:
    /// - the loads: `ld` takes `r8, r8` (not `[hl], [hl]`), `r8, n8`, `r16, n16`, `sp, hl`,
    ///   `a` with `[bc]`, `[de]`, `[hli]`, `[hld]` or `[n16]` (either way), and `[n16], sp`;
    ///   `ldh` takes `a` with `[c]` or `[n16]` (either way), the address `$FF00` to `$FFFF`;
    /// - a constant value ([`Expr::value`]) must fit its operand: -128 to 255 for 8 bits,
    ///   -32768 to 65535 for 16 bits (a symbol is not checked: its value is known only to
    ///   RGBDS);
    /// - the bit number of `bit` / `set` / `res` (0 to 7) and the vector of `rst` (`$00`,
    ///   `$08`, …, `$38`).
    pub fn check(&self) -> Result<(), String> {
        match self {
            Instr::Ld { dst, src } => {
                check_ld(dst, src).map_err(|why| format!("ld {}, {}: {}", dst, src, why))
            }
            Instr::Ldh { dst, src } => {
                check_ldh(dst, src).map_err(|why| format!("ldh {}, {}: {}", dst, src, why))
            }
            Instr::Add { src }
            | Instr::Adc { src }
            | Instr::Sub { src }
            | Instr::Sbc { src }
            | Instr::And { src }
            | Instr::Xor { src }
            | Instr::Or { src }
            | Instr::Cp { src } => match src {
                AluOperand::R8(_) => Ok(()),
                AluOperand::Imm(value) => fits(value, Width::Byte)
                    .map_err(|why| format!("{} a, {}: {}", self.mnemonic(), src, why)),
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
            _ => Ok(()),
        }
    }

    /// Size in bytes once assembled, or `None` when only RGBDS can know it
    ///
    /// Every SM83 instruction has a size (checked against rgbasm, family by family, in
    /// `gb_asm::isa_tests`): `cp a, b` 1 byte, `cp a, n8` 2, `jr` 2, `jp` and `call` 3, ...
    /// Labels, comments, `DEF` and `SECTION` take no room. The directives have a size when
    /// it is written as plain numbers: `ds 4`, `db 1, 2, $FF` (3), `dw 1, $8000` (4),
    /// `INCBIN` with a length, a raw line with no code (empty or a comment). The others are
    /// `None`:
    /// - `ds $150 - @`, `INCLUDE`, `INCBIN` without a length, a raw line with code;
    /// - a `db` / `dw` with a string (its bytes depend on the charmap), an expression or a
    ///   symbol: a symbol can be an `EQUS` that expands to several values (`dw Label` is a
    ///   label's address, 2 bytes, but `db S` with `DEF S EQUS "1, 2, 3"` is 3);
    /// - anything whose text has a line break (a comment or a label written with `\n`
    ///   prints the next line as code).
    ///
    /// The jump relaxation of [`Asm::to_asm`](super::Asm::to_asm) never keeps a `jr` over
    /// one of them. An instruction's operand is taken as written: a symbol is a value (an
    /// `EQUS` that expands to a register, `cp a, S` with `DEF S EQUS "b"`, is not seen).
    pub fn size(&self) -> Option<usize> {
        if self.check().is_ok() && self.to_string().contains('\n') {
            return None;
        }
        let size = match self {
            Instr::Label { .. }
            | Instr::Comment { .. }
            | Instr::Def { .. }
            | Instr::Section { .. } => 0,
            Instr::Ld { dst, src } => match (dst, src) {
                (Dst::R8(_), Operand::R8(_)) | (Dst::R16(R16::SP), Operand::R16(R16::HL)) => 1,
                (Dst::R8(_), Operand::Imm(_)) => 2,
                (Dst::R16(_), Operand::Imm(_)) => 3,
                (Dst::Mem(Mem::Addr(_)), _) | (_, Operand::Mem(Mem::Addr(_))) => 3,
                (Dst::Mem(_), Operand::R8(_)) | (Dst::R8(_), Operand::Mem(_)) => 1,
                // Not an SM83 instruction: `check` rejects it
                _ => return None,
            },
            // `ldh a, [c]` / `ldh [c], a` 1 byte, `ldh a, [n8]` / `ldh [n8], a` 2
            Instr::Ldh {
                dst: Dst::Mem(Mem::C),
                ..
            }
            | Instr::Ldh {
                src: Operand::Mem(Mem::C),
                ..
            } => 1,
            Instr::Ldh { .. } => 2,
            // `op a, src`: register or [hl] 1 byte, value 2
            Instr::Add { src }
            | Instr::Adc { src }
            | Instr::Sub { src }
            | Instr::Sbc { src }
            | Instr::And { src }
            | Instr::Xor { src }
            | Instr::Or { src }
            | Instr::Cp { src } => match src {
                AluOperand::R8(_) => 1,
                AluOperand::Imm(_) => 2,
            },
            Instr::Inc { .. }
            | Instr::Dec { .. }
            | Instr::AddHl { .. }
            | Instr::Push { .. }
            | Instr::Pop { .. }
            | Instr::Rlca
            | Instr::Rrca
            | Instr::Rla
            | Instr::Rra
            | Instr::Daa
            | Instr::Cpl
            | Instr::Scf
            | Instr::Ccf
            | Instr::Nop
            | Instr::Halt
            | Instr::Di
            | Instr::Ei
            | Instr::JpHl
            | Instr::Ret
            | Instr::RetCond { .. }
            | Instr::Reti
            | Instr::Rst { .. } => 1,
            // rgbasm follows `stop` with a $00 byte
            Instr::Stop | Instr::AddSp { .. } | Instr::LdHlSp { .. } => 2,
            // The `$CB`-prefixed instructions
            Instr::Rlc { .. }
            | Instr::Rrc { .. }
            | Instr::Rl { .. }
            | Instr::Rr { .. }
            | Instr::Sla { .. }
            | Instr::Sra { .. }
            | Instr::Swap { .. }
            | Instr::Srl { .. }
            | Instr::Bit { .. }
            | Instr::Set { .. }
            | Instr::Res { .. } => 2,
            Instr::Jr { .. } | Instr::JrCond { .. } => 2,
            Instr::Jp { .. }
            | Instr::JpCond { .. }
            | Instr::Call { .. }
            | Instr::CallCond { .. } => 3,
            Instr::Ds { num_bytes, .. } => plain_number(num_bytes)?,
            Instr::Db { values } => data_items(values)?,
            Instr::Dw { value } => 2 * data_items(value)?,
            Instr::Incbin {
                length: Some(length),
                ..
            } => usize::try_from(*length).ok()?,
            Instr::Incbin { length: None, .. } | Instr::Include { .. } => return None,
            Instr::Raw { line } => {
                if code_lines(line).iter().all(|code| code.trim().is_empty()) {
                    0
                } else {
                    return None;
                }
            }
        };
        Some(size)
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
            _ => "instruction",
        }
    }
}

/// The value of `text` if it is a plain, non-negative RGBDS number (`4`, `$10`)
fn plain_number(text: &str) -> Option<usize> {
    let (value, _) = parse_number(text.trim())?;
    usize::try_from(value).ok()
}

/// How many values a `db` / `dw` line lists, if each is a plain number (`1, $FF, %101`);
/// `None` with a symbol (it can be an `EQUS` of several values), a string, a character, an
/// expression or no value
fn data_items(values: &str) -> Option<usize> {
    let code = code_lines(values).join(" ");
    let items: Vec<&str> = code.split(',').map(str::trim).collect();
    if items.iter().all(|item| plain_number(item).is_some()) {
        Some(items.len())
    } else {
        None
    }
}

/// The size of a value operand
#[derive(Clone, Copy)]
enum Width {
    /// `n8`: -128 to 255
    Byte,
    /// `n16`: -32768 to 65535
    Word,
}

/// `Ok` unless `value` is a constant that does not fit in `width` (rgbasm would truncate it)
fn fits(value: &Expr, width: Width) -> Result<(), String> {
    let (min, max, bits) = match width {
        Width::Byte => (-0x80, 0xFF, 8),
        Width::Word => (-0x8000, 0xFFFF, 16),
    };
    match value.value() {
        Some(v) if !(min..=max).contains(&v) => Err(format!(
            "the value {} does not fit in {} bits ({} to {})",
            v, bits, min, max
        )),
        _ => Ok(()),
    }
}

/// Why `ld dst, src` is not an SM83 instruction, if it is not
fn check_ld(dst: &Dst, src: &Operand) -> Result<(), String> {
    match (dst, src) {
        (Dst::R8(R8::AtHl), Operand::R8(R8::AtHl)) => {
            Err("[hl] cannot be both the destination and the source".to_string())
        }
        (Dst::R8(_), Operand::R8(_)) | (Dst::R16(R16::SP), Operand::R16(R16::HL)) => Ok(()),
        (Dst::R8(_), Operand::Imm(value)) => fits(value, Width::Byte),
        (Dst::R16(_), Operand::Imm(value)) => fits(value, Width::Word),
        (Dst::Mem(Mem::C), _) | (_, Operand::Mem(Mem::C)) => {
            Err("[c] is an ldh operand: use ldh".to_string())
        }
        (Dst::Mem(_), Operand::R8(R8::A)) | (Dst::R8(R8::A), Operand::Mem(_)) => Ok(()),
        (Dst::Mem(Mem::Addr(_)), Operand::R16(R16::SP)) => Ok(()),
        (Dst::Mem(_), Operand::R8(_)) | (Dst::R8(_), Operand::Mem(_)) => Err(
            "only a moves between a register and [bc], [de], [hli], [hld] or an address \
             ([hl] takes any 8-bit register)"
                .to_string(),
        ),
        (Dst::Mem(_), Operand::Imm(_)) => {
            Err("a value can only be stored to [hl] (ld [hl], n8)".to_string())
        }
        (Dst::R16(_), Operand::R16(_)) => {
            Err("the only copy between 16-bit registers is ld sp, hl".to_string())
        }
        _ => Err("no SM83 ld takes these two operands".to_string()),
    }
}

/// Why `ldh dst, src` is not an SM83 instruction, if it is not
fn check_ldh(dst: &Dst, src: &Operand) -> Result<(), String> {
    match (dst, src) {
        (Dst::Mem(mem), Operand::R8(R8::A)) | (Dst::R8(R8::A), Operand::Mem(mem)) => match mem {
            Mem::C => Ok(()),
            Mem::Addr(address) => match address.value() {
                Some(v) if !(0xFF00..=0xFFFF).contains(&v) => {
                    Err(format!("the address {} is not in $FF00 to $FFFF", address))
                }
                _ => Ok(()),
            },
            _ => Err("ldh takes [c] or an address from $FF00 to $FFFF".to_string()),
        },
        _ => Err("ldh moves a to or from [c] or an address from $FF00 to $FFFF".to_string()),
    }
}

/// An 8-bit register, or the byte at `[hl]`: the operand of `ld r8, …`, the ALU
/// instructions, `inc` / `dec`, the rotates, shifts, `swap`, `bit`, `set` and `res`
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
}

/// A 16-bit register: the operand of `ld r16, n16`, `inc` / `dec` and `add hl, r16`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R16 {
    BC,
    DE,
    HL,
    SP,
}

impl R16 {
    /// Every value, in the order of the SM83 encoding
    pub const ALL: [R16; 4] = [R16::BC, R16::DE, R16::HL, R16::SP];
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

/// A byte of memory a load reads or writes, other than `[hl]` ([`R8::AtHl`])
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mem {
    /// `[bc]`
    Bc,
    /// `[de]`
    De,
    /// `[hli]`: the byte at `hl`, then `hl` is incremented
    Hli,
    /// `[hld]`: the byte at `hl`, then `hl` is decremented
    Hld,
    /// `[c]`: the byte at `$FF00 + c` (`ldh` only)
    C,
    /// `[address]`: a variable, a hardware register, any address (`[n16]`; for `ldh`,
    /// `$FF00` to `$FFFF`)
    Addr(Expr),
}

impl Mem {
    /// `[address]`: `Mem::addr("wScore")`, `Mem::addr(Expr::sym("_OAMRAM") + 4)`,
    /// `Mem::addr(Expr::hex(0xC000))`
    #[track_caller]
    pub fn addr(address: impl Into<Expr>) -> Mem {
        Mem::Addr(address.into())
    }
}

/// The destination of a load: a register or a byte of memory, never a value
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dst {
    /// `a`, `b`, …, `l`, `[hl]`
    R8(R8),
    /// `bc`, `de`, `hl`, `sp`
    R16(R16),
    /// `[bc]`, `[de]`, `[hli]`, `[hld]`, `[c]`, `[address]`
    Mem(Mem),
}

/// The source of a load: a register, a byte of memory, or a value
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    /// `a`, `b`, …, `l`, `[hl]`
    R8(R8),
    /// `bc`, `de`, `hl`, `sp`
    R16(R16),
    /// A value: `n8` or `n16` (a number, a symbol, an expression)
    Imm(Expr),
    /// `[bc]`, `[de]`, `[hli]`, `[hld]`, `[c]`, `[address]`
    Mem(Mem),
}

/// The source of an 8-bit ALU instruction (`add a, src`, …, `cp a, src`): an 8-bit
/// register, `[hl]`, or a value
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AluOperand {
    /// `a`, `b`, …, `l`, `[hl]`
    R8(R8),
    /// A value, `n8` (a number, a symbol, an expression)
    Imm(Expr),
}

/// The operand of `inc` and `dec`: an 8-bit register, `[hl]`, or a 16-bit register
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncDec {
    R8(R8),
    R16(R16),
}

impl From<R8> for Dst {
    fn from(r8: R8) -> Dst {
        Dst::R8(r8)
    }
}

impl From<R16> for Dst {
    fn from(r16: R16) -> Dst {
        Dst::R16(r16)
    }
}

impl From<Mem> for Dst {
    fn from(mem: Mem) -> Dst {
        Dst::Mem(mem)
    }
}

impl From<R8> for Operand {
    fn from(r8: R8) -> Operand {
        Operand::R8(r8)
    }
}

impl From<R16> for Operand {
    fn from(r16: R16) -> Operand {
        Operand::R16(r16)
    }
}

impl From<Mem> for Operand {
    fn from(mem: Mem) -> Operand {
        Operand::Mem(mem)
    }
}

impl From<Dst> for Operand {
    fn from(dst: Dst) -> Operand {
        match dst {
            Dst::R8(r8) => Operand::R8(r8),
            Dst::R16(r16) => Operand::R16(r16),
            Dst::Mem(mem) => Operand::Mem(mem),
        }
    }
}

impl From<R8> for AluOperand {
    fn from(r8: R8) -> AluOperand {
        AluOperand::R8(r8)
    }
}

impl From<R8> for IncDec {
    fn from(r8: R8) -> IncDec {
        IncDec::R8(r8)
    }
}

impl From<R16> for IncDec {
    fn from(r16: R16) -> IncDec {
        IncDec::R16(r16)
    }
}

/// A value converts into an [`Operand::Imm`] and an [`AluOperand::Imm`]: an [`Expr`], a
/// Rust integer, or text read by `Expr::from` (a symbol or a number; it panics on
/// anything else, a register name included)
macro_rules! from_value {
    ($($t:ty),*) => {$(
        impl From<$t> for Operand {
            #[track_caller]
            fn from(value: $t) -> Operand {
                Operand::Imm(Expr::from(value))
            }
        }

        impl From<$t> for AluOperand {
            #[track_caller]
            fn from(value: $t) -> AluOperand {
                AluOperand::Imm(Expr::from(value))
            }
        }
    )*};
}
from_value!(Expr, &Expr, &str, String, &String, u8, i8, u16, i16, i32);

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
        let name = match self {
            R8::A => "a",
            R8::B => "b",
            R8::C => "c",
            R8::D => "d",
            R8::E => "e",
            R8::H => "h",
            R8::L => "l",
            R8::AtHl => "[hl]",
        };
        write!(f, "{}", name)
    }
}

impl fmt::Display for R16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            R16::BC => "bc",
            R16::DE => "de",
            R16::HL => "hl",
            R16::SP => "sp",
        };
        write!(f, "{}", name)
    }
}

impl fmt::Display for R16Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            R16Stack::BC => "bc",
            R16Stack::DE => "de",
            R16Stack::HL => "hl",
            R16Stack::AF => "af",
        };
        write!(f, "{}", name)
    }
}

impl fmt::Display for Mem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mem::Bc => write!(f, "[bc]"),
            Mem::De => write!(f, "[de]"),
            Mem::Hli => write!(f, "[hli]"),
            Mem::Hld => write!(f, "[hld]"),
            Mem::C => write!(f, "[c]"),
            Mem::Addr(address) => write!(f, "[{}]", address),
        }
    }
}

impl fmt::Display for Dst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dst::R8(r8) => write!(f, "{}", r8),
            Dst::R16(r16) => write!(f, "{}", r16),
            Dst::Mem(mem) => write!(f, "{}", mem),
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::R8(r8) => write!(f, "{}", r8),
            Operand::R16(r16) => write!(f, "{}", r16),
            Operand::Imm(value) => write!(f, "{}", value),
            Operand::Mem(mem) => write!(f, "{}", mem),
        }
    }
}

impl fmt::Display for AluOperand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AluOperand::R8(r8) => write!(f, "{}", r8),
            AluOperand::Imm(value) => write!(f, "{}", value),
        }
    }
}

impl fmt::Display for IncDec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IncDec::R8(r8) => write!(f, "{}", r8),
            IncDec::R16(r16) => write!(f, "{}", r16),
        }
    }
}
