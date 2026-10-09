//! A tiny model of the Game Boy CPU for unit tests.
//!
//! It runs the [`Instr`]s a routine emits, so a test can check what the generated code
//! does rather than how it looks. It models the 8-bit registers, the Z and C flags, a
//! memory addressed by symbol (`[wCurKeys]`, `[_OAMRAM+1]`, …) and symbolic constants
//! (`PADF_LEFT`). A memory symbol plus an offset (`+4`, `+$4`, `+%100`) has one name however it is
//! written: `_OAMRAM+4+1`, `_OAMRAM + 5` and `_OAMRAM+5` are the same byte, `_OAMRAM+0`
//! is `_OAMRAM`. A number is an address too, named `$XXXX`: `$9800+33` is `$9821`.
//! The register pairs `bc`, `de` and `hl` hold a symbolic address
//! ([`Pointer`]): `ld hl, _OAMRAM` then `ld [hli], a` writes `[_OAMRAM]`, then
//! `[_OAMRAM+1]`, the names a direct access such as `ld [_OAMRAM+1], a` uses. Loading a
//! pair with a symbol makes its two 8-bit registers unknown, and changing one of them
//! makes the pair unknown; reading an unknown register, or using an unknown pair, panics.
//! A pair loaded with a number (`ld bc, $9800`, or a symbol of [`TestCpu::consts16`]),
//! or whose two registers are known, holds that number, as on the CPU: `ld h, 0` then
//! `ld l, a` sets `hl`. 16-bit `add hl, rr`, `inc rr` and `dec rr` work on numbers and on
//! a symbol plus a number (the carry of `add hl` is then unknown). Jumps and
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

use super::{Condition, Instr, JumpTarget, Operand, Register};

/// The address held by a register pair: a symbol plus an offset in bytes, or a number
/// (an empty symbol, the number in `offset`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pointer {
    pub symbol: String,
    pub offset: u16,
}

impl Pointer {
    /// The address `symbol`, split into a base symbol and offsets (any RGBDS number):
    /// `_OAMRAM+4+1` and `_OAMRAM+$5` are `_OAMRAM` + 5; a number (`$9800`, `0x9800`, `%1001`, `38912`) has
    /// no symbol
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
            None => Pointer {
                symbol: symbol.to_string(),
                offset: 0,
            },
        }
    }

    /// The address `number`
    fn number(number: u16) -> Pointer {
        Pointer {
            symbol: String::new(),
            offset: number,
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

/// The value of an RGBDS number
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

/// The byte an 8-bit operand written as the number `text` assembles to: 0 to 255, or
/// -128 to -1 (`-1` is `$FF`); anything else panics (rgbasm would truncate it with a
/// warning). The range is checked on the signed value, before [`parse_number`] wraps a
/// negative number to 16 bits (`-65535` would wrap to 1).
fn imm8(text: &str) -> u8 {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let magnitude = parse_number(digits)
        .unwrap_or_else(|| panic!("{} is not an 8-bit value (-128 to 255)", text));
    match (negative, magnitude) {
        (false, 0..=0xFF) => magnitude as u8,
        (true, 0..=0x80) => (magnitude as u8).wrapping_neg(),
        _ => panic!("{} is not an 8-bit value (-128 to 255)", text),
    }
}

/// The one name of the memory byte `symbol`: `_OAMRAM+4+1` is `_OAMRAM+5`
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
    /// Memory accessed through `[symbol]` operands, by normalised name (`_OAMRAM+5`);
    /// reading a symbol never written panics
    pub mem: Memory,
    /// Values of the symbols used as immediates (`and PADF_LEFT`); reading one that is
    /// not set panics
    pub consts: BTreeMap<String, u8>,
    /// Values of the symbols loaded into a register pair as numbers
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
        // Return addresses of the calls in progress
        let mut stack = Vec::new();
        while pc < instrs.len() {
            steps += 1;
            assert!(steps <= 100_000, "the code does not terminate");
            match &instrs[pc] {
                Instr::Ld { dst, src } => {
                    if let Operand::Reg(pair @ (Register::BC | Register::DE | Register::HL)) = dst {
                        self.load_pair(pair, src);
                    } else {
                        let value = self.read(src);
                        self.write(dst, value);
                    }
                }
                Instr::Add {
                    dst: Operand::Reg(Register::A),
                    src,
                } => {
                    let value = self.read(src);
                    let (result, carry) = self.get(&Register::A).overflowing_add(value);
                    self.set(&Register::A, result);
                    self.set_zero(result == 0);
                    self.set_carry(carry);
                }
                // `adc a, src` (both forms): a + src + carry
                Instr::Adc {
                    dst: Operand::Reg(Register::A),
                    src,
                }
                | Instr::AdcA { operand: src } => {
                    let value = self.read(src);
                    let carry = self.holds(&Condition::C);
                    let sum =
                        u16::from(self.get(&Register::A)) + u16::from(value) + u16::from(carry);
                    let result = sum as u8;
                    self.set(&Register::A, result);
                    self.set_zero(result == 0);
                    self.set_carry(sum > 0xFF);
                }
                // `add hl, rr`: the Z flag is unchanged
                Instr::Add {
                    dst: Operand::Reg(Register::HL),
                    src: Operand::Reg(pair @ (Register::BC | Register::DE | Register::HL)),
                } => {
                    let hl = self.address(&Register::HL);
                    let other = self.address(pair);
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
                            "add hl, {:?}: the sum of {} and {} is not supported by the test CPU",
                            pair,
                            hl.name(),
                            other.name()
                        ),
                    };
                    self.set_pair(&Register::HL, result);
                }
                Instr::Sub {
                    dst: Operand::Reg(Register::A),
                    src,
                } => {
                    let value = self.read(src);
                    self.compare(value);
                    let result = self.get(&Register::A).wrapping_sub(value);
                    self.set(&Register::A, result);
                }
                Instr::Cp { operand } => {
                    let value = self.read(operand);
                    self.compare(value);
                }
                // 16-bit `inc rr` / `dec rr`: no flag changes
                Instr::Inc {
                    operand: Operand::Reg(pair @ (Register::BC | Register::DE | Register::HL)),
                } => {
                    let moved = self.address(pair).moved(1);
                    self.set_pair(pair, moved);
                }
                Instr::Dec {
                    operand: Operand::Reg(pair @ (Register::BC | Register::DE | Register::HL)),
                } => {
                    let moved = self.address(pair).moved(-1);
                    self.set_pair(pair, moved);
                }
                Instr::Inc {
                    operand: Operand::Reg(reg),
                } => {
                    let value = self.get(reg).wrapping_add(1);
                    self.set(reg, value);
                    self.set_zero(value == 0); // carry unchanged
                }
                Instr::Dec {
                    operand: Operand::Reg(reg),
                } => {
                    let value = self.get(reg).wrapping_sub(1);
                    self.set(reg, value);
                    self.set_zero(value == 0); // carry unchanged
                }
                Instr::And { operand } => {
                    let value = self.get(&Register::A) & self.read(operand);
                    self.set(&Register::A, value);
                    self.set_zero(value == 0);
                    self.set_carry(false);
                }
                Instr::Or {
                    dst: Operand::Reg(Register::A),
                    src,
                } => {
                    let value = self.get(&Register::A) | self.read(src);
                    self.set(&Register::A, value);
                    self.set_zero(value == 0);
                    self.set_carry(false);
                }
                // `srl r`: shift right, bit 0 into the carry
                Instr::Srl {
                    operand: Operand::Reg(reg),
                } => {
                    let value = self.get(reg);
                    self.set(reg, value >> 1);
                    self.set_zero(value >> 1 == 0);
                    self.set_carry(value & 1 == 1);
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
                Instr::Call { target: t } => {
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
                    stack.push(pc + 1);
                    pc = target(pc, t);
                    continue;
                }
                Instr::Ret => match stack.pop() {
                    Some(back) => {
                        pc = back;
                        continue;
                    }
                    None => return,
                },
                Instr::RetCond { condition } => {
                    if self.holds(condition) {
                        match stack.pop() {
                            Some(back) => {
                                pc = back;
                                continue;
                            }
                            None => return,
                        }
                    }
                }
                Instr::Label { .. } | Instr::Comment { .. } => {}
                other => panic!("instruction not supported by the test CPU: {}", other),
            }
            pc += 1;
        }
    }

    /// Panics if one of the `bits` is unknown
    fn check_known(&self, bits: u16, what: &dyn std::fmt::Debug) {
        assert!(
            self.unknown & bits == 0,
            "{:?} read while its value is unknown (its register pair was loaded with an \
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

    /// Flags of `a - value`, as `cp` and `sub` set them
    fn compare(&mut self, value: u8) {
        let a = self.get(&Register::A);
        self.set_zero(a == value);
        self.set_carry(a < value);
    }

    /// The unknown bit of the 8-bit register `reg`
    fn unknown_bit(reg: &Register) -> u16 {
        match reg {
            Register::A => UNKNOWN_A,
            Register::B => UNKNOWN_B,
            Register::C => UNKNOWN_C,
            Register::D => UNKNOWN_D,
            Register::E => UNKNOWN_E,
            Register::H => UNKNOWN_H,
            Register::L => UNKNOWN_L,
            other => panic!("register {:?} not supported by the test CPU", other),
        }
    }

    /// The value of the 8-bit register `reg`; panics if it is unknown
    fn get(&mut self, reg: &Register) -> u8 {
        self.check_known(Self::unknown_bit(reg), reg);
        *self.reg(reg)
    }

    /// Set the 8-bit register `reg`; the pair it belongs to no longer holds an address
    fn set(&mut self, reg: &Register, value: u8) {
        match reg {
            Register::B | Register::C => self.bc = None,
            Register::D | Register::E => self.de = None,
            Register::H | Register::L => self.hl = None,
            _ => {}
        }
        self.unknown &= !Self::unknown_bit(reg);
        *self.reg(reg) = value;
    }

    /// The register pair `reg`, or `None` for an 8-bit register
    fn pair(&mut self, reg: &Register) -> Option<&mut Option<Pointer>> {
        match reg {
            Register::BC => Some(&mut self.bc),
            Register::DE => Some(&mut self.de),
            Register::HL => Some(&mut self.hl),
            _ => None,
        }
    }

    /// The 8-bit registers of the register pair `reg`, high then low
    fn halves(reg: &Register) -> (Register, Register) {
        match reg {
            Register::BC => (Register::B, Register::C),
            Register::DE => (Register::D, Register::E),
            Register::HL => (Register::H, Register::L),
            other => panic!("{:?} is not a register pair", other),
        }
    }

    /// The address in the register pair `reg`: its symbolic address, or the number its
    /// two 8-bit registers make; panics if it holds neither
    fn address(&mut self, reg: &Register) -> Pointer {
        let (high, low) = Self::halves(reg);
        if let Some(pointer) = self.pair(reg).and_then(|pair| pair.clone()) {
            return pointer;
        }
        let known = self.unknown & (Self::unknown_bit(&high) | Self::unknown_bit(&low)) == 0;
        assert!(known, "{:?} used without an address loaded", reg);
        Pointer::number(u16::from_be_bytes([*self.reg(&high), *self.reg(&low)]))
    }

    /// Put the address `pointer` in the register pair `reg`: a number goes into its two
    /// 8-bit registers; with a symbol, they hold an address the model does not know as a
    /// number, so they become unknown
    fn set_pair(&mut self, reg: &Register, pointer: Pointer) {
        let (high, low) = Self::halves(reg);
        if pointer.is_number() {
            let [high_byte, low_byte] = pointer.offset.to_be_bytes();
            self.set(&high, high_byte);
            self.set(&low, low_byte);
        } else {
            self.unknown |= Self::unknown_bit(&high) | Self::unknown_bit(&low);
            *self
                .pair(reg)
                .unwrap_or_else(|| panic!("{:?} is not a register pair", reg)) = Some(pointer);
        }
    }

    fn reg(&mut self, reg: &Register) -> &mut u8 {
        match reg {
            Register::A => &mut self.a,
            Register::B => &mut self.b,
            Register::C => &mut self.c,
            Register::D => &mut self.d,
            Register::E => &mut self.e,
            Register::H => &mut self.h,
            Register::L => &mut self.l,
            other => panic!("register {:?} not supported by the test CPU", other),
        }
    }

    fn read(&mut self, operand: &Operand) -> u8 {
        match operand {
            Operand::Reg(reg) => self.get(reg),
            Operand::Imm(value) => *value,
            Operand::AddrDef(symbol) => *self.mem.get(symbol).unwrap_or_else(|| {
                panic!("read of [{}], which was never written", normalize(symbol))
            }),
            Operand::AddrReg(reg) => {
                let name = self.address(reg).name();
                self.read(&Operand::AddrDef(name))
            }
            Operand::Label(symbol) => match parse_number(symbol.trim()) {
                // A number written as text (`ld a, -1`): an 8-bit operand, -128 to 255
                Some(_) => imm8(symbol),
                None => *self
                    .consts
                    .get(symbol)
                    .unwrap_or_else(|| panic!("constant {} not set in the test CPU", symbol)),
            },
            other => panic!("operand {} not supported by the test CPU", other),
        }
    }

    /// `ld rr, n16`: put a number (`$9800`, a symbol of [`TestCpu::consts16`]) or a
    /// symbolic address (`_OAMRAM+4`) in the register pair `rr`
    fn load_pair(&mut self, reg: &Register, src: &Operand) {
        let pointer = match src {
            Operand::Imm16(number) => Pointer::number(*number),
            Operand::Label(symbol) => match self.consts16.get(symbol.trim()) {
                Some(number) => Pointer::number(*number),
                None => Pointer::parse(symbol),
            },
            other => panic!("ld {:?}, {} not supported by the test CPU", reg, other),
        };
        self.set_pair(reg, pointer);
    }

    fn write(&mut self, operand: &Operand, value: u8) {
        match operand {
            Operand::Reg(reg) => self.set(reg, value),
            Operand::AddrDef(symbol) => {
                let name = normalize(symbol);
                self.trace.push(Event::Write(name.clone(), value));
                self.mem.insert(name, value);
            }
            Operand::AddrReg(reg) => {
                let name = self.address(reg).name();
                self.write(&Operand::AddrDef(name), value);
            }
            Operand::AddrRegInc(Register::HL) => {
                let hl = self.address(&Register::HL);
                self.write(&Operand::AddrDef(hl.name()), value);
                self.set_pair(&Register::HL, hl.moved(1));
            }
            other => panic!("cannot write to {} in the test CPU", other),
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
            .ld(Operand::Reg(Register::B), Operand::Reg(Register::A))
            .ret() // ends the run: no call to return to
            .ld_a(99)
            .label("Double")
            .add(Operand::Reg(Register::A), Operand::Reg(Register::A))
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
        asm.dec(Operand::Reg(Register::A))
            .ret_cond(Condition::Z)
            .inc(Operand::Reg(Register::B));
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
        asm.ld(
            Operand::Reg(Register::HL),
            Operand::Label("_OAMRAM".to_string()),
        )
        .ld_a(7)
        .ld(Operand::AddrRegInc(Register::HL), Operand::Reg(Register::A))
        .ld(Operand::AddrRegInc(Register::HL), Operand::Reg(Register::A))
        .ld_a(9)
        .ld(Operand::AddrReg(Register::HL), Operand::Reg(Register::A))
        .ld_a(0)
        .ld(Operand::Reg(Register::B), Operand::AddrReg(Register::HL))
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
        asm.ld(
            Operand::Reg(Register::HL),
            Operand::Label("_OAMRAM".to_string()),
        )
        .ld(Operand::Reg(Register::L), Operand::Imm(4))
        .ld(Operand::AddrReg(Register::HL), Operand::Reg(Register::A));
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

    fn ld_pair(asm: &mut Asm, pair: Register, symbol: &str) {
        asm.ld(Operand::Reg(pair), Operand::Label(symbol.to_string()));
    }

    #[test]
    fn test_loading_a_pair_makes_its_halves_unknown() {
        let pairs = [
            (Register::BC, Register::B, Register::C),
            (Register::DE, Register::D, Register::E),
            (Register::HL, Register::H, Register::L),
        ];
        for (pair, high, low) in pairs {
            for half in [high, low] {
                let mut asm = Asm::new();
                ld_pair(&mut asm, pair.clone(), "_OAMRAM");
                asm.ld(Operand::Reg(Register::A), Operand::Reg(half.clone()));
                assert!(panics(&asm), "{:?} read after ld {:?}", half, pair);

                // Set again, the half is known (and the pair no longer holds an address)
                let mut asm = Asm::new();
                ld_pair(&mut asm, pair.clone(), "_OAMRAM");
                asm.ld(Operand::Reg(half.clone()), Operand::Imm(3))
                    .ld(Operand::Reg(Register::A), Operand::Reg(half.clone()));
                assert!(!panics(&asm), "{:?} set after ld {:?}", half, pair);
            }
        }
    }

    #[test]
    fn test_a_stub_leaves_registers_pairs_and_flags_unknown() {
        let after_stub = |tail: &dyn Fn(&mut Asm)| {
            let mut asm = Asm::new();
            ld_pair(&mut asm, Register::HL, "_OAMRAM");
            ld_pair(&mut asm, Register::DE, "Tiles");
            ld_pair(&mut asm, Register::BC, "TilesEnd - Tiles");
            asm.ld_a(1).cp_imm(1).ld_b(2).call("Memcopy");
            tail(&mut asm);
            asm
        };
        let reads: [(&str, Snippet); 7] = [
            ("a", |asm| {
                asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));
            }),
            ("b", |asm| {
                asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::B));
            }),
            ("hl", |asm| {
                asm.ld(Operand::AddrRegInc(Register::HL), Operand::Imm(0));
            }),
            ("de", |asm| {
                asm.ld(Operand::Reg(Register::A), Operand::AddrReg(Register::DE));
            }),
            ("bc", |asm| {
                asm.ld(Operand::Reg(Register::A), Operand::AddrReg(Register::BC));
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
        ld_pair(&mut asm, Register::HL, "_OAMRAM+4");
        asm.ld_a(7)
            .ld(Operand::AddrRegInc(Register::HL), Operand::Reg(Register::A))
            .ld_a(8)
            .ld(Operand::AddrRegInc(Register::HL), Operand::Reg(Register::A))
            .ld_a(9)
            .ld_addr_def_a("_OAMRAM + 6")
            .ld_a(0)
            .ld_addr_def_a("_OAMRAM+0")
            .ld_a_addr_def("_OAMRAM+5");
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

    fn reg(r: Register) -> Operand {
        Operand::Reg(r)
    }

    #[test]
    fn test_a_pair_holds_a_number() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, Register::BC, "$9800");
        ld_pair(&mut asm, Register::DE, "LenEnd - Len");
        // hl set through its halves: $12FF
        asm.ld(reg(Register::H), Operand::Imm(0x12))
            .ld(reg(Register::L), Operand::Imm(0xFF))
            .ld_a(7)
            .ld(Operand::AddrRegInc(Register::HL), reg(Register::A))
            .ld(Operand::AddrRegInc(Register::HL), reg(Register::A))
            .ld(reg(Register::A), Operand::AddrReg(Register::BC));
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

    #[test]
    fn test_an_8_bit_operand_written_as_a_number() {
        // `ld a, -1` (a number written as text, as `Var::set` emits it) is the byte $FF
        for (text, value) in [
            ("-1", 0xFF),
            ("-128", 0x80),
            ("$10", 0x10),
            ("255", 0xFF),
            ("-0", 0),
            ("-$7F", 0x81),
        ] {
            let mut cpu = TestCpu::default();
            cpu.run(&[Instr::Ld {
                dst: Operand::Reg(Register::A),
                src: Operand::Label(text.to_string()),
            }]);
            assert_eq!(cpu.a, value, "{}", text);
        }
        // Out of the 8-bit range: rgbasm would truncate it, the model panics. The range
        // is checked on the signed value: -65535 and -$FF00 wrap to 1 and $100 in 16 bits
        // (-65535 was taken for 1)
        for text in ["256", "-129", "$FFFF", "-65535", "-$FF00", "--1"] {
            let result = std::panic::catch_unwind(|| {
                TestCpu::default().run(&[Instr::Ld {
                    dst: Operand::Reg(Register::A),
                    src: Operand::Label(text.to_string()),
                }])
            });
            assert!(result.is_err(), "{} should panic", text);
        }
    }

    #[test]
    fn test_add_hl() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, Register::HL, "$8000");
        ld_pair(&mut asm, Register::BC, "$0005");
        asm.ld_a(1)
            .cp_imm(1) // Z set, C clear
            .add(reg(Register::HL), reg(Register::HL)) // $0000, carry
            .ld(reg(Register::D), reg(Register::H))
            .ld(reg(Register::E), reg(Register::L))
            .ld_a(0)
            .jp_cond(Condition::NC, "End")
            .jp_cond(Condition::NZ, "End")
            .ld_a(1) // carry set and Z unchanged
            .add(reg(Register::HL), reg(Register::BC)) // $0005, no carry
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
            ld_pair(&mut asm, Register::HL, "33");
            ld_pair(&mut asm, Register::BC, "_SCRN0");
            asm.add(reg(Register::HL), reg(Register::BC));
            tail(&mut asm);
            asm
        };
        let mut cpu = TestCpu::default();
        cpu.mem.insert("_SCRN0+33".to_string(), 4);
        cpu.run(
            &symbol_plus_number(&|asm| {
                asm.ld_a_addr_reg(Register::HL);
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
        ld_pair(&mut asm, Register::HL, "_SCRN0+5");
        ld_pair(&mut asm, Register::DE, "$FFFF");
        asm.add(reg(Register::HL), reg(Register::DE))
            .ld_a_addr_reg(Register::HL);
        let mut cpu = TestCpu::default();
        cpu.mem.insert("_SCRN0+4".to_string(), 8);
        cpu.run(&asm.get_main_instrs());
        assert_eq!(cpu.a, 8);

        // Two symbols cannot be added
        let mut asm = Asm::new();
        ld_pair(&mut asm, Register::HL, "_SCRN0");
        asm.add(reg(Register::HL), reg(Register::HL));
        assert!(panics(&asm));
    }

    #[test]
    fn test_16_bit_inc_and_dec() {
        let mut asm = Asm::new();
        ld_pair(&mut asm, Register::DE, "Tiles");
        ld_pair(&mut asm, Register::BC, "$0000");
        asm.ld_a(1)
            .cp_imm(1) // Z set: inc rr / dec rr leave the flags alone
            .inc(reg(Register::DE))
            .inc(reg(Register::DE))
            .dec(reg(Register::DE))
            .dec(reg(Register::BC))
            .ld(reg(Register::A), Operand::AddrReg(Register::DE))
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
        ld_pair(&mut asm, Register::DE, "Tiles");
        asm.dec(reg(Register::DE));
        assert!(panics(&asm));
    }

    #[test]
    fn test_srl_adc_and_or() {
        let mut asm = Asm::new();
        asm.ld_a(0b11)
            .srl(reg(Register::A)) // 1, carry
            .ld(reg(Register::B), reg(Register::A))
            .srl(reg(Register::A)) // 0, carry, zero
            .ld_a(0xFF)
            .adc(reg(Register::A), Operand::Imm(0)) // 0xFF + 0 + 1 = 0, carry
            .ld(reg(Register::C), reg(Register::A))
            .adc_a(Operand::Imm(1)) // 0 + 1 + 1 = 2, no carry
            .ld(reg(Register::D), reg(Register::A))
            .adc(reg(Register::A), Operand::Imm(3)) // 2 + 3 + 0
            .ld(reg(Register::E), reg(Register::A))
            .ld_a(0)
            .or(reg(Register::A), Operand::Imm(0)) // zero, carry cleared
            .jp_cond(Condition::NZ, "End")
            .jp_cond(Condition::C, "End")
            .or(reg(Register::A), reg(Register::B)) // 1
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
}
