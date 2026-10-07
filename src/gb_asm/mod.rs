// Module declarations
pub mod asm;
mod codegen;
pub mod instr;
#[cfg(test)]
pub(crate) mod test_cpu;

// Re-export main types for convenience
pub use asm::{Asm, Chunk};
pub use instr::{Condition, Instr, JumpTarget, Operand, Register};
