//! How a `RustBoy` program is laid out: its code in [`Chunk`]s, put together in a fixed
//! order by [`Layout::program`].
//!
//! The asm layer knows instructions, sections and programs ([`Asm`]); the order of a
//! game's parts (the cartridge header, the constants, the start-up code, the main loop,
//! the functions, the tile data, the variables) is the engine's. `RustBoy::build` writes
//! each part to its chunk, in whatever order it works them out, and the program is the
//! chunks one after the other, a blank line after each. The code written with
//! [`RustBoy::raw`](super::RustBoy::raw) is a `Layout` too: `build()` puts each of its
//! chunks after the code it generates for the same chunk.

use std::collections::BTreeMap;

// The builder methods `instruction_builders!` expands into `impl Layout` name these types
use crate::gb_asm::builders::instruction_builders;
use crate::gb_asm::{
    AluOperand, Asm, Condition, Dst, Emittable, Expr, IncDec, Instr, JumpTarget, LabelAllocator,
    Mem, Operand, R8, R16, R16Stack, Section,
};
use std::fmt::Display;

/// A part of a `RustBoy` program; the program is its chunks in the order of
/// [`Chunk::ORDER`] (the order they are declared in)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Chunk {
    /// `INCLUDE "hardware.inc"` and the cartridge header section (`ROM0[$100]`)
    Header,
    /// `DEF` constant definitions
    Constants,
    /// The start-up code, from `EntryPoint` to turning the LCD on
    Init,
    /// The main loop, from `Main` to `jp Main`
    MainLoop,
    /// The default chunk of [`RustBoy::raw`](super::RustBoy::raw) code: after the main
    /// loop, reached only through a label
    Main,
    /// The functions the program uses
    Functions,
    /// Tile data
    Tiles,
    /// Tilemap data
    Tilemap,
    /// The variables, in `WRAM0` sections
    Data,
}

impl Chunk {
    /// The order of the chunks in the program
    pub const ORDER: [Chunk; 9] = [
        Chunk::Header,
        Chunk::Constants,
        Chunk::Init,
        Chunk::MainLoop,
        Chunk::Main,
        Chunk::Functions,
        Chunk::Tiles,
        Chunk::Tilemap,
        Chunk::Data,
    ];
}

/// Code in [`Chunk`]s, written with the same methods as an [`Asm`] (`ld`, `call`, `label`,
/// `section`, ...) to the current chunk ([`Layout::chunk`]; [`Chunk::Main`] at first)
///
/// Each instruction is checked when it is written ([`Instr::check`]); what a section
/// accepts is checked by [`Layout::program`], once the chunks are in their order.
///
/// # Example
/// ```
/// use rust_boy::gb_asm::Section;
/// use rust_boy::rust_boy::{Chunk, Layout};
///
/// let mut layout = Layout::new();
/// layout.chunk(Chunk::Functions).label("Routine").ret();
/// layout.chunk(Chunk::Header).section(Section::rom0("Code"));
/// layout.chunk(Chunk::MainLoop).label("Main").call("Routine").jp("Main");
/// let text = layout.program().to_asm();
/// assert!(text.starts_with("    SECTION \"Code\", ROM0\n\n    Main:\n"));
/// assert!(text.ends_with("jp Main\n\n    Routine:\n    ret\n\n"));
/// ```
pub struct Layout {
    chunks: BTreeMap<Chunk, Vec<Instr>>,
    current: Chunk,
    labels: LabelAllocator,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout {
    /// No code yet, with a new label allocator
    pub fn new() -> Self {
        Self::with_labels(LabelAllocator::new())
    }

    /// No code yet; the generated labels come from `labels`
    pub fn with_labels(labels: LabelAllocator) -> Self {
        Layout {
            chunks: BTreeMap::new(),
            current: Chunk::Main,
            labels,
        }
    }

    /// The program's label allocator (see [`Asm::labels`])
    pub fn labels(&self) -> &LabelAllocator {
        &self.labels
    }

    /// Write to `chunk` from now on
    pub fn chunk(&mut self, chunk: Chunk) -> &mut Self {
        self.current = chunk;
        self
    }

    /// Write one instruction at the end of the current chunk
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects one of its operands.
    #[track_caller]
    pub fn emit(&mut self, instr: Instr) -> &mut Self {
        if let Err(error) = instr.check() {
            panic!("invalid instruction: {}", error);
        }
        self.chunks.entry(self.current).or_default().push(instr);
        self
    }

    /// Write instructions at the end of the current chunk: a `Vec<Instr>`, a `Block`, ...
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

    /// Write `code` (an `If`, a `Block`, ...) to the current chunk, its labels taken from
    /// the program's allocator ([`Layout::labels`])
    #[track_caller]
    pub fn emit_code(&mut self, mut code: impl Emittable) -> &mut Self {
        let instrs = code.emit(&self.labels);
        self.emit_all(instrs)
    }

    /// What was written to `chunk`, if anything
    pub fn get_chunk(&self, chunk: Chunk) -> Option<&Vec<Instr>> {
        self.chunks.get(&chunk)
    }

    /// The program: every chunk that has code, in [`Chunk::ORDER`], a blank line after
    /// each ([`Asm::blank_line`]); its labels come from this layout's allocator
    ///
    /// # Panics
    /// If an instruction does not belong in the section it lands in, in this order: code
    /// or data in a RAM section, a section name used twice (see [`Asm::emit`]).
    #[track_caller]
    pub fn program(&self) -> Asm {
        let mut asm = Asm::with_labels(self.labels.clone());
        for chunk in Chunk::ORDER {
            if let Some(code) = self.chunks.get(&chunk) {
                asm.emit_all(code.iter().cloned()).blank_line();
            }
        }
        asm
    }

    instruction_builders!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::label_check::jr_range_errors;

    #[test]
    fn test_chunks_are_printed_in_their_order() {
        // Written in reverse order, printed in Chunk::ORDER, one group each
        let mut layout = Layout::new();
        for chunk in Chunk::ORDER.iter().rev() {
            layout.chunk(*chunk).comment(&format!("{:?}", chunk));
        }
        let text = layout.program().to_asm();
        let expected: String = Chunk::ORDER
            .iter()
            .map(|chunk| format!("    ; {:?}\n\n", chunk))
            .collect();
        assert_eq!(text, expected);
        // The declared order is the print order
        let mut sorted = Chunk::ORDER;
        sorted.sort();
        assert_eq!(sorted, Chunk::ORDER);
    }

    #[test]
    fn test_empty_chunks_print_nothing() {
        let mut layout = Layout::new();
        layout.chunk(Chunk::Init).label("Start");
        layout.chunk(Chunk::Tiles);
        layout.chunk(Chunk::Data).emit_all(Vec::new());
        assert_eq!(layout.program().to_asm(), "    Start:\n\n");
        assert_eq!(Layout::new().program().to_asm(), "");
    }

    #[test]
    fn test_the_chunks_are_relaxed_as_one_program() {
        // A jump from the main loop to a label in the functions: the chunks are one
        // program, in their order, and each one keeps its own instructions
        let build = || {
            let mut layout = Layout::new();
            layout
                .chunk(Chunk::Header)
                .section(Section::rom0("Code").at(0x0000));
            layout.chunk(Chunk::MainLoop).label("Main").jr("Done");
            layout.chunk(Chunk::Functions);
            for _ in 0..200 {
                layout.nop();
            }
            layout.label("Done").jr("Main");
            layout.chunk(Chunk::Init).label("Init").jr("Main");
            layout
        };
        let text = build().program().to_asm();
        assert_eq!(text, build().program().to_asm(), "the same text");
        assert!(text.contains("    jp Done\n\n    nop\n"), "{}", text);
        assert!(text.contains("    jp Main\n\n"), "{}", text);
        assert!(text.contains("Init:\n    jr Main\n\n    Main:"), "{}", text);
        assert_eq!(
            jr_range_errors(&build().program().program()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_the_sections_are_checked_in_the_program_order() {
        // Code written to Functions before the Data chunk opens a RAM section: in the
        // program the Data chunk comes last, so the code stays in ROM
        let mut layout = Layout::new();
        layout
            .chunk(Chunk::Data)
            .section(Section::wram0("Vars"))
            .label("wA")
            .ds("1");
        layout.chunk(Chunk::Header).section(Section::rom0("Code"));
        layout.chunk(Chunk::Functions).label("F").ret();
        let text = layout.program().to_asm();
        assert!(
            text.ends_with("SECTION \"Vars\", WRAM0\n    wA:\n    ds 1\n\n"),
            "{}",
            text
        );

        // Code after the RAM section, in the program order, panics
        layout.chunk(Chunk::Data).ld_a(1);
        let message = crate::rust_boy::panic_message(|| layout.program());
        assert!(
            message.contains("`ld a, 1` in the WRAM0 section \"Vars\""),
            "{}",
            message
        );
    }
}
