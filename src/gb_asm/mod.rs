// Module declarations
pub mod asm;
mod codegen;
pub mod instr;

// Re-export main types for convenience
pub use asm::{Asm, Chunk};
pub use instr::{Condition, Instr, JumpTarget, Operand, Register};
