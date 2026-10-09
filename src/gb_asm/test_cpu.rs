//! A tiny model of the Game Boy CPU for unit tests.
//!
//! It runs the [`Instr`]s a routine emits, so a test can check what the generated code
//! does rather than how it looks. It models the 8-bit registers, the Z and C flags, a
//! memory addressed by symbol (`[wCurKeys]`, `[_OAMRAM+1]`, …) and symbolic constants
//! (`PADF_LEFT`).
//!
//! It reads the typed operands ([`Operand`], [`Dst`], [`Mem`], [`AluOperand`], [`IncDec`])
//! and the [`Expr`] they hold, never their text. An address is a symbol plus an offset, or
//! a number ([`Pointer`]): `Expr::sym("_OAMRAM") + 4 + 1` is `_OAMRAM` + 5, a byte the
//! memory names `_OAMRAM+5`, and a number is named `$XXXX` (`$9821`). A value is a
//! constant ([`Expr::value`]), a symbol of [`TestCpu::consts`], or an expression of them
//! (`LCDCF_ON | LCDCF_BGON`); [`TestCpu::consts`] can also give the value of a whole
//! expression, by its text. A value that does not fit its 8-bit operand (-128 to 255)
//! panics, as rgbasm would truncate it. A test names the bytes it sets or reads with the
//! same text (`"_OAMRAM+5"`, `"_OAMRAM + 5"`, `"$9821"`): [`Memory`] reads it into the same
//! name, which is the only text the model parses.
//!
//! The register pairs `bc`, `de` and `hl` hold a symbolic address
//! ([`Pointer`]): `ld hl, _OAMRAM` then `ld [hli], a` writes `[_OAMRAM]`, then
//! `[_OAMRAM+1]`, the names a direct access such as `ld [_OAMRAM+1], a` uses. Loading a
//! pair with a symbol makes its two 8-bit registers unknown, and changing one of them
//! makes the pair unknown; reading an unknown register, or using an unknown pair, panics.
//! A pair loaded with a number (`ld bc, $9800`, or an expression of [`TestCpu::consts16`]),
//! or whose two registers are known, holds that number, as on the CPU: `ld h, 0` then
//! `ld l, a` sets `hl`. 16-bit `add hl, rr`, `inc rr` and `dec rr` work on numbers and on
//! a symbol plus a number (the carry of `add hl` is then unknown). Every load is modelled
//! but the ones with `sp`: `ld [bc]`, `[de]`, `[hl]`, `[hli]`, `[hld]` and `[n16]` either
//! way, `ldh [n8]` and `ldh [c]` (the byte at `$FF00 + c`). The 8-bit ALU instructions,
//! `inc` / `dec` (also on `[hl]`), the rotates and shifts, `swap`, `bit` / `set` / `res`
//! (on a register or `[hl]`), `cpl`, `scf`, `ccf` and `nop` are modelled exactly for the Z
//! and C flags; N and H are not modelled, so `daa` is not either. `push` and `pop` share
//! one stack with `call` and `ret`, as on the CPU: a pushed pair can be popped into
//! another pair (`push hl` / `pop de`), with what the model knows of it, but a `ret` to a
//! pushed value or a `pop` of a return address panics. `halt`, `stop`, `di`, `ei`, `reti`,
//! `rst`, `jp hl`, `add sp`, `ld hl, sp + e`, `add hl, sp` and every other use of `sp` are
//! not modelled. Jumps and
//! calls go to labels in the same instruction list, and
//! execution ends when it runs past the last instruction or on a `ret` with no `call`
//! to return to (so a routine can be run on its own, or a test can put its routines
//! after a `ret`). A call to one of the [`TestCpu::stubs`] returns at once (for a
//! routine the model cannot run, such as `Memcopy`) and leaves every register, pair and
//! flag unknown, as a routine may change them all. Every write to memory and every
//! call is recorded in [`TestCpu::trace`], so a test can check the order of side
//! effects. A local label (`.name`) belongs to the last global label before
//! it, as in RGBDS, so two routines can each have their own `.loop`; `label_check`
//! checks the scopes of a whole program. Anything it does not model panics, so a test
//! never passes by skipping code.

use std::collections::{BTreeMap, BTreeSet};

use super::expr::{BinOp, Expr};
use super::{
    AluOperand, Condition, Dst, IncDec, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};

/// The address held by a register pair: a symbol plus an offset in bytes, or a number
/// (an empty symbol, the number in `offset`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pointer {
    pub symbol: String,
    pub offset: u16,
}

impl Pointer {
    /// The address a test names with the text `symbol`, split into a base symbol and
    /// offsets (any RGBDS number): `_OAMRAM+4+1` and `_OAMRAM + $5` are `_OAMRAM` + 5; a
    /// number (`$9800`, `0x9800`, `%1001`, `38912`) has no symbol. Only for the names of
    /// [`Memory`]: the code's operands are read as [`Expr`]s.
    fn parse(symbol: &str) -> Pointer {
        let symbol = symbol.trim();
        if let Some((base, offset)) = symbol.rsplit_once('+') {
            if let Some(offset) = parse_number(offset.trim()) {
                let mut pointer = Pointer::parse(base);
                pointer.offset = pointer.offset.wrapping_add(offset);
                return pointer;
            }
        }
        match parse_number(symbol) {
            Some(number) => Pointer::number(number),
            None => Pointer::symbol(symbol),
        }
    }

    /// The address `number`
    fn number(number: u16) -> Pointer {
        Pointer {
            symbol: String::new(),
            offset: number,
        }
    }

    /// The address of the symbol `name`
    fn symbol(name: &str) -> Pointer {
        Pointer {
            symbol: name.to_string(),
            offset: 0,
        }
    }

    fn is_number(&self) -> bool {
        self.symbol.is_empty()
    }

    /// The memory symbol of the byte it points to: `_OAMRAM`, `_OAMRAM+1`, `$9821`, …
    fn name(&self) -> String {
        if self.is_number() {
            format!("${:04X}", self.offset)
        } else if self.offset == 0 {
            self.symbol.clone()
        } else {
            format!("{}+{}", self.symbol, self.offset)
        }
    }

    /// The address `delta` bytes further (`delta` may be negative); panics below the
    /// symbol, whose value the model does not know
    fn moved(&self, delta: i32) -> Pointer {
        let offset = if self.is_number() {
            (i32::from(self.offset) + delta).rem_euclid(0x10000)
        } else {
            i32::from(self.offset) + delta
        };
        let offset = u16::try_from(offset).unwrap_or_else(|_| {
            panic!(
                "{} {:+}: an address before its symbol is not supported by the test CPU",
                self.name(),
                delta
            )
        });
        Pointer {
            symbol: self.symbol.clone(),
            offset,
        }
    }
}

/// The value of an RGBDS number a test writes in the name of a byte of [`Memory`]
///
/// `None` for a symbol (text that does not start like a number). Every RGBDS form is
/// read: `$FF`, `0xFF`, `%101`, `0b101`, `&17`, `0o17`, decimal, `_` between digits, and a
/// leading `-` (a 16-bit two's complement: `-1` is `$FFFF`). Text that starts like a
/// number but is none of these (`$98G0`, `$9800 + X`, `70000`) panics: the model does not
/// guess.
fn parse_number(text: &str) -> Option<u16> {
    let starts_like_a_number =
        |text: &str| text.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '$' | '%' | '&'));
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) if starts_like_a_number(rest) => (true, rest),
        _ => (false, text),
    };
    if !starts_like_a_number(unsigned) {
        return None;
    }
    let prefixes: [(&str, u32); 8] = [
        ("$", 16),
        ("0x", 16),
        ("0X", 16),
        ("%", 2),
        ("0b", 2),
        ("0B", 2),
        ("&", 8),
        ("0o", 8),
    ];
    let (digits, radix) = prefixes
        .iter()
        .find_map(|(prefix, radix)| unsigned.strip_prefix(prefix).map(|rest| (rest, *radix)))
        .unwrap_or((unsigned, 10));
    let digits: String = digits.chars().filter(|&c| c != '_').collect();
    let value = u16::from_str_radix(&digits, radix)
        .ok()
        .filter(|_| !digits.is_empty() && !digits.starts_with('+'))
        .unwrap_or_else(|| panic!("number {} not supported by the test CPU", text));
    Some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

/// The one name of the memory byte a test names `symbol`: `_OAMRAM+4+1` is `_OAMRAM+5`
fn normalize(symbol: &str) -> String {
    Pointer::parse(symbol).name()
}

/// The memory of [`TestCpu`]: bytes by symbol, every symbol normalised, so a test can
/// read `mem["_OAMRAM+0"]` or insert `"_OAMRAM + 5"` and reach the same byte as the
/// code (`_OAMRAM`, `_OAMRAM+5`)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Memory(BTreeMap<String, u8>);

impl Memory {
    pub fn insert(&mut self, symbol: String, value: u8) -> Option<u8> {
        self.0.insert(normalize(&symbol), value)
    }

    pub fn get(&self, symbol: &str) -> Option<&u8> {
        self.0.get(&normalize(symbol))
    }

    /// The normalised symbols written, in order
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }
}

impl<Q: AsRef<str> + ?Sized> std::ops::Index<&Q> for Memory {
    type Output = u8;

    fn index(&self, symbol: &Q) -> &u8 {
        self.get(symbol.as_ref())
            .unwrap_or_else(|| panic!("[{}] was never written", symbol.as_ref()))
    }
}

/// Bits of [`TestCpu`]'s unknown set: a register or flag whose value the model does
/// not know (reading it panics)
const UNKNOWN_A: u16 = 1 << 0;
const UNKNOWN_B: u16 = 1 << 1;
const UNKNOWN_C: u16 = 1 << 2;
const UNKNOWN_D: u16 = 1 << 3;
const UNKNOWN_E: u16 = 1 << 4;
const UNKNOWN_H: u16 = 1 << 5;
const UNKNOWN_L: u16 = 1 << 6;
const UNKNOWN_ZERO: u16 = 1 << 7;
const UNKNOWN_CARRY: u16 = 1 << 8;
const UNKNOWN_ALL: u16 = (1 << 9) - 1;

/// A side effect recorded in [`TestCpu::trace`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// `value` written to the memory symbol
    Write(String, u8),
    /// A call to the label (a stub or a routine in the code)
    Call(String),
}

/// What `call` and `push` put on the stack of [`TestCpu::run`]
#[derive(Debug)]
enum StackEntry {
    /// The instruction a `ret` goes back to
    Return(usize),
    /// A register pair pushed with `push`
    Pair(Saved),
}

/// The two bytes `push` saves (high, then low), which of their bits the model does not
/// know (`unknown`, a mask per byte), and the symbolic address the pair held, if any (its
/// bytes are then unknown)
#[derive(Debug)]
struct Saved {
    bytes: [u8; 2],
    unknown: [u8; 2],
    pointer: Option<Pointer>,
}

#[derive(Debug, Default)]
pub(crate) struct TestCpu {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub zero: bool,
    pub carry: bool,
    /// Memory accessed through `[address]` operands and the register pairs, by normalised
    /// name (`_OAMRAM+5`); reading a byte never written panics
    pub mem: Memory,
    /// Values of the symbols used as 8-bit values (`and PADF_LEFT`), or of a whole value
    /// expression by its text (`LCDCF_ON | LCDCF_BGON`); reading one that is not set
    /// panics
    pub consts: BTreeMap<String, u8>,
    /// Values of the expressions loaded into a register pair as numbers, by their text
    /// (`ld bc, TilesEnd - Tiles`); a symbol not set here is an address
    pub consts16: BTreeMap<String, u16>,
    /// The register pairs that hold a symbolic address; `None` for a pair that holds a
    /// number (its two 8-bit registers, which must then be known) or nothing (using it
    /// panics), as after one of its 8-bit halves is changed
    pub bc: Option<Pointer>,
    pub de: Option<Pointer>,
    pub hl: Option<Pointer>,
    /// Routines that are not run: a call to one is recorded and returns at once
    pub stubs: BTreeSet<String>,
    /// Every memory write and every call, in order
    pub trace: Vec<Event>,
    /// The registers and flags the model does not know (`UNKNOWN_*` bits): the halves
    /// of a pair loaded with an address, everything after a stub. A test that reads
    /// one of the public fields directly gets a stale value.
    unknown: u16,
}

impl TestCpu {
    /// Run `instrs` from the first instruction until execution runs past the last one,
    /// or returns from the code it started in
    pub fn run(&mut self, instrs: &[Instr]) {
        // The global label each instruction is under ("" before the first one), and
        // every label by its full name (`Scope.local` for a local label)
        let mut scopes = Vec::with_capacity(instrs.len());
        let mut scope = "";
        let mut labels = BTreeMap::new();
        let full_name = |scope: &str, name: &str| {
            if name.starts_with('.') {
                format!("{}{}", scope, name)
            } else {
                name.to_string()
            }
        };
        for (pos, instr) in instrs.iter().enumerate() {
            if let Instr::Label { name } = instr {
                if !name.starts_with('.') {
                    scope = name;
                }
                let full = full_name(scope, name);
                assert!(
                    labels.insert(full.clone(), pos).is_none(),
                    "label {} defined twice",
                    full
                );
            }
            scopes.push(scope);
        }
        let target = |pc: usize, target: &JumpTarget| match target {
            JumpTarget::Label(name) => {
                let full = full_name(scopes[pc], name);
                *labels
                    .get(&full)
                    .unwrap_or_else(|| panic!("label {} not found", full))
            }
            JumpTarget::Addr(addr) => panic!("jump to address ${:04X} not supported", addr),
        };

        let mut pc = 0;
        let mut steps = 0;
        // What `call` (the instruction to go back to) and `push` (a register pair) put
        // on the stack, which they share as on the CPU
        let mut stack = Vec::new();
        while pc < instrs.len() {
            steps += 1;
            assert!(steps <= 100_000, "the code does not terminate");
            let instr = &instrs[pc];
            if let Err(error) = instr.check() {
                panic!("invalid instruction: {}", error);
            }
            match instr {
                Instr::Ld {
                    dst: Dst::R16(pair),
                    src,
                } => self.load_pair(*pair, src),
                // `ldh` is `ld` with an address from $FF00 (`Instr::check` makes sure)
                Instr::Ld { dst, src } | Instr::Ldh { dst, src } => {
                    let value = self.read(src);
                    self.write(dst, value);
                }
                Instr::Push { pair } => {
                    let saved = self.save(*pair);
                    stack.push(StackEntry::Pair(saved));
                }
                Instr::Pop { pair } => match stack.pop() {
                    Some(StackEntry::Pair(saved)) => self.restore(*pair, saved),
                    Some(StackEntry::Return(_)) => panic!(
                        "pop {}: popping the return address of a call is not supported by the \
                         test CPU",
                        pair
                    ),
                    None => panic!("pop {} with nothing pushed", pair),
                },
                // `add a, src` and `adc a, src` (a + src + carry)
                Instr::Add { src } | Instr::Adc { src } => {
                    let value = self.read_alu(src);
                    let carry = matches!(instr, Instr::Adc { .. }) && self.holds(&Condition::C);
                    let sum = u16::from(self.get(R8::A)) + u16::from(value) + u16::from(carry);
                    let result = sum as u8;
                    self.set(R8::A, result);
                    self.set_zero(result == 0);
                    self.set_carry(sum > 0xFF);
                }
                // `sub a, src`, `sbc a, src` (a - src - carry) and `cp a, src` (the flags
                // of `sub`, `a` unchanged)
                Instr::Sub { src } | Instr::Sbc { src } | Instr::Cp { src } => {
                    let value = self.read_alu(src);
                    let borrow = matches!(instr, Instr::Sbc { .. }) && self.holds(&Condition::C);
                    let difference =
                        i16::from(self.get(R8::A)) - i16::from(value) - i16::from(borrow);
                    let result = difference as u8;
                    self.set_zero(result == 0);
                    self.set_carry(difference < 0);
                    if !matches!(instr, Instr::Cp { .. }) {
                        self.set(R8::A, result);
                    }
                }
                Instr::And { src } | Instr::Xor { src } | Instr::Or { src } => {
                    let value = self.read_alu(src);
                    let a = self.get(R8::A);
                    let result = match instr {
                        Instr::And { .. } => a & value,
                        Instr::Xor { .. } => a ^ value,
                        _ => a | value,
                    };
                    self.set(R8::A, result);
                    self.set_zero(result == 0);
                    self.set_carry(false);
                }
                // `add hl, rr`: the Z flag is unchanged
                Instr::AddHl { src } => {
                    let hl = self.address(R16::HL);
                    let other = self.address(*src);
                    let result = match (hl.is_number(), other.is_number()) {
                        (true, true) => {
                            let (sum, carry) = hl.offset.overflowing_add(other.offset);
                            self.set_carry(carry);
                            Pointer::number(sum)
                        }
                        // A symbol plus a number: the carry depends on the symbol's value
                        (false, true) | (true, false) => {
                            let (base, number) = if hl.is_number() {
                                (other, hl.offset)
                            } else {
                                (hl, other.offset)
                            };
                            self.unknown |= UNKNOWN_CARRY;
                            // 16-bit wrap-around: adding $FFFF is going back one byte
                            base.moved(i32::from(number as i16))
                        }
                        (false, false) => panic!(
                            "add hl, {}: the sum of {} and {} is not supported by the test CPU",
                            src,
                            hl.name(),
                            other.name()
                        ),
                    };
                    self.set_pair(R16::HL, result);
                }
                // 16-bit `inc rr` / `dec rr`: no flag changes
                Instr::Inc {
                    operand: IncDec::R16(pair),
                }
                | Instr::Dec {
                    operand: IncDec::R16(pair),
                } => {
                    let delta = if matches!(instr, Instr::Inc { .. }) {
                        1
                    } else {
                        -1
                    };
                    let moved = self.address(*pair).moved(delta);
                    self.set_pair(*pair, moved);
                }
                // 8-bit `inc` / `dec`, also on `[hl]`: the carry unchanged
                Instr::Inc {
                    operand: IncDec::R8(r8),
                }
                | Instr::Dec {
                    operand: IncDec::R8(r8),
                } => {
                    let value = self.read_r8(*r8);
                    let value = if matches!(instr, Instr::Inc { .. }) {
                        value.wrapping_add(1)
                    } else {
                        value.wrapping_sub(1)
                    };
                    self.write_r8(*r8, value);
                    self.set_zero(value == 0);
                }
                // The rotates on `a`: as `rlc a`, … but Z is always reset
                Instr::Rlca | Instr::Rrca | Instr::Rla | Instr::Rra => {
                    let value = self.get(R8::A);
                    let (result, carry) = self.rotate(instr, value);
                    self.set(R8::A, result);
                    self.set_zero(false);
                    self.set_carry(carry);
                }
                Instr::Rlc { operand }
                | Instr::Rrc { operand }
                | Instr::Rl { operand }
                | Instr::Rr { operand }
                | Instr::Sla { operand }
                | Instr::Sra { operand }
                | Instr::Swap { operand }
                | Instr::Srl { operand } => {
                    let value = self.read_r8(*operand);
                    let (result, carry) = self.rotate(instr, value);
                    self.write_r8(*operand, result);
                    self.set_zero(result == 0);
                    self.set_carry(carry);
                }
                // `bit n, r8`: Z set when the bit is 0, the carry unchanged
                Instr::Bit { bit, operand } => {
                    let value = self.read_r8(*operand);
                    self.set_zero(value & (1 << bit) == 0);
                }
                // `set` and `res`: no flag changes
                Instr::Set { bit, operand } | Instr::Res { bit, operand } => {
                    let value = self.read_r8(*operand);
                    let result = if matches!(instr, Instr::Set { .. }) {
                        value | (1 << bit)
                    } else {
                        value & !(1 << bit)
                    };
                    self.write_r8(*operand, result);
                }
                // `cpl`: Z and the carry unchanged
                Instr::Cpl => {
                    let value = !self.get(R8::A);
                    self.set(R8::A, value);
                }
                Instr::Scf => self.set_carry(true),
                Instr::Ccf => {
                    let carry = self.holds(&Condition::C);
                    self.set_carry(!carry);
                }
                Instr::Jp { target: t } | Instr::Jr { target: t } => {
                    pc = target(pc, t);
                    continue;
                }
                Instr::JpCond {
                    condition,
                    target: t,
                }
                | Instr::JrCond {
                    condition,
                    target: t,
                } => {
                    if self.holds(condition) {
                        pc = target(pc, t);
                        continue;
                    }
                }
                Instr::Call { target: t } | Instr::CallCond { target: t, .. } => {
                    if let Instr::CallCond { condition, .. } = instr {
                        if !self.holds(condition) {
                            pc += 1;
                            continue;
                        }
                    }
                    if let JumpTarget::Label(name) = t {
                        self.trace.push(Event::Call(name.clone()));
                        if self.stubs.contains(name) {
                            // The routine may change every register and flag
                            self.unknown = UNKNOWN_ALL;
                            self.bc = None;
                            self.de = None;
                            self.hl = None;
                            pc += 1;
                            continue;
                        }
                    }
                    stack.push(StackEntry::Return(pc + 1));
                    pc = target(pc, t);
                    continue;
                }
                Instr::Ret | Instr::RetCond { .. } => {
                    if let Instr::RetCond { condition } = instr {
                        if !self.holds(condition) {
                            pc += 1;
                            continue;
                        }
                    }
                    match stack.pop() {
                        Some(StackEntry::Return(back)) => {
                            pc = back;
                            continue;
                        }
                        Some(StackEntry::Pair(_)) => panic!(
                            "{}: returning to a value pushed with push is not supported by the \
                             test CPU",
                            instr
                        ),
                        None => return,
                    }
                }
                Instr::Nop | Instr::Label { .. } | Instr::Comment { .. } => {}
                other => panic!("instruction not supported by the test CPU: {}", other),
            }
            pc += 1;
        }
    }

    /// The result and the carry of the rotate, shift or swap `instr` on `value`; reads the
    /// carry only for the rotates through it (`rl`, `rr`, `rla`, `rra`)
    fn rotate(&self, instr: &Instr, value: u8) -> (u8, bool) {
        let bit7 = value & 0x80 != 0;
        let bit0 = value & 1 != 0;
        match instr {
            Instr::Rlca | Instr::Rlc { .. } => (value.rotate_left(1), bit7),
            Instr::Rrca | Instr::Rrc { .. } => (value.rotate_right(1), bit0),
            Instr::Rla | Instr::Rl { .. } => {
                let carry = u8::from(self.holds(&Condition::C));
                ((value << 1) | carry, bit7)
            }
            Instr::Rra | Instr::Rr { .. } => {
                let carry = u8::from(self.holds(&Condition::C));
                ((value >> 1) | (carry << 7), bit0)
            }
            Instr::Sla { .. } => (value << 1, bit7),
            Instr::Sra { .. } => ((value >> 1) | (value & 0x80), bit0),
            Instr::Swap { .. } => (value.rotate_left(4), false),
            Instr::Srl { .. } => (value >> 1, bit0),
            other => unreachable!("{} is not a rotate", other),
        }
    }

    /// What `push pair` puts on the stack
    fn save(&mut self, pair: R16Stack) -> Saved {
        let unknown_mask = |unknown: bool| if unknown { 0xFF } else { 0 };
        let pair = match pair {
            R16Stack::AF => {
                let flags = (u8::from(self.zero) << 7) | (u8::from(self.carry) << 4);
                // N and H are not modelled: they are always unknown
                let mut unknown_flags = 0b0110_0000;
                if self.unknown & UNKNOWN_ZERO != 0 {
                    unknown_flags |= 0b1000_0000;
                }
                if self.unknown & UNKNOWN_CARRY != 0 {
                    unknown_flags |= 0b0001_0000;
                }
                return Saved {
                    bytes: [self.a, flags],
                    unknown: [unknown_mask(self.unknown & UNKNOWN_A != 0), unknown_flags],
                    pointer: None,
                };
            }
            R16Stack::BC => R16::BC,
            R16Stack::DE => R16::DE,
            R16Stack::HL => R16::HL,
        };
        let (high, low) = Self::halves(pair);
        let unknown_high = self.unknown & Self::unknown_bit(high) != 0;
        let unknown_low = self.unknown & Self::unknown_bit(low) != 0;
        Saved {
            bytes: [*self.reg(high), *self.reg(low)],
            unknown: [unknown_mask(unknown_high), unknown_mask(unknown_low)],
            pointer: self.pair(pair).clone(),
        }
    }

    /// `pop pair` of what [`save`](Self::save) put on the stack (from the same pair or
    /// another one)
    fn restore(&mut self, pair: R16Stack, saved: Saved) {
        let pair = match pair {
            R16Stack::AF => {
                let [a, flags] = saved.bytes;
                let [unknown_a, unknown_flags] = saved.unknown;
                self.set(R8::A, a);
                self.set_zero(flags & 0b1000_0000 != 0);
                self.set_carry(flags & 0b0001_0000 != 0);
                // An address pushed from another pair: its bytes are not known
                let address = saved.pointer.is_some();
                if unknown_a != 0 || address {
                    self.unknown |= UNKNOWN_A;
                }
                if unknown_flags & 0b1000_0000 != 0 || address {
                    self.unknown |= UNKNOWN_ZERO;
                }
                if unknown_flags & 0b0001_0000 != 0 || address {
                    self.unknown |= UNKNOWN_CARRY;
                }
                return;
            }
            R16Stack::BC => R16::BC,
            R16Stack::DE => R16::DE,
            R16Stack::HL => R16::HL,
        };
        if let Some(pointer) = saved.pointer {
            self.set_pair(pair, pointer);
            return;
        }
        let (high, low) = Self::halves(pair);
        for ((half, byte), unknown) in [high, low].into_iter().zip(saved.bytes).zip(saved.unknown) {
            self.set(half, byte);
            if unknown != 0 {
                self.unknown |= Self::unknown_bit(half);
            }
        }
    }

    /// Panics if one of the `bits` is unknown
    fn check_known(&self, bits: u16, what: &dyn std::fmt::Display) {
        assert!(
            self.unknown & bits == 0,
            "{} read while its value is unknown (its register pair was loaded with an \
             address, or a stubbed routine was called)",
            what
        );
    }

    /// Whether `condition` holds with the current flags; panics if that flag is unknown
    fn holds(&self, condition: &Condition) -> bool {
        match condition {
            Condition::Z | Condition::NZ => self.check_known(UNKNOWN_ZERO, &"the Z flag"),
            Condition::C | Condition::NC => self.check_known(UNKNOWN_CARRY, &"the C flag"),
        }
        match condition {
            Condition::Z => self.zero,
            Condition::NZ => !self.zero,
            Condition::C => self.carry,
            Condition::NC => !self.carry,
        }
    }

    fn set_zero(&mut self, zero: bool) {
        self.zero = zero;
        self.unknown &= !UNKNOWN_ZERO;
    }

    fn set_carry(&mut self, carry: bool) {
        self.carry = carry;
        self.unknown &= !UNKNOWN_CARRY;
    }

    /// The unknown bit of the 8-bit register `reg` (not `[hl]`)
    fn unknown_bit(reg: R8) -> u16 {
        match reg {
            R8::A => UNKNOWN_A,
            R8::B => UNKNOWN_B,
            R8::C => UNKNOWN_C,
            R8::D => UNKNOWN_D,
            R8::E => UNKNOWN_E,
            R8::H => UNKNOWN_H,
            R8::L => UNKNOWN_L,
            R8::AtHl => unreachable!("[hl] is memory, not a register"),
        }
    }

    /// The value of the 8-bit register `reg` (not `[hl]`); panics if it is unknown
    fn get(&mut self, reg: R8) -> u8 {
        self.check_known(Self::unknown_bit(reg), &reg);
        *self.reg(reg)
    }

    /// Set the 8-bit register `reg` (not `[hl]`); the pair it belongs to no longer holds an
    /// address
    fn set(&mut self, reg: R8, value: u8) {
        match reg {
            R8::B | R8::C => self.bc = None,
            R8::D | R8::E => self.de = None,
            R8::H | R8::L => self.hl = None,
            _ => {}
        }
        self.unknown &= !Self::unknown_bit(reg);
        *self.reg(reg) = value;
    }

    /// The symbolic address the register pair `pair` holds; panics on `sp`
    fn pair(&mut self, pair: R16) -> &mut Option<Pointer> {
        match pair {
            R16::BC => &mut self.bc,
            R16::DE => &mut self.de,
            R16::HL => &mut self.hl,
            R16::SP => panic!("sp is not supported by the test CPU"),
        }
    }

    /// The 8-bit registers of the register pair `pair`, high then low; panics on `sp`
    fn halves(pair: R16) -> (R8, R8) {
        match pair {
            R16::BC => (R8::B, R8::C),
            R16::DE => (R8::D, R8::E),
            R16::HL => (R8::H, R8::L),
            R16::SP => panic!("sp is not supported by the test CPU"),
        }
    }

    /// The address in the register pair `pair`: its symbolic address, or the number its
    /// two 8-bit registers make; panics if it holds neither
    fn address(&mut self, pair: R16) -> Pointer {
        let (high, low) = Self::halves(pair);
        if let Some(pointer) = self.pair(pair).clone() {
            return pointer;
        }
        let known = self.unknown & (Self::unknown_bit(high) | Self::unknown_bit(low)) == 0;
        assert!(
            known,
            "{} used without an address loaded",
            pair.to_string().to_uppercase()
        );
        Pointer::number(u16::from_be_bytes([*self.reg(high), *self.reg(low)]))
    }

    /// Put the address `pointer` in the register pair `pair`: a number goes into its two
    /// 8-bit registers; with a symbol, they hold an address the model does not know as a
    /// number, so they become unknown
    fn set_pair(&mut self, pair: R16, pointer: Pointer) {
        let (high, low) = Self::halves(pair);
        if pointer.is_number() {
            let [high_byte, low_byte] = pointer.offset.to_be_bytes();
            self.set(high, high_byte);
            self.set(low, low_byte);
        } else {
            self.unknown |= Self::unknown_bit(high) | Self::unknown_bit(low);
            *self.pair(pair) = Some(pointer);
        }
    }

    fn reg(&mut self, reg: R8) -> &mut u8 {
        match reg {
            R8::A => &mut self.a,
            R8::B => &mut self.b,
            R8::C => &mut self.c,
            R8::D => &mut self.d,
            R8::E => &mut self.e,
            R8::H => &mut self.h,
            R8::L => &mut self.l,
            R8::AtHl => unreachable!("[hl] is memory, not a register"),
        }
    }

    /// The value of the expression `e`, reading its symbols in [`TestCpu::consts`]; or
    /// `None` if it holds a symbol (or raw text) that is not set there
    fn eval(&self, e: &Expr) -> Option<i32> {
        if let Some(value) = self.consts.get(&e.to_string()) {
            return Some(i32::from(*value));
        }
        match e {
            Expr::Num(value, _) => Some(*value),
            Expr::Sym(_) | Expr::Raw(_) => None,
            Expr::Neg(e) => self.eval(e).map(i32::wrapping_neg),
            Expr::Not(e) => self.eval(e).map(|v| !v),
            Expr::Low(e) => self.eval(e).map(|v| v & 0xFF),
            Expr::High(e) => self.eval(e).map(|v| (v >> 8) & 0xFF),
            Expr::Binary(op, left, right) => {
                let constant = Expr::Binary(
                    *op,
                    Box::new(Expr::num(self.eval(left)?)),
                    Box::new(Expr::num(self.eval(right)?)),
                );
                constant.value()
            }
        }
    }

    /// The byte an 8-bit value operand gives: -128 to 255 (`-1` is `$FF`); panics on a
    /// symbol not set in [`TestCpu::consts`], or a value out of that range (rgbasm would
    /// truncate it)
    fn value8(&self, e: &Expr) -> u8 {
        let value = self
            .eval(e)
            .unwrap_or_else(|| panic!("constant {} not set in the test CPU", e));
        match value {
            0..=0xFF => value as u8,
            -0x80..=-1 => value as u8,
            _ => panic!("{} = {} is not an 8-bit value (-128 to 255)", e, value),
        }
    }

    /// The address `e`: a number, a symbol, or a symbol plus or minus constant offsets
    /// (`_OAMRAM+4+1` is `_OAMRAM` + 5); panics on anything else
    fn pointer(&self, e: &Expr) -> Pointer {
        self.try_pointer(e)
            .unwrap_or_else(|| panic!("address {} not supported by the test CPU", e))
    }

    /// [`pointer`](Self::pointer), or `None` for an expression that is not an address
    fn try_pointer(&self, e: &Expr) -> Option<Pointer> {
        if let Some(value) = e.value() {
            // A 16-bit value, -32768 to 65535 (`-1` is $FFFF)
            if !(-0x8000..=0xFFFF).contains(&value) {
                panic!("address {} = {} is not a 16-bit value", e, value);
            }
            return Some(Pointer::number(value as u16));
        }
        match e {
            Expr::Sym(name) => Some(Pointer::symbol(name)),
            Expr::Binary(BinOp::Add, left, right) => match (left.value(), right.value()) {
                (_, Some(offset)) => Some(self.try_pointer(left)?.moved(offset)),
                (Some(offset), _) => Some(self.try_pointer(right)?.moved(offset)),
                _ => None,
            },
            Expr::Binary(BinOp::Sub, left, right) => {
                Some(self.try_pointer(left)?.moved(-right.value()?))
            }
            _ => None,
        }
    }

    /// The address of the memory operand `mem`; `[hli]` and `[hld]` then move `hl`
    fn access(&mut self, mem: &Mem) -> Pointer {
        match mem {
            Mem::Bc => self.address(R16::BC),
            Mem::De => self.address(R16::DE),
            Mem::Hli | Mem::Hld => {
                let hl = self.address(R16::HL);
                let delta = if *mem == Mem::Hli { 1 } else { -1 };
                self.set_pair(R16::HL, hl.moved(delta));
                hl
            }
            // `ldh [c]`: the byte at $FF00 + c
            Mem::C => {
                let c = self.get(R8::C);
                Pointer::number(0xFF00 + u16::from(c))
            }
            Mem::Addr(address) => self.pointer(address),
        }
    }

    fn read_memory(&self, at: &Pointer) -> u8 {
        let name = at.name();
        *self
            .mem
            .0
            .get(&name)
            .unwrap_or_else(|| panic!("read of [{}], which was never written", name))
    }

    fn write_memory(&mut self, at: &Pointer, value: u8) {
        let name = at.name();
        self.trace.push(Event::Write(name.clone(), value));
        self.mem.0.insert(name, value);
    }

    /// The value of an 8-bit register or `[hl]`
    fn read_r8(&mut self, r8: R8) -> u8 {
        match r8 {
            R8::AtHl => {
                let hl = self.address(R16::HL);
                self.read_memory(&hl)
            }
            reg => self.get(reg),
        }
    }

    /// Set an 8-bit register or `[hl]`
    fn write_r8(&mut self, r8: R8, value: u8) {
        match r8 {
            R8::AtHl => {
                let hl = self.address(R16::HL);
                self.write_memory(&hl, value);
            }
            reg => self.set(reg, value),
        }
    }

    /// The byte the source of an 8-bit load gives
    fn read(&mut self, operand: &Operand) -> u8 {
        match operand {
            Operand::R8(r8) => self.read_r8(*r8),
            Operand::Imm(value) => self.value8(value),
            Operand::Mem(mem) => {
                let at = self.access(mem);
                self.read_memory(&at)
            }
            Operand::R16(r16) => panic!("ld …, {} is not supported by the test CPU", r16),
        }
    }

    /// The byte the source of an 8-bit ALU instruction gives
    fn read_alu(&mut self, operand: &AluOperand) -> u8 {
        match operand {
            AluOperand::R8(r8) => self.read_r8(*r8),
            AluOperand::Imm(value) => self.value8(value),
        }
    }

    /// `ld rr, n16`: put a number (`$9800`, an expression of [`TestCpu::consts16`]) or a
    /// symbolic address (`_OAMRAM+4`) in the register pair `rr`. Any other value
    /// (`TilesEnd - Tiles` not in [`TestCpu::consts16`]) is one the model does not know:
    /// the pair and its two registers become unknown, so using them panics.
    fn load_pair(&mut self, pair: R16, src: &Operand) {
        let Operand::Imm(value) = src else {
            panic!("ld {}, {} not supported by the test CPU", pair, src);
        };
        let pointer = match self.consts16.get(&value.to_string()) {
            Some(number) => Some(Pointer::number(*number)),
            None => self.try_pointer(value),
        };
        match pointer {
            Some(pointer) => self.set_pair(pair, pointer),
            None => {
                let (high, low) = Self::halves(pair);
                self.unknown |= Self::unknown_bit(high) | Self::unknown_bit(low);
                *self.pair(pair) = None;
            }
        }
    }

    /// Store `value` to the destination of a load
    fn write(&mut self, dst: &Dst, value: u8) {
        match dst {
            Dst::R8(r8) => self.write_r8(*r8, value),
            Dst::Mem(mem) => {
                let at = self.access(mem);
                self.write_memory(&at, value);
            }
            Dst::R16(r16) => panic!("ld {}, … is not supported by the test CPU", r16),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::Asm;

    #[test]
    fn test_call_and_ret() {
        let mut asm = Asm::new();
        asm.call("Double")
            .call("Double")
            .ld(R8::B, R8::A)
            .ret() // ends the run: no call to return to
            .ld_a(99)
            .label("Double")
            .add(R8::A)
            .ret();
        let mut cpu = TestCpu {
            a: 3,
            ..TestCpu::default()
        };
        cpu.run(&asm.get_main_instrs());
        assert_eq!((cpu.a, cpu.b), (12, 12));
    }

    #[test]
    fn test_conditional_ret_and_inc_dec() {
        let mut asm = Asm::new();
        asm.dec(R8::A).ret_cond(Condition::Z).inc(R8::B);
        let code = asm.get_main_instrs();
        let mut cpu = TestCpu {
            a: 1,
            ..TestCpu::default()
        };
        cpu.run(&code);
        assert_eq!((cpu.a, cpu.b, cpu.zero), (0, 0, true), "returned on zero");
        cpu.run(&code);
        assert_eq!((cpu.a, cpu.b, cpu.zero), (255, 1, false), "went on");
    }

    #[test]
    fn test_register_pairs_stubs_and_trace() {
        let mut asm = Asm::new();
        asm.ld(R16::HL, "_OAMRAM")
            .ld_a(7)
            .ld(Mem::Hli, R8::A)
            .ld(Mem::Hli, R8::A)
            .ld_a(9)
            .ld(R8::AtHl, R8::A)
            .ld_a(0)
            .ld(R8::B, R8::AtHl)
            .call("Memcopy");
        let mut cpu = TestCpu::default();
        cpu.stubs.insert("Memcopy".to_string());
        cpu.run(&asm.get_main_instrs());
        assert_eq!(cpu.b, 9, "read back through hl");
        assert_eq!(
            cpu.trace,
            [
                Event::Write("_OAMRAM".to_string(), 7),
                Event::Write("_OAMRAM+1".to_string(), 7),
                Event::Write("_OAMRAM+2".to_string(), 9),
                Event::Call("Memcopy".to_string()),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "HL used without an address loaded")]
    fn test_changing_half_of_a_pair_forgets_its_address() {
        let mut asm = Asm::new();
        asm.ld(R16::HL, "_OAMRAM").ld(R8::L, 4).ld(R8::AtHl, R8::A);
        TestCpu::default().run(&asm.get_main_instrs());
    }

    /// Whether running `code` on a fresh CPU with the stub `Memcopy` panics
    fn panics(code: &Asm) -> bool {
        let instrs = code.get_main_instrs();
        std::panic::catch_unwind(move || {
            let mut cpu = TestCpu::default();
            cpu.stubs.insert("Memcopy".to_string());
            cpu.run(&instrs);
        })
        .is_err()
    }

    /// Code appended to a test program
    type Snippet = fn(&mut Asm);

    fn ld_pair(asm: &mut Asm, pair: R16, address: impl Into<Expr>) {
        asm.ld(pair, address.into());
    }

    #[test]
    fn test_loading_a_pair_makes_its_halves_unknown() {
        let pairs = [
            (R16::BC, R8::B, R8::C),
            (R16::DE, R8::D, R8::E),
            (R16::HL, R8::H, R8::L),
        ];
        for (pair, high, low) in pairs {
            for half in [high, low] {
                let mut asm = Asm::new();
                ld_pair(&mut asm, pair, "_OAMRAM");
                asm.ld(R8::A, half);
                assert!(panics(&asm), "{:?} read after ld {:?}", half, pair);

                // Set again, the half is known (and the pair no longer holds an address)
                let mut asm = Asm::new();
                ld_pair(&mut asm, pair, "_OAMRAM");
                asm.ld(half, 3).ld(R8::A, half);
                assert!(!panics(&asm), "{:?} set after ld {:?}", half, pair);
            }
        }
    }

    #[test]
    fn test_a_stub_leaves_registers_pairs_and_flags_unknown() {
        let after_stub = |tail: &dyn Fn(&mut Asm)| {
            let mut asm = Asm::new();
            ld_pair(&mut asm, R16::HL, "_OAMRAM");
            ld_pair(&mut asm, R16::DE, "Tiles");
            ld_pair(&mut asm, R16::BC, Expr::sym("TilesEnd") - "Tiles");
            asm.ld_a(1).cp_imm(1).ld_b(2).call("Memcopy");
            tail(&mut asm);
            asm
        };
        let reads: [(&str, Snippet); 7] = [
            ("a", |asm| {
                asm.ld(R8::B, R8::A);
            }),
            ("b", |asm| {
                asm.ld(R8::A, R8::B);
            }),
            ("hl", |asm| {
                asm.ld(R8::AtHl, 0);
            }),
            ("de", |asm| {
                asm.ld(R8::A, Mem::De);
            }),
            ("bc", |asm| {
                asm.ld(R8::A, Mem::Bc);
            }),
            ("Z flag", |asm| {
                asm.jp_cond(Condition::NZ, "End").label("End");
            }),
            ("C flag", |asm| {
                asm.jp_cond(Condition::C, "End").label("End");
            }),
        ];
        for (what, read) in reads {
            assert!(panics(&after_stub(&read)), "{} read after a stub", what);
        }
        // Writing them again is fine
        assert!(!panics(&after_stub(&|asm| {
            asm.ld_a(0)
                .cp_imm(1)
                .jp_cond(Condition::C, "End")
                .label("End");
        })));
    }

    #[test]
    fn test_one_address_has_one_name() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::HL, Expr::sym("_OAMRAM") + 4);
        asm.ld_a(7)
            .ld(Mem::Hli, R8::A)
            .ld_a(8)
            .ld(Mem::Hli, R8::A)
            .ld_a(9)
            .ld_addr_def_a(Expr::sym("_OAMRAM") + 4 + 2)
            .ld_a(0)
            .ld_addr_def_a(Expr::sym("_OAMRAM") + 0)
            .ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        assert_eq!(cpu.a, 8, "[_OAMRAM+5] written through hl, read directly");
        let names: Vec<_> = cpu.mem.keys().cloned().collect();
        assert_eq!(names, ["_OAMRAM", "_OAMRAM+4", "_OAMRAM+5", "_OAMRAM+6"]);
    }

    #[test]
    fn test_local_labels_belong_to_their_global_label() {
        // Each routine has its own .done; `jr .done` stays in its routine
        let mut asm = Asm::new();
        asm.call("SetOne")
            .call("SetTwo")
            .ret()
            .label("SetOne")
            .ld_b(1)
            .jr(".done")
            .ld_b(0)
            .label(".done")
            .ret()
            .label("SetTwo")
            .ld_c(2)
            .jr(".done")
            .ld_c(0)
            .label(".done")
            .ret();
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        assert_eq!((cpu.b, cpu.c), (1, 2));
    }

    #[test]
    fn test_a_pair_holds_a_number() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::BC, "$9800");
        ld_pair(&mut asm, R16::DE, Expr::sym("LenEnd") - "Len");
        // hl set through its halves: $12FF
        asm.ld(R8::H, 0x12)
            .ld(R8::L, 0xFF)
            .ld_a(7)
            .ld(Mem::Hli, R8::A)
            .ld(Mem::Hli, R8::A)
            .ld(R8::A, Mem::Bc);
        let mut cpu = TestCpu::default();
        cpu.consts16.insert("LenEnd - Len".to_string(), 300);
        cpu.mem.insert("$9800".to_string(), 9);
        cpu.run(&asm.get_main_instrs());
        assert_eq!((cpu.b, cpu.c), (0x98, 0x00), "ld bc, $9800");
        assert_eq!((cpu.d, cpu.e), (0x01, 0x2C), "ld de, 300");
        assert_eq!((cpu.h, cpu.l), (0x13, 0x01), "[hli] carries into h");
        assert_eq!(cpu.a, 9, "read through bc");
        assert_eq!(
            cpu.trace,
            [
                Event::Write("$12FF".to_string(), 7),
                Event::Write("$1300".to_string(), 7),
            ]
        );
        // One name per address: a number plus an offset, in any base
        assert_eq!(cpu.mem.get("$9800+0"), Some(&9));
        assert_eq!(cpu.mem.get("0x12FF"), Some(&7));
        assert_eq!(cpu.mem.get("%1001100000000000"), Some(&9));
        assert_eq!(cpu.mem.get("4863+1"), Some(&7), "$12FF + 1");
        // Offsets in any base too: $9800+$21 is $9821, _OAMRAM+%101 is _OAMRAM+5
        assert_eq!(normalize("$9800+$21"), "$9821");
        assert_eq!(normalize("$9800 + 0x10 + 17"), "$9821");
        assert_eq!(normalize("_OAMRAM+%101"), "_OAMRAM+5");
    }

    #[test]
    fn test_every_rgbds_number_form() {
        for (text, value) in [
            ("$9821", 0x9821),
            ("0x9821", 0x9821),
            ("0X9821", 0x9821),
            ("%1001", 9),
            ("0b1001", 9),
            ("&17", 15),
            ("0o17", 15),
            ("38945", 38945),
            ("$98_21", 0x9821),
            ("1_000", 1000),
            ("-1", 0xFFFF),
            ("-$10", 0xFFF0),
        ] {
            assert_eq!(parse_number(text), Some(value), "{}", text);
        }
        // Symbols are not numbers
        for text in ["_OAMRAM", "wCurKeys", "TilesEnd - Tiles", "-Offset"] {
            assert_eq!(parse_number(text), None, "{}", text);
        }
        // Text that starts like a number but is not one panics
        for text in ["$98G0", "$9800 + X", "70000", "0b102", "$", "1+2"] {
            let result = std::panic::catch_unwind(|| parse_number(text));
            assert!(result.is_err(), "{} should panic", text);
        }
    }

    /// Run `ld a, value` on a CPU whose constants are `consts`
    fn load_a(value: Expr, consts: &[(&str, u8)]) -> u8 {
        let mut cpu = TestCpu::default();
        for (name, value) in consts {
            cpu.consts.insert(name.to_string(), *value);
        }
        cpu.run(&[Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::Imm(value),
        }]);
        cpu.a
    }

    #[test]
    fn test_an_8_bit_value() {
        // A number, in any form (`ld a, -1`, as `Var::set` emits it, is the byte $FF)
        let numbers = [
            (Expr::num(-1), 0xFF),
            (Expr::num(-128), 0x80),
            (Expr::hex(0x10), 0x10),
            (Expr::num(255), 0xFF),
            (Expr::from("-0"), 0),
            (Expr::from("-$7F"), 0x81),
            (Expr::bin(0b1010) + 1, 11),
            (Expr::low(Expr::hex(0x1234)), 0x34),
        ];
        for (value, byte) in numbers {
            assert_eq!(load_a(value.clone(), &[]), byte, "{}", value);
        }
        // Symbols of `consts`, and expressions of them; a whole expression can be set
        let consts = [("ON", 0x80), ("BG", 0x01), ("ON | BG | OBJ", 0x83)];
        let on_bg = Expr::sym("ON") | "BG";
        assert_eq!(load_a(on_bg, &consts), 0x81);
        let whole = Expr::sym("ON") | "BG" | "OBJ";
        assert_eq!(load_a(whole, &consts), 0x83, "by its text");
        assert_eq!(load_a(Expr::sym("ON") - 1, &consts), 0x7F);

        // Out of the 8-bit range: rgbasm would truncate it, the model panics (a constant
        // is rejected by `Instr::check`, an expression of symbols by the model itself);
        // and so does a symbol that is not set
        let out_of_range = [
            Expr::num(256),
            Expr::num(-129),
            Expr::hex(0xFFFF),
            Expr::num(-65535),
            Expr::sym("ON") + "ON",
            Expr::sym("UNSET"),
            Expr::raw("BANK(ON)"),
        ];
        for value in out_of_range {
            let text = value.to_string();
            let result = std::panic::catch_unwind(|| load_a(value, &consts));
            assert!(result.is_err(), "{} should panic", text);
        }
    }

    #[test]
    fn test_add_hl() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::HL, "$8000");
        ld_pair(&mut asm, R16::BC, "$0005");
        asm.ld_a(1)
            .cp_imm(1) // Z set, C clear
            .add_hl(R16::HL) // $0000, carry
            .ld(R8::D, R8::H)
            .ld(R8::E, R8::L)
            .ld_a(0)
            .jp_cond(Condition::NC, "End")
            .jp_cond(Condition::NZ, "End")
            .ld_a(1) // carry set and Z unchanged
            .add_hl(R16::BC) // $0005, no carry
            .jp_cond(Condition::C, "End")
            .ld_b(2)
            .label("End");
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        assert_eq!((cpu.d, cpu.e), (0, 0));
        assert_eq!((cpu.a, cpu.b), (1, 2), "flags of add hl");
        assert_eq!((cpu.h, cpu.l), (0x00, 0x05));

        // A symbol plus a number is that symbol at an offset; its carry is unknown
        let symbol_plus_number = |tail: &dyn Fn(&mut Asm)| {
            let mut asm = Asm::new();
            ld_pair(&mut asm, R16::HL, "33");
            ld_pair(&mut asm, R16::BC, "_SCRN0");
            asm.add_hl(R16::BC);
            tail(&mut asm);
            asm
        };
        let mut cpu = TestCpu::default();
        cpu.mem.insert("_SCRN0+33".to_string(), 4);
        cpu.run(
            &symbol_plus_number(&|asm| {
                asm.ld(R8::A, R8::AtHl);
            })
            .get_main_instrs(),
        );
        assert_eq!(cpu.a, 4);
        assert!(panics(&symbol_plus_number(&|asm| {
            asm.jp_cond(Condition::C, "End").label("End");
        })));

        // A number from $8000 is a step back, as the sum wraps around at 16 bits:
        // _SCRN0+5 + $FFFF is _SCRN0+4
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::HL, Expr::sym("_SCRN0") + 5);
        ld_pair(&mut asm, R16::DE, "$FFFF");
        asm.add_hl(R16::DE).ld(R8::A, R8::AtHl);
        let mut cpu = TestCpu::default();
        cpu.mem.insert("_SCRN0+4".to_string(), 8);
        cpu.run(&asm.get_main_instrs());
        assert_eq!(cpu.a, 8);

        // Two symbols cannot be added
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::HL, "_SCRN0");
        asm.add_hl(R16::HL);
        assert!(panics(&asm));
    }

    #[test]
    fn test_16_bit_inc_and_dec() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::DE, "Tiles");
        ld_pair(&mut asm, R16::BC, "$0000");
        asm.ld_a(1)
            .cp_imm(1) // Z set: inc rr / dec rr leave the flags alone
            .inc(R16::DE)
            .inc(R16::DE)
            .dec(R16::DE)
            .dec(R16::BC)
            .ld(R8::A, Mem::De)
            .jp_cond(Condition::NZ, "End")
            .ld_h(1)
            .label("End");
        let mut cpu = TestCpu::default();
        cpu.mem.insert("Tiles+1".to_string(), 6);
        cpu.run(&asm.get_main_instrs());
        assert_eq!(cpu.a, 6, "[Tiles+1]");
        assert_eq!((cpu.b, cpu.c), (0xFF, 0xFF), "$0000 - 1");
        assert_eq!(cpu.h, 1, "Z unchanged");

        // Before a symbol, the address is unknown
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::DE, "Tiles");
        asm.dec(R16::DE);
        assert!(panics(&asm));
    }

    #[test]
    fn test_srl_adc_and_or() {
        let mut asm = Asm::new();
        asm.ld_a(0b11)
            .srl(R8::A) // 1, carry
            .ld(R8::B, R8::A)
            .srl(R8::A) // 0, carry, zero
            .ld_a(0xFF)
            .adc(0) // 0xFF + 0 + 1 = 0, carry
            .ld(R8::C, R8::A)
            .adc(1) // 0 + 1 + 1 = 2, no carry
            .ld(R8::D, R8::A)
            .adc(3) // 2 + 3 + 0
            .ld(R8::E, R8::A)
            .ld_a(0)
            .or(0) // zero, carry cleared
            .jp_cond(Condition::NZ, "End")
            .jp_cond(Condition::C, "End")
            .or(R8::B) // 1
            .label("End");
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        assert_eq!(
            (cpu.b, cpu.c, cpu.d, cpu.e, cpu.a),
            (1, 0, 2, 5, 1),
            "srl, adc with and without carry, or"
        );
        assert!(!cpu.zero && !cpu.carry);
    }

    /// Run `a = value`, the carry set to `carry`, then `code`
    fn run_with(value: u8, carry: bool, code: &dyn Fn(&mut Asm)) -> TestCpu {
        let mut asm = Asm::new();
        asm.ld_a(value).scf();
        if !carry {
            asm.ccf();
        }
        code(&mut asm);
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        cpu
    }

    #[test]
    fn test_sub_sbc_cp_and_xor() {
        // (a, carry in, code, a after, carry after, zero after)
        let cases: [(u8, bool, Snippet, u8, bool, bool); 9] = [
            (
                0x10,
                true,
                |asm| {
                    asm.sbc(0x0F);
                },
                0x00,
                false,
                true,
            ),
            (
                0x00,
                true,
                |asm| {
                    asm.sbc(0x00);
                },
                0xFF,
                true,
                false,
            ),
            (
                0x05,
                false,
                |asm| {
                    asm.sbc(0x03);
                },
                0x02,
                false,
                false,
            ),
            (
                0x05,
                true,
                |asm| {
                    asm.sbc(0x05);
                },
                0xFF,
                true,
                false,
            ),
            (
                0x05,
                true,
                |asm| {
                    asm.sbc(0x04);
                },
                0x00,
                false,
                true,
            ),
            // `sub` ignores the carry, `cp` leaves `a`
            (
                0x05,
                true,
                |asm| {
                    asm.sub(0x05);
                },
                0x00,
                false,
                true,
            ),
            (
                0x05,
                false,
                |asm| {
                    asm.cp(0x06);
                },
                0x05,
                true,
                false,
            ),
            // `xor` clears the carry
            (
                0b1100,
                true,
                |asm| {
                    asm.xor(0b1010);
                },
                0b0110,
                false,
                false,
            ),
            (
                0x5A,
                true,
                |asm| {
                    asm.xor(R8::A);
                },
                0x00,
                false,
                true,
            ),
        ];
        for (index, (a, carry, code, result, carry_after, zero_after)) in
            cases.into_iter().enumerate()
        {
            let cpu = run_with(a, carry, &code);
            assert_eq!(
                (cpu.a, cpu.carry, cpu.zero),
                (result, carry_after, zero_after),
                "case {}",
                index
            );
        }
    }

    #[test]
    fn test_rotates_shifts_and_swap() {
        // (instruction, a, carry in, a after, carry after, zero after); the `$CB` forms
        // set Z from the result, `rlca`, `rrca`, `rla` and `rra` always reset it
        let cases: [(&str, Snippet, u8, bool, u8, bool, bool); 14] = [
            (
                "rlc",
                |asm| {
                    asm.rlc(R8::A);
                },
                0x85,
                false,
                0x0B,
                true,
                false,
            ),
            (
                "rrc",
                |asm| {
                    asm.rrc(R8::A);
                },
                0x01,
                false,
                0x80,
                true,
                false,
            ),
            (
                "rl",
                |asm| {
                    asm.rl(R8::A);
                },
                0x80,
                false,
                0x00,
                true,
                true,
            ),
            (
                "rl",
                |asm| {
                    asm.rl(R8::A);
                },
                0x01,
                true,
                0x03,
                false,
                false,
            ),
            (
                "rr",
                |asm| {
                    asm.rr(R8::A);
                },
                0x01,
                false,
                0x00,
                true,
                true,
            ),
            (
                "rr",
                |asm| {
                    asm.rr(R8::A);
                },
                0x02,
                true,
                0x81,
                false,
                false,
            ),
            (
                "sla",
                |asm| {
                    asm.sla(R8::A);
                },
                0xC0,
                false,
                0x80,
                true,
                false,
            ),
            (
                "sra",
                |asm| {
                    asm.sra(R8::A);
                },
                0x81,
                false,
                0xC0,
                true,
                false,
            ),
            (
                "swap",
                |asm| {
                    asm.swap(R8::A);
                },
                0xF0,
                true,
                0x0F,
                false,
                false,
            ),
            (
                "srl",
                |asm| {
                    asm.srl(R8::A);
                },
                0x81,
                false,
                0x40,
                true,
                false,
            ),
            (
                "rlca",
                |asm| {
                    asm.rlca();
                },
                0x80,
                false,
                0x01,
                true,
                false,
            ),
            (
                "rla",
                |asm| {
                    asm.rla();
                },
                0x80,
                false,
                0x00,
                true,
                false,
            ),
            (
                "rrca",
                |asm| {
                    asm.rrca();
                },
                0x01,
                false,
                0x80,
                true,
                false,
            ),
            (
                "rra",
                |asm| {
                    asm.rra();
                },
                0x01,
                false,
                0x00,
                true,
                false,
            ),
        ];
        for (name, code, a, carry, result, carry_after, zero_after) in cases {
            let cpu = run_with(a, carry, &code);
            assert_eq!(
                (cpu.a, cpu.carry, cpu.zero),
                (result, carry_after, zero_after),
                "{} of {:#04x}, carry {}",
                name,
                a,
                carry
            );
        }

        // On [hl] the result is written back to memory
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::HL, "wValue");
        asm.sra(R8::AtHl).swap(R8::AtHl);
        let mut cpu = TestCpu::default();
        cpu.mem.insert("wValue".to_string(), 0x82);
        cpu.run(&asm.get_main_instrs());
        assert_eq!(
            cpu.trace,
            [
                Event::Write("wValue".to_string(), 0xC1),
                Event::Write("wValue".to_string(), 0x1C),
            ]
        );
    }

    #[test]
    fn test_bit_set_res_cpl_scf_ccf() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::HL, "wFlags");
        asm.scf()
            .bit(2, R8::AtHl) // bit set: Z reset, carry unchanged
            .ld_a(0)
            .jp_cond(Condition::Z, "End")
            .jp_cond(Condition::NC, "End")
            .bit(3, R8::AtHl) // bit clear: Z set
            .jp_cond(Condition::NZ, "End")
            .set(7, R8::AtHl) // $84
            .res(2, R8::AtHl) // $80
            .ld_b(0)
            .set(0, R8::B)
            .set(5, R8::B)
            .res(0, R8::B) // $20
            .ld_a(0x0F)
            .cpl() // $F0, Z and carry unchanged
            .jp_cond(Condition::NZ, "End")
            .ccf() // carry clear
            .jp_cond(Condition::C, "End")
            .ccf() // carry set
            .ld_c(1)
            .label("End");
        let mut cpu = TestCpu::default();
        cpu.mem.insert("wFlags".to_string(), 0b0000_0100);
        cpu.run(&asm.get_main_instrs());
        assert_eq!((cpu.a, cpu.b, cpu.c), (0xF0, 0x20, 1));
        assert!(cpu.zero && cpu.carry);
        assert_eq!(
            cpu.trace,
            [
                Event::Write("wFlags".to_string(), 0x84),
                Event::Write("wFlags".to_string(), 0x80),
            ]
        );
    }

    #[test]
    fn test_push_and_pop() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, R16::BC, "$1234");
        ld_pair(&mut asm, R16::HL, "_OAMRAM");
        asm.push(R16Stack::BC)
            .push(R16Stack::HL)
            .ld_a(5)
            .cp_imm(5) // Z set, carry clear
            .push(R16Stack::AF)
            .ld_a(0)
            .cp_imm(1) // Z clear, carry set
            .call("Routine")
            .pop(R16Stack::AF) // a = 5, Z set, carry clear
            .pop(R16Stack::DE) // the address in hl
            .pop(R16Stack::HL) // $1234
            .ld(Mem::De, R8::A)
            .ret()
            // A routine that keeps bc
            .label("Routine")
            .push(R16Stack::BC)
            .ld_b(9)
            .pop(R16Stack::BC)
            .ret();
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        assert_eq!(
            (cpu.a, cpu.h, cpu.l, cpu.b, cpu.c),
            (5, 0x12, 0x34, 0x12, 0x34)
        );
        assert!(cpu.zero && !cpu.carry);
        assert_eq!(
            cpu.trace.last(),
            Some(&Event::Write("_OAMRAM".to_string(), 5))
        );

        // `pop bc` after `push af`: b is a, c holds the flags, N and H included, which
        // the model does not know
        let push_af_pop_bc = |read: R8| {
            let mut asm = Asm::new();
            asm.ld_a(3)
                .push(R16Stack::AF)
                .pop(R16Stack::BC)
                .ld(R8::E, read);
            asm
        };
        assert!(!panics(&push_af_pop_bc(R8::B)), "b is a");
        assert!(panics(&push_af_pop_bc(R8::C)), "c is unknown");
        let mut cpu = TestCpu::default();
        cpu.run(&push_af_pop_bc(R8::B).get_main_instrs());
        assert_eq!(cpu.e, 3);

        // The stack is shared with the calls: a `ret` to a pushed value, a `pop` of a
        // return address, or a `pop` with nothing pushed is not supported
        let mut asm = Asm::new();
        asm.push(R16Stack::BC).ret();
        assert!(panics(&asm));
        let mut asm = Asm::new();
        asm.call("Routine").ret().label("Routine").pop(R16Stack::BC);
        assert!(panics(&asm));
        let mut asm = Asm::new();
        asm.pop(R16Stack::DE);
        assert!(panics(&asm));
    }

    #[test]
    fn test_conditional_call() {
        let mut asm = Asm::new();
        asm.ld_a(1)
            .cp_imm(1) // Z set
            .call_cond(Condition::NZ, "SetB") // not taken
            .call_cond(Condition::Z, "SetC") // taken
            .ret()
            .label("SetB")
            .ld_b(1)
            .ret()
            .label("SetC")
            .ld_c(2)
            .ret();
        let mut cpu = TestCpu::default();
        cpu.run(&asm.get_main_instrs());
        assert_eq!((cpu.b, cpu.c), (0, 2));
        assert_eq!(cpu.trace, [Event::Call("SetC".to_string())]);
    }

    #[test]
    fn test_unmodelled_instructions_panic() {
        // Interrupts, the stack pointer, `daa` (needs the N and H flags) and the jumps to
        // an address in a register are not modelled
        let unmodelled: [Snippet; 11] = [
            |asm| {
                asm.halt();
            },
            |asm| {
                asm.stop();
            },
            |asm| {
                asm.di();
            },
            |asm| {
                asm.ei();
            },
            |asm| {
                asm.reti();
            },
            |asm| {
                asm.rst(0x38);
            },
            |asm| {
                asm.daa();
            },
            |asm| {
                asm.jp_hl();
            },
            |asm| {
                asm.add_sp(1);
            },
            |asm| {
                asm.ld_hl_sp(1);
            },
            |asm| {
                asm.ld_hl(0).add_hl(R16::SP);
            },
        ];
        for code in unmodelled {
            let mut asm = Asm::new();
            asm.ld_a(0);
            code(&mut asm);
            assert!(panics(&asm), "{}", asm.get_main_instrs().last().unwrap());
        }
        // `nop` does nothing
        let mut asm = Asm::new();
        asm.nop();
        assert!(!panics(&asm));
    }
}
