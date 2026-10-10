//! [`Block`], a buffer of instructions, and [`Emittable`], anything that emits them.
//!
//! A routine or snippet is built in a `Block`, with the same methods as [`Asm`](super::Asm)
//! (`ld`, `call`, `label`, ...), and handed on as a `Vec<Instr>` ([`Block::into_instrs`])
//! or as an [`Emittable`]. An `Asm` is a whole program, checked against its sections and
//! printed with its jumps relaxed; a `Block` is a piece of one (code used to be built in a
//! scratch `Asm` and read back with `get_main_instrs()`).

use std::ops::Deref;

// The builder methods `instruction_builders!` expands into `impl Block` name these types
use super::builders::instruction_builders;
use super::expr::Expr;
use super::instr::{
    AluOperand, Condition, Dst, IncDec, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};
use super::labels::LabelAllocator;
use super::section::Section;
use std::fmt::Display;

/// A sequence of instructions, built with the same methods as [`Asm`](super::Asm)
///
/// Every instruction is checked when it is added ([`Instr::check`]), like in an `Asm`. A
/// `Block` reads as a slice of instructions (`&block[..]`, `block.len()`, `block.iter()`),
/// and is [`Emittable`].
///
/// # Example
/// ```
/// use rust_boy::gb_asm::{Block, Instr, Mem, R8};
///
/// fn clear_byte(address: &str) -> Vec<Instr> {
///     let mut code = Block::new();
///     code.ld_a(0).ld(Mem::addr(address), R8::A);
///     code.into_instrs()
/// }
///
/// let text: Vec<String> = clear_byte("wScore").iter().map(|i| i.to_string()).collect();
/// assert_eq!(text, ["ld a, 0", "ld [wScore], a"]);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    instrs: Vec<Instr>,
}

impl Block {
    /// An empty block
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one instruction at the end
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects one of its operands.
    #[track_caller]
    pub fn emit(&mut self, instr: Instr) -> &mut Self {
        if let Err(error) = instr.check() {
            panic!("invalid instruction: {}", error);
        }
        self.instrs.push(instr);
        self
    }

    /// Add instructions at the end: a `Vec<Instr>`, another `Block`, ...
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects an operand of one of them.
    #[track_caller]
    pub fn emit_all(&mut self, instrs: impl IntoIterator<Item = Instr>) -> &mut Self {
        for instr in instrs {
            self.emit(instr);
        }
        self
    }

    /// The instructions, in order
    pub fn into_instrs(self) -> Vec<Instr> {
        self.instrs
    }

    instruction_builders!();
}

impl Deref for Block {
    type Target = [Instr];

    fn deref(&self) -> &[Instr] {
        &self.instrs
    }
}

impl From<Block> for Vec<Instr> {
    fn from(block: Block) -> Vec<Instr> {
        block.instrs
    }
}

/// A block of these instructions
///
/// # Panics
/// If [`Instr::check`] rejects one of them.
impl From<Vec<Instr>> for Block {
    #[track_caller]
    fn from(instrs: Vec<Instr>) -> Block {
        let mut block = Block::new();
        block.emit_all(instrs);
        block
    }
}

impl IntoIterator for Block {
    type Item = Instr;
    type IntoIter = std::vec::IntoIter<Instr>;

    fn into_iter(self) -> Self::IntoIter {
        self.instrs.into_iter()
    }
}

/// Anything that emits instructions: plain code ([`Block`], `Vec<Instr>`) and the
/// control-flow structures of `gb_std` (`If`, `IfConst`, `Call`, ...)
///
/// `labels` is the program's label allocator ([`Asm::labels`](super::Asm::labels)): every
/// label the code makes up comes from it (an `If` takes one number for its `.end_if_N`,
/// `.else_N` and `.then_N`), so the labels are unique in the whole program. Give every
/// piece of code of a program the same allocator; [`Asm::emit_code`](super::Asm::emit_code)
/// does it.
pub trait Emittable {
    /// The instructions, with their labels taken from `labels`
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr>;
}

/// The instructions of the block
impl Emittable for Block {
    fn emit(&mut self, _labels: &LabelAllocator) -> Vec<Instr> {
        std::mem::take(&mut self.instrs)
    }
}

/// The instructions, as they are
impl Emittable for Vec<Instr> {
    fn emit(&mut self, _labels: &LabelAllocator) -> Vec<Instr> {
        std::mem::take(self)
    }
}

/// Several sequences, one after the other:
/// ```ignore
/// vec![
///     TileRef::set_tile_label("BLANK_TILE"),
///     TileRef::next_tile(),
///     TileRef::set_tile_label("BLANK_TILE"),
/// ]
/// ```
impl Emittable for Vec<Vec<Instr>> {
    fn emit(&mut self, _labels: &LabelAllocator) -> Vec<Instr> {
        std::mem::take(self).into_iter().flatten().collect()
    }
}

/// Different kinds of emittables, one after the other (see [`boxed`]):
/// ```ignore
/// vec![
///     boxed(IfConst::eq(...)),
///     boxed(IfA::eq(...)),
/// ]
/// ```
impl Emittable for Vec<Box<dyn Emittable>> {
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        self.iter_mut().flat_map(|e| e.emit(labels)).collect()
    }
}

/// Box an [`Emittable`], to put different kinds in one `Vec`
///
/// # Example
/// ```ignore
/// gb.define_function_from("MyFunc", vec![
///     boxed(IfConst::eq(value, "CONST", body1)),
///     boxed(IfA::eq("OTHER", body2)),
/// ]);
/// ```
pub fn boxed(e: impl Emittable + 'static) -> Box<dyn Emittable> {
    Box::new(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::Asm;

    #[test]
    fn test_a_block_builds_like_an_asm() {
        // The same builder calls give the same instructions, in a Block and in the
        // an Asm: the methods are one definition
        let mut block = Block::new();
        let mut asm = Asm::new();
        block
            .label("Start")
            .ld(R8::A, Mem::addr("wScore"))
            .cp("BRICK_LEFT")
            .jr_cond(Condition::NZ, "Start")
            .ret();
        asm.label("Start")
            .ld(R8::A, Mem::addr("wScore"))
            .cp("BRICK_LEFT")
            .jr_cond(Condition::NZ, "Start")
            .ret();
        assert_eq!(&block[..], asm.instrs());
        assert_eq!(block.len(), 5);
    }

    #[test]
    #[should_panic(expected = "invalid instruction: ld [hl], [hl]")]
    fn test_a_block_checks_its_instructions() {
        Block::new().ld(R8::AtHl, R8::AtHl);
    }

    #[test]
    #[should_panic(expected = "invalid instruction: res 8")]
    fn test_a_block_checks_the_instructions_it_is_given() {
        let _ = Block::from(vec![Instr::Res {
            bit: 8,
            operand: R8::B,
        }]);
    }

    #[test]
    fn test_blocks_are_emittable() {
        let mut first = Block::new();
        first.ld_a(1);
        let mut second = Block::new();
        second.ld_b(2);
        let mut all = Block::new();
        all.emit_all(first.clone()).emit_all(second.into_instrs());
        assert_eq!(all.len(), 2);

        let labels = LabelAllocator::new();
        let instrs = boxed(first).emit(&labels);
        assert_eq!(
            instrs,
            [Instr::Ld {
                dst: Dst::R8(R8::A),
                src: Operand::from(1u8),
            }]
        );
        assert_eq!(labels.next_id(), 0, "plain code takes no label number");
    }
}
