// The builder methods `instruction_builders!` expands into `impl Asm` name these types
use super::block::Emittable;
use super::builders::instruction_builders;
use super::expr::Expr;
use super::instr::{
    AluOperand, Condition, Dst, IncDec, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};
use super::labels::LabelAllocator;
use std::collections::HashMap;
use std::fmt::Display;

/// A whole program, in [`Chunk`]s, printed by [`Asm::to_asm`]
///
/// It owns the program's [`LabelAllocator`] ([`Asm::labels`]): every label generated code
/// makes up comes from it, so they are unique in the program. [`Asm::to_asm`] turns each
/// `jr` that does not reach its target into a `jp` (see [`Asm::program`]).
pub struct Asm {
    pub(crate) chunks: HashMap<Chunk, Vec<Instr>>,
    current_chunk: Chunk,
    labels: LabelAllocator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chunk {
    /// INCLUDE statements and header section
    Header,
    /// DEF constant definitions
    Constants,
    /// Initialization code (before main loop)
    Init,
    /// Main game loop
    MainLoop,
    /// Legacy: combines Init + MainLoop (for backwards compatibility)
    Main,
    /// Function definitions
    Functions,
    /// Tile data
    Tiles,
    /// Tilemap data
    Tilemap,
    /// Variable/data sections (WRAM)
    Data,
}

impl Default for Asm {
    fn default() -> Self {
        Self::new()
    }
}

impl Asm {
    /// An empty program, with a new label allocator
    pub fn new() -> Self {
        Self::with_labels(LabelAllocator::new())
    }

    /// An empty program whose generated labels come from `labels` (a clone shares its
    /// sequence; [`LabelAllocator::fork`] goes on with it without sharing it)
    pub fn with_labels(labels: LabelAllocator) -> Self {
        Asm {
            chunks: HashMap::new(),
            current_chunk: Chunk::Main,
            labels,
        }
    }

    /// The program's label allocator: give it to the code that makes up labels (the
    /// `gb_std` snippets such as `check_key`, any [`Emittable`])
    ///
    /// # Example
    /// ```
    /// use rust_boy::gb_asm::Asm;
    ///
    /// let asm = Asm::new();
    /// assert_eq!(asm.labels().local("loop"), ".loop_0");
    /// assert_eq!(asm.labels().local("loop"), ".loop_1");
    /// ```
    pub fn labels(&self) -> &LabelAllocator {
        &self.labels
    }

    /// Emit `code` (an `If`, a `Block`, ...) to the current chunk, its labels taken from
    /// the program's allocator ([`Asm::labels`])
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects an operand of one of its instructions.
    ///
    /// # Example
    /// ```
    /// use rust_boy::gb_asm::{Asm, Block};
    /// use rust_boy::gb_std::flow::IfA;
    ///
    /// let mut then_branch = Block::new();
    /// then_branch.ld_b(1);
    /// let mut asm = Asm::new();
    /// asm.label("Main").emit_code(IfA::eq(5, then_branch));
    /// assert!(asm.to_asm().contains(".end_if_0:"));
    /// ```
    #[track_caller]
    pub fn emit_code(&mut self, mut code: impl Emittable) -> &mut Self {
        let instrs = code.emit(&self.labels);
        self.emit_all(instrs)
    }

    /// Set the current chunk for subsequent emit calls
    pub fn chunk(&mut self, chunk: Chunk) -> &mut Self {
        self.current_chunk = chunk;
        self
    }

    /// Emit a single instruction to the current chunk
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects one of its operands.
    #[track_caller]
    pub fn emit(&mut self, instr: Instr) -> &mut Self {
        if let Err(error) = instr.check() {
            panic!("invalid instruction: {}", error);
        }
        self.chunks
            .entry(self.current_chunk)
            .or_default()
            .push(instr);
        self
    }

    /// Emit multiple instructions to the current chunk: a `Vec<Instr>`, a
    /// [`Block`](super::Block), ...
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects an operand of one of them.
    #[track_caller]
    pub fn emit_all(&mut self, instrs: impl IntoIterator<Item = Instr>) -> &mut Self {
        let instrs: Vec<Instr> = instrs.into_iter().collect();
        if let Some(error) = instrs.iter().find_map(|instr| instr.check().err()) {
            panic!("invalid instruction: {}", error);
        }
        self.chunks
            .entry(self.current_chunk)
            .or_default()
            .extend(instrs);
        self
    }

    /// Get all instructions for a specific chunk
    pub fn get_chunk(&self, chunk: Chunk) -> Option<&Vec<Instr>> {
        self.chunks.get(&chunk)
    }

    instruction_builders!();
}
