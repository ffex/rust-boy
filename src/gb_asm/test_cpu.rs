//! A tiny model of the Game Boy CPU for unit tests.
//!
//! It runs the [`Instr`]s a routine emits, so a test can check what the generated code
//! does rather than how it looks. It models the 8-bit registers, the Z and C flags, a
//! memory addressed by symbol (`[wCurKeys]`, `[_OAMRAM+1]`, …) and symbolic constants
//! (`PADF_LEFT`). Jumps and calls go to labels in the same instruction list, and
//! execution ends when it runs past the last instruction or on a `ret` with no `call`
//! to return to (so a routine can be run on its own, or a test can put its routines
//! after a `ret`). A local label (`.name`) belongs to the last global label before
//! it, as in RGBDS, so two routines can each have their own `.loop`; `label_check`
//! checks the scopes of a whole program. Anything it does not model panics, so a test
//! never passes by skipping code.

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
    /// Values of the symbols used as immediates (`and PADF_LEFT`); reading one that is
    /// not set panics
    pub consts: BTreeMap<String, u8>,
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
                    let value = self.reg(reg).wrapping_add(1);
                    *self.reg(reg) = value;
                    self.zero = value == 0; // carry unchanged
                }
                Instr::Dec {
                    operand: Operand::Reg(reg),
                } => {
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
            Operand::Label(symbol) => *self
                .consts
                .get(symbol)
                .unwrap_or_else(|| panic!("constant {} not set in the test CPU", symbol)),
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
