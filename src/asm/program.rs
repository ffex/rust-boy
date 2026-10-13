// The builder methods `instruction_builders!` expands into `impl Asm` name these types
use super::block::Emittable;
use super::builders::instruction_builders;
use super::expr::Expr;
use super::instr::{
    AluOperand, Condition, Dst, IncDec, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};
use super::labels::LabelAllocator;
use super::section::{Section, SectionTracker};
use std::fmt::Display;

/// A whole program: its instructions in order, printed by [`Asm::to_asm`]
///
/// It owns the program's [`LabelAllocator`] ([`Asm::labels`]): every label generated code
/// makes up comes from it, so they are unique in the program. [`Asm::to_asm`] turns each
/// `jr` that does not reach its target into a `jp` (see [`Asm::program`]).
///
/// The code goes where it is emitted: the program is printed in that order, in groups of
/// lines separated by a blank line ([`Asm::blank_line`]). Each instruction is checked when
/// it is emitted, against its operands ([`Instr::check`]) and against the section it lands
/// in: code or data in a RAM section, or a section name used twice, panics (see
/// [`Section`]). How a game is laid out (header, start-up code, main loop,
/// functions, data, ...) is the engine's business (`engine::Layout`), not this type's.
pub struct Asm {
    instrs: Vec<Instr>,
    /// Where each group of lines ends (indices into `instrs`), in order
    group_ends: Vec<usize>,
    sections: SectionTracker,
    labels: LabelAllocator,
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
            instrs: Vec::new(),
            group_ends: Vec::new(),
            sections: SectionTracker::default(),
            labels,
        }
    }

    /// The program's label allocator: give it to the code that makes up labels (the
    /// `stdlib` snippets such as `check_key`, any [`Emittable`])
    ///
    /// # Example
    /// ```
    /// use rust_boy::asm::Asm;
    ///
    /// let asm = Asm::new();
    /// assert_eq!(asm.labels().local("loop"), ".loop_0");
    /// assert_eq!(asm.labels().local("loop"), ".loop_1");
    /// ```
    pub fn labels(&self) -> &LabelAllocator {
        &self.labels
    }

    /// Emit `code` (an `If`, a `Block`, ...) at the end of the program, its labels taken
    /// from the program's allocator ([`Asm::labels`])
    ///
    /// # Panics
    /// Panics if one of its instructions is invalid, or does not belong in its section
    /// (see [`Asm::emit`]).
    ///
    /// # Example
    /// ```
    /// use rust_boy::asm::{Asm, Block};
    /// use rust_boy::stdlib::flow::IfA;
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

    /// Emit a single instruction at the end of the program
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects one of its operands, or if it does not belong in
    /// the section it lands in: code or data in a RAM section, a section name used twice
    /// (see [`Section`]).
    ///
    /// ```should_panic
    /// use rust_boy::asm::{Asm, Section};
    ///
    /// // A RAM section only reserves space
    /// Asm::new().section(Section::wram0("Variables")).ld_a(1);
    /// ```
    #[track_caller]
    pub fn emit(&mut self, instr: Instr) -> &mut Self {
        if let Err(error) = instr.check() {
            panic!("invalid instruction: {}", error);
        }
        if let Err(error) = self.try_emit(instr) {
            panic!("invalid program: {}", error);
        }
        self
    }

    /// [`Asm::emit`] for an instruction already checked against its operands: `Err` with
    /// what is wrong, instead of a panic, if it does not belong in the section it lands in
    /// (code or data in a RAM section, a section name used twice); the program is then
    /// unchanged
    ///
    /// ```
    /// use rust_boy::asm::{Asm, Instr, Section};
    ///
    /// let mut asm = Asm::new();
    /// asm.section(Section::wram0("Variables"));
    /// let error = asm.try_emit(Instr::Nop).err().unwrap();
    /// assert!(error.contains("a RAM section holds no code"), "{}", error);
    /// ```
    pub fn try_emit(&mut self, instr: Instr) -> Result<&mut Self, String> {
        self.sections.add(&instr)?;
        self.instrs.push(instr);
        Ok(self)
    }

    /// Emit multiple instructions at the end of the program: a `Vec<Instr>`, a
    /// [`Block`](super::Block), ...
    ///
    /// # Panics
    /// Panics on the first one that [`Asm::emit`] rejects.
    #[track_caller]
    pub fn emit_all(&mut self, instrs: impl IntoIterator<Item = Instr>) -> &mut Self {
        for instr in instrs {
            self.emit(instr);
        }
        self
    }

    /// End the current group of lines: [`Asm::to_asm`] prints a blank line after it
    ///
    /// The program is printed in groups, each followed by a blank line (the last one too);
    /// a group without lines prints nothing, so two calls in a row give one blank line.
    /// It changes nothing in the code.
    ///
    /// ```
    /// use rust_boy::asm::Asm;
    ///
    /// let mut asm = Asm::new();
    /// asm.label("Main").ret().blank_line().blank_line();
    /// asm.label("Other").ret();
    /// assert_eq!(asm.to_asm(), "    Main:\n    ret\n\n    Other:\n    ret\n\n");
    /// ```
    pub fn blank_line(&mut self) -> &mut Self {
        self.group_ends.push(self.instrs.len());
        self
    }

    /// The instructions, as they were emitted (before the jump relaxation of
    /// [`Asm::program`])
    pub fn instrs(&self) -> &[Instr] {
        &self.instrs
    }

    /// Return a copy with flag-safe peephole optimizations applied.
    ///
    /// Currently this rewrites `ld a, 0` to `xor a` and `cp a, 0` to `and a` only when
    /// the next instruction unconditionally overwrites every flag. This preserves the
    /// flags visible after that next instruction while saving one byte and one M-cycle.
    /// It is opt-in; [`Asm::to_asm`](super::codegen::Asm::to_asm) keeps the original code.
    pub fn peephole_optimized(&self) -> Self {
        let mut optimized = Self::with_labels(self.labels.clone());
        let mut start = 0;
        for end in self.group_ends() {
            let group = &self.instrs[start..end];
            for (index, instr) in group.iter().enumerate() {
                let next_overwrites_flags = group.get(index + 1).is_some_and(writes_all_flags);
                let replacement = if next_overwrites_flags {
                    match instr {
                        Instr::Ld {
                            dst: Dst::R8(R8::A),
                            src: Operand::Imm(value),
                        } if value.value() == Some(0) => Some(Instr::Xor { src: R8::A.into() }),
                        Instr::Cp {
                            src: super::instr::AluOperand::Imm(value),
                        } if value.value() == Some(0) => Some(Instr::And { src: R8::A.into() }),
                        _ => None,
                    }
                } else {
                    None
                };
                optimized.emit(replacement.unwrap_or_else(|| instr.clone()));
            }
            optimized.blank_line();
            start = end;
        }
        optimized
    }

    /// Where each group of lines ends, with the end of the program ([`Asm::blank_line`])
    pub(crate) fn group_ends(&self) -> impl Iterator<Item = usize> + '_ {
        self.group_ends
            .iter()
            .copied()
            .chain(std::iter::once(self.instrs.len()))
    }

    instruction_builders!();
}

fn writes_all_flags(instr: &Instr) -> bool {
    matches!(
        instr,
        Instr::Add { .. }
            | Instr::Adc { .. }
            | Instr::Sub { .. }
            | Instr::Sbc { .. }
            | Instr::And { .. }
            | Instr::Xor { .. }
            | Instr::Or { .. }
            | Instr::Cp { .. }
            | Instr::Rlca
            | Instr::Rrca
            | Instr::Rla
            | Instr::Rra
            | Instr::Rlc { .. }
            | Instr::Rrc { .. }
            | Instr::Rl { .. }
            | Instr::Rr { .. }
            | Instr::Sla { .. }
            | Instr::Sra { .. }
            | Instr::Swap { .. }
            | Instr::Srl { .. }
    )
}

#[cfg(test)]
mod peephole_tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;

    #[test]
    fn peepholes_only_run_when_the_following_instruction_overwrites_flags() {
        let mut asm = Asm::new();
        asm.ld_a(0).cp_imm(0).add(1).cp_imm(0).label("Done").ret();
        let optimized = asm.peephole_optimized();

        assert!(asm.to_asm().contains("ld a, 0"));
        assert!(optimized.to_asm().contains("xor a"));
        assert!(optimized.to_asm().contains("and a, a"));

        let original_size: usize = asm.instrs().iter().map(|instr| instr.size().unwrap()).sum();
        let optimized_size: usize = optimized
            .instrs()
            .iter()
            .map(|instr| instr.size().unwrap())
            .sum();
        assert_eq!(original_size - optimized_size, 2);

        let mut original_cpu = TestCpu::default();
        let mut optimized_cpu = TestCpu::default();
        original_cpu.run(asm.instrs());
        optimized_cpu.run(optimized.instrs());
        assert_eq!(
            (original_cpu.a, original_cpu.zero, original_cpu.carry),
            (optimized_cpu.a, optimized_cpu.zero, optimized_cpu.carry)
        );
    }

    #[test]
    fn peepholes_keep_flags_when_the_next_instruction_does_not_replace_them() {
        let mut asm = Asm::new();
        asm.ld_a(0).inc(R8::B).label("Done").ret();
        let optimized = asm.peephole_optimized();
        assert!(optimized.to_asm().contains("ld a, 0"));
    }
}
