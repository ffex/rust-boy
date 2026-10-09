// Module declarations
pub mod asm;
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
pub use expr::Expr;
pub use instr::{Condition, Instr, JumpTarget, Operand, R8, R16, R16Stack, Register};
pub use labels::{LabelAllocator, is_identifier};
