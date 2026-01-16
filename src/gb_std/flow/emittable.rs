use crate::gb_asm::Instr;

/// Trait for anything that can emit assembly instructions.
///
/// This provides a unified interface for both raw instructions and
/// control flow structures like If statements.
pub trait Emittable {
    /// Emit assembly instructions, using the counter for generating unique labels.
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr>;
}

/// Implementation for raw instruction vectors - just returns the instructions.
impl Emittable for Vec<Instr> {
    fn emit(&mut self, _counter: &mut usize) -> Vec<Instr> {
        std::mem::take(self)
    }
}
