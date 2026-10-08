//! A tiny model of the Game Boy CPU for unit tests.
//!
//! It runs the [`Instr`]s a routine emits, so a test can check what the generated code
//! does rather than how it looks. It models the 8-bit registers, the Z and C flags, a
//! memory addressed by symbol (`[wCurKeys]`, `[_OAMRAM+1]`, …) and symbolic constants
//! (`PADF_LEFT`). The register pairs `bc`, `de` and `hl` hold a symbolic address
//! ([`Pointer`]): `ld hl, _OAMRAM` then `ld [hli], a` writes `[_OAMRAM]`, then
//! `[_OAMRAM+1]`, the names a direct access such as `ld [_OAMRAM+1], a` uses. Jumps and
//! calls go to labels in the same instruction list, and
//! execution ends when it runs past the last instruction or on a `ret` with no `call`
//! to return to (so a routine can be run on its own, or a test can put its routines
//! after a `ret`). A call to one of the [`TestCpu::stubs`] returns at once (for a
//! routine the model cannot run, such as `Memcopy`). Every write to memory and every
//! call is recorded in [`TestCpu::trace`], so a test can check the order of side
//! effects. A local label (`.name`) belongs to the last global label before
//! it, as in RGBDS, so two routines can each have their own `.loop`; `label_check`
//! checks the scopes of a whole program. Anything it does not model panics, so a test
//! never passes by skipping code.

use std::collections::{BTreeMap, BTreeSet};

use super::{Condition, Instr, JumpTarget, Operand, Register};

/// The address held by a register pair: a symbol plus an offset in bytes
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pointer {
    pub symbol: String,
    pub offset: u16,
}

impl Pointer {
    /// The memory symbol of the byte it points to: `_OAMRAM`, `_OAMRAM+1`, …
    fn name(&self) -> String {
        if self.offset == 0 {
            self.symbol.clone()
        } else {
            format!("{}+{}", self.symbol, self.offset)
        }
    }
}

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
    /// Memory accessed through `[symbol]` operands; reading a symbol never written panics
    pub mem: BTreeMap<String, u8>,
    /// Values of the symbols used as immediates (`and PADF_LEFT`); reading one that is
    /// not set panics
    pub consts: BTreeMap<String, u8>,
    /// The register pairs; `None` until loaded with an address, and again after one of
    /// their 8-bit halves is changed (using such a pair panics)
    pub bc: Option<Pointer>,
    pub de: Option<Pointer>,
    pub hl: Option<Pointer>,
    /// Routines that are not run: a call to one is recorded and returns at once
    pub stubs: BTreeSet<String>,
    /// Every memory write and every call, in order
    pub trace: Vec<Event>,
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
                Instr::Ld {
                    dst: Operand::Reg(reg),
                    src,
                } if self.load_pair(reg, src) => {}
                Instr::Ld { dst, src } => {
                    let value = self.read(src);
                    self.write(dst, value);
                }
                Instr::Add {
                    dst: Operand::Reg(Register::A),
                    src,
                } => {
                    let (result, carry) = self.a.overflowing_add(self.read(src));
                    self.a = result;
                    self.zero = result == 0;
                    self.carry = carry;
                }
                Instr::Sub {
                    dst: Operand::Reg(Register::A),
                    src,
                } => {
                    let value = self.read(src);
                    self.compare(value);
                    self.a = self.a.wrapping_sub(value);
                }
                Instr::Cp { operand } => {
                    let value = self.read(operand);
                    self.compare(value);
                }
                Instr::Inc {
                    operand: Operand::Reg(reg),
                } => {
                    self.split_pair(reg);
                    let value = self.reg(reg).wrapping_add(1);
                    *self.reg(reg) = value;
                    self.zero = value == 0; // carry unchanged
                }
                Instr::Dec {
                    operand: Operand::Reg(reg),
                } => {
                    self.split_pair(reg);
                    let value = self.reg(reg).wrapping_sub(1);
                    *self.reg(reg) = value;
                    self.zero = value == 0; // carry unchanged
                }
                Instr::And { operand } => {
                    self.a &= self.read(operand);
                    self.zero = self.a == 0;
                    self.carry = false;
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

    /// Whether `condition` holds with the current flags
    fn holds(&self, condition: &Condition) -> bool {
        match condition {
            Condition::Z => self.zero,
            Condition::NZ => !self.zero,
            Condition::C => self.carry,
            Condition::NC => !self.carry,
        }
    }

    /// Flags of `a - value`, as `cp` and `sub` set them
    fn compare(&mut self, value: u8) {
        self.zero = self.a == value;
        self.carry = self.a < value;
    }

    /// Forget the address in the pair `reg` belongs to: its 8-bit half is about to change
    fn split_pair(&mut self, reg: &Register) {
        match reg {
            Register::B | Register::C => self.bc = None,
            Register::D | Register::E => self.de = None,
            Register::H | Register::L => self.hl = None,
            _ => {}
        }
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

    /// The address in the register pair `reg`; panics if it holds none
    fn pointer(&mut self, reg: &Register) -> &mut Pointer {
        self.pair(reg)
            .unwrap_or_else(|| panic!("{:?} is not a register pair", reg))
            .as_mut()
            .unwrap_or_else(|| panic!("{:?} used without an address loaded", reg))
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
            Operand::Reg(reg) => *self.reg(reg),
            Operand::Imm(value) => *value,
            Operand::AddrDef(symbol) => *self
                .mem
                .get(symbol)
                .unwrap_or_else(|| panic!("read of [{}], which was never written", symbol)),
            Operand::AddrReg(reg) => {
                let name = self.pointer(reg).name();
                self.read(&Operand::AddrDef(name))
            }
            Operand::Label(symbol) => *self
                .consts
                .get(symbol)
                .unwrap_or_else(|| panic!("constant {} not set in the test CPU", symbol)),
            other => panic!("operand {} not supported by the test CPU", other),
        }
    }

    /// `ld rr, symbol`: point the register pair `rr` at `symbol`
    fn load_pair(&mut self, reg: &Register, src: &Operand) -> bool {
        let Some(pair) = self.pair(reg) else {
            return false;
        };
        match src {
            Operand::Label(symbol) => {
                *pair = Some(Pointer {
                    symbol: symbol.clone(),
                    offset: 0,
                })
            }
            other => panic!("ld {:?}, {} not supported by the test CPU", reg, other),
        }
        true
    }

    fn write(&mut self, operand: &Operand, value: u8) {
        match operand {
            Operand::Reg(reg) => {
                self.split_pair(reg);
                *self.reg(reg) = value
            }
            Operand::AddrDef(symbol) => {
                self.trace.push(Event::Write(symbol.clone(), value));
                self.mem.insert(symbol.clone(), value);
            }
            Operand::AddrReg(reg) => {
                let name = self.pointer(reg).name();
                self.write(&Operand::AddrDef(name), value);
            }
            Operand::AddrRegInc(Register::HL) => {
                let name = self.pointer(&Register::HL).name();
                self.write(&Operand::AddrDef(name), value);
                self.pointer(&Register::HL).offset += 1;
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
        .call("Memcopy")
        .ld_a(0)
        .ld(Operand::Reg(Register::A), Operand::AddrReg(Register::HL));
        let mut cpu = TestCpu::default();
        cpu.stubs.insert("Memcopy".to_string());
        cpu.run(&asm.get_main_instrs());
        assert_eq!(cpu.a, 9, "read back through hl");
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
}
