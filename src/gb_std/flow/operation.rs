use crate::gb_asm::{Asm, Instr, Operand, Register};

use super::emittable::Emittable;

/// A wrapper for instruction sequences with arithmetic applied.
pub struct Op(pub Vec<Instr>);

impl Emittable for Op {
    fn emit(&mut self, _counter: &mut usize) -> Vec<Instr> {
        std::mem::take(&mut self.0)
    }
}

/// Extension trait for arithmetic operations on instruction sequences.
///
/// This allows writing readable expressions like:
/// ```ignore
/// paddle.get_x().minus(8)
/// ball.get_y().plus(5)
/// ```
///
/// Instead of:
/// ```ignore
/// let mut a = Asm::new();
/// a.emit_all(paddle.get_x());
/// a.sub(Operand::Reg(Register::A), Operand::Imm(8));
/// a.get_main_instrs()
/// ```
pub trait InstrOps {
    fn plus(self, value: u8) -> Op;
    fn minus(self, value: u8) -> Op;
}

impl InstrOps for Vec<Instr> {
    fn plus(self, value: u8) -> Op {
        let mut asm = Asm::new();
        asm.emit_all(self);
        asm.add(Operand::Reg(Register::A), Operand::Imm(value));
        Op(asm.get_main_instrs())
    }

    fn minus(self, value: u8) -> Op {
        let mut asm = Asm::new();
        asm.emit_all(self);
        asm.sub(Operand::Reg(Register::A), Operand::Imm(value));
        Op(asm.get_main_instrs())
    }
}
