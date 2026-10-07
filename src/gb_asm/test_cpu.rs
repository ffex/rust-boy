//! A tiny model of the Game Boy CPU for unit tests.
//!
//! It runs the [`Instr`]s a routine emits, so a test can check what the generated code
//! does rather than how it looks. It models the 8-bit registers, the Z and C flags and a
//! memory addressed by symbol (`[wCurKeys]`, `[_OAMRAM+1]`, …). Jumps go to labels in the
//! same instruction list, and execution ends when it runs past the last instruction.
//! Anything it does not model panics, so a test never passes by skipping code.

use std::collections::BTreeMap;

use super::{Condition, Instr, JumpTarget, Operand, Register};

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
}

impl TestCpu {
    /// Run `instrs` from the first instruction until execution runs past the last one
    pub fn run(&mut self, instrs: &[Instr]) {
        let mut labels = BTreeMap::new();
        for (pos, instr) in instrs.iter().enumerate() {
            if let Instr::Label { name } = instr {
                assert!(
                    labels.insert(name.as_str(), pos).is_none(),
                    "label {} defined twice",
                    name
                );
            }
        }
        let target = |target: &JumpTarget| match target {
            JumpTarget::Label(name) => *labels
                .get(name.as_str())
                .unwrap_or_else(|| panic!("label {} not found", name)),
            JumpTarget::Addr(addr) => panic!("jump to address ${:04X} not supported", addr),
        };

        let mut pc = 0;
        let mut steps = 0;
        while pc < instrs.len() {
            steps += 1;
            assert!(steps <= 100_000, "the code does not terminate");
            match &instrs[pc] {
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
                Instr::Jp { target: t } | Instr::Jr { target: t } => {
                    pc = target(t);
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
                    let taken = match condition {
                        Condition::Z => self.zero,
                        Condition::NZ => !self.zero,
                        Condition::C => self.carry,
                        Condition::NC => !self.carry,
                    };
                    if taken {
                        pc = target(t);
                        continue;
                    }
                }
                Instr::Label { .. } | Instr::Comment { .. } => {}
                other => panic!("instruction not supported by the test CPU: {}", other),
            }
            pc += 1;
        }
    }

    /// Flags of `a - value`, as `cp` and `sub` set them
    fn compare(&mut self, value: u8) {
        self.zero = self.a == value;
        self.carry = self.a < value;
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
            other => panic!("operand {} not supported by the test CPU", other),
        }
    }

    fn write(&mut self, operand: &Operand, value: u8) {
        match operand {
            Operand::Reg(reg) => *self.reg(reg) = value,
            Operand::AddrDef(symbol) => {
                self.mem.insert(symbol.clone(), value);
            }
            other => panic!("cannot write to {} in the test CPU", other),
        }
    }
}
