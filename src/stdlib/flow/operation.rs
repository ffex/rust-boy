use crate::asm::{Block, Instr, LabelAllocator};

use super::emittable::Emittable;

/// A wrapper for instruction sequences with arithmetic applied.
pub struct Op(pub Vec<Instr>);

impl Emittable for Op {
    fn emit(&mut self, _labels: &LabelAllocator) -> Vec<Instr> {
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
/// let mut a = Block::new();
/// a.emit_all(paddle.get_x());
/// a.sub(8);
/// a.into_instrs()
/// ```
pub trait InstrOps {
    fn plus(self, value: u8) -> Op;
    fn minus(self, value: u8) -> Op;
}

impl InstrOps for Vec<Instr> {
    fn plus(self, value: u8) -> Op {
        let mut asm = Block::new();
        asm.emit_all(self);
        asm.add(value);
        Op(asm.into_instrs())
    }

    fn minus(self, value: u8) -> Op {
        let mut asm = Block::new();
        asm.emit_all(self);
        asm.sub(value);
        Op(asm.into_instrs())
    }
}
