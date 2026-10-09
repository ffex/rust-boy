// Module declarations
pub mod asm;
pub mod block;
mod builders;
mod codegen;
pub mod expr;
pub mod instr;
#[cfg(test)]
mod isa_tests;
#[cfg(test)]
pub(crate) mod label_check;
pub mod labels;
#[cfg(test)]
pub(crate) mod test_cpu;

// Re-export main types for convenience
pub use asm::{Asm, Chunk};
pub use block::{Block, Emittable, boxed};
pub use expr::Expr;
pub use instr::{
    AluOperand, Condition, Dst, IncDec, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};
pub use labels::{LabelAllocator, is_identifier};
