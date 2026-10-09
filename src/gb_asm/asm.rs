// The builder methods `instruction_builders!` expands into `impl Asm` name these types
use super::builders::instruction_builders;
use super::expr::Expr;
use super::instr::{
    AluOperand, Condition, Dst, IncDec, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};
use std::collections::HashMap;
use std::fmt::Display;

pub struct Asm {
    pub(crate) chunks: HashMap<Chunk, Vec<Instr>>,
    current_chunk: Chunk,
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
    pub fn new() -> Self {
        Asm {
            chunks: HashMap::new(),
            current_chunk: Chunk::Main,
        }
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
