//! The instruction and directive builders of [`Asm`](super::Asm) and
//! [`Block`](super::Block), written once.
//!
//! `instruction_builders!` expands into the `impl` of each: one method per instruction
//! (`ld`, `add`, `jp_cond`, ...) and directive (`label`, `section`, `db`, ...), each a call
//! to the type's own `emit`, which checks the instruction ([`Instr::check`]) and adds it
//! where that type keeps its code (the current chunk of an `Asm`, the end of a
//! `Block`). The module that expands it imports the operand types the methods name.
//!
//! [`Instr::check`]: super::Instr::check

/// The builder methods shared by `Asm` and `Block`; see the module documentation
macro_rules! instruction_builders {
    () => {
        // ============================================
        // Load instructions
        // ============================================
        // The operands are typed: a destination is a register (`R8`, `R16`) or memory
        // (`Mem`, `R8::AtHl`), a source is one of those or a value (an `Expr`, a Rust integer,
        // or a symbol or number as text). A pair that no SM83 load takes (`ld [hl], [hl]`,
        // `ld b, [de]`) or a value that does not fit panics.

        /// `ld dst, src`: `ld(R8::B, R8::A)`, `ld(R8::A, 5)`, `ld(R16::HL, "_OAMRAM")`,
        /// `ld(Mem::Hli, R8::A)`, `ld(Mem::addr("wScore"), R8::A)`
        ///
        /// ```
        /// use rust_boy::gb_asm::{Block, Expr, Mem, R8, R16};
        ///
        /// let mut asm = Block::new();
        /// asm.ld(R16::HL, Expr::sym("_OAMRAM") + 4)
        ///     .ld(R8::A, -1)
        ///     .ld(Mem::Hli, R8::A)
        ///     .ld(R8::B, R8::AtHl);
        /// let text: Vec<String> = asm.iter().map(|i| i.to_string()).collect();
        /// assert_eq!(text, ["ld hl, _OAMRAM+4", "ld a, -1", "ld [hli], a", "ld b, [hl]"]);
        /// ```
        ///
        /// A value is never a destination, so `ld 1, 2` does not compile:
        /// ```compile_fail,E0277
        /// use rust_boy::gb_asm::Asm;
        ///
        /// Asm::new().ld(1, 2);
        /// ```
        ///
        /// # Panics
        /// If [`Instr::check`] rejects the operands: a pair that no SM83 load takes
        /// (`ld [hl], [hl]`, `ld b, [de]`, `ld bc, de`, `ld [hli], 5`), or a constant that does
        /// not fit (`ld a, 300`).
        #[track_caller]
        pub fn ld(&mut self, dst: impl Into<Dst>, src: impl Into<Operand>) -> &mut Self {
            self.emit(Instr::Ld {
                dst: dst.into(),
                src: src.into(),
            })
        }

        /// `ld a, value`
        pub fn ld_a(&mut self, value: u8) -> &mut Self {
            self.ld(R8::A, value)
        }

        /// `ld b, value`
        pub fn ld_b(&mut self, value: u8) -> &mut Self {
            self.ld(R8::B, value)
        }

        /// `ld c, value`
        pub fn ld_c(&mut self, value: u8) -> &mut Self {
            self.ld(R8::C, value)
        }

        /// `ld d, value`
        pub fn ld_d(&mut self, value: u8) -> &mut Self {
            self.ld(R8::D, value)
        }

        /// `ld e, value`
        pub fn ld_e(&mut self, value: u8) -> &mut Self {
            self.ld(R8::E, value)
        }

        /// `ld h, value`
        pub fn ld_h(&mut self, value: u8) -> &mut Self {
            self.ld(R8::H, value)
        }

        /// `ld l, value`
        pub fn ld_l(&mut self, value: u8) -> &mut Self {
            self.ld(R8::L, value)
        }

        /// `ld bc, value`
        pub fn ld_bc(&mut self, value: u16) -> &mut Self {
            self.ld(R16::BC, value)
        }

        /// `ld de, value`
        pub fn ld_de(&mut self, value: u16) -> &mut Self {
            self.ld(R16::DE, value)
        }

        /// `ld hl, value`
        pub fn ld_hl(&mut self, value: u16) -> &mut Self {
            self.ld(R16::HL, value)
        }

        /// `ld a, [address]`: `ld_a_addr_def("rLY")`, `ld_a_addr_def(Expr::sym("_OAMRAM") + 4)`
        ///
        /// # Panics
        /// If `address` is text that is neither a symbol nor a number (see [`Expr`]).
        #[track_caller]
        pub fn ld_a_addr_def(&mut self, address: impl Into<Expr>) -> &mut Self {
            self.ld(R8::A, Mem::addr(address))
        }

        /// `ld [address], a`: `ld_addr_def_a("rLCDC")`, `ld_addr_def_a(Expr::sym("wScore") + 1)`
        ///
        /// # Panics
        /// If `address` is text that is neither a symbol nor a number (see [`Expr`]).
        #[track_caller]
        pub fn ld_addr_def_a(&mut self, address: impl Into<Expr>) -> &mut Self {
            self.ld(Mem::addr(address), R8::A)
        }

        /// `ldh dst, src`: `ldh(Mem::addr("rP1"), R8::A)`, `ldh(R8::A, Mem::C)`
        ///
        /// # Panics
        /// If [`Instr::check`] rejects the operands: `ldh` moves `a` to or from `[c]` or an
        /// address from `$FF00` to `$FFFF`.
        #[track_caller]
        pub fn ldh(&mut self, dst: impl Into<Dst>, src: impl Into<Operand>) -> &mut Self {
            self.emit(Instr::Ldh {
                dst: dst.into(),
                src: src.into(),
            })
        }

        /// `ld hl, sp + offset`
        pub fn ld_hl_sp(&mut self, offset: i8) -> &mut Self {
            self.emit(Instr::LdHlSp { offset })
        }

        /// `push pair`
        pub fn push(&mut self, pair: R16Stack) -> &mut Self {
            self.emit(Instr::Push { pair })
        }

        /// `pop pair`
        pub fn pop(&mut self, pair: R16Stack) -> &mut Self {
            self.emit(Instr::Pop { pair })
        }

        // ============================================
        // 8-bit arithmetic and logic: `op a, src`
        // ============================================
        // `src` is an 8-bit register or `[hl]` (`R8`), or a value: an `Expr`, a Rust integer,
        // or a symbol or number as text (`cp("BRICK_LEFT")`). A constant that does not fit in
        // 8 bits (-128 to 255) panics.

        /// `add a, src`
        ///
        /// The source is an 8-bit register, `[hl]` or a value; a 16-bit register or memory
        /// does not compile (`add a, hl`, `cp a, [wCount]`):
        /// ```compile_fail,E0277
        /// use rust_boy::gb_asm::{Asm, R16};
        ///
        /// Asm::new().add(R16::HL);
        /// ```
        /// ```compile_fail,E0277
        /// use rust_boy::gb_asm::{Asm, Mem};
        ///
        /// Asm::new().cp(Mem::addr("wCount"));
        /// ```
        #[track_caller]
        pub fn add(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Add { src: src.into() })
        }

        /// `adc a, src`: `a + src + carry`
        #[track_caller]
        pub fn adc(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Adc { src: src.into() })
        }

        /// `sub a, src`
        #[track_caller]
        pub fn sub(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Sub { src: src.into() })
        }

        /// `sbc a, src`: `a - src - carry`
        #[track_caller]
        pub fn sbc(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Sbc { src: src.into() })
        }

        /// `and a, src`
        #[track_caller]
        pub fn and(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::And { src: src.into() })
        }

        /// `xor a, src`
        #[track_caller]
        pub fn xor(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Xor { src: src.into() })
        }

        /// `or a, src`
        #[track_caller]
        pub fn or(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Or { src: src.into() })
        }

        /// `cp a, src`: the flags of `a - src`
        #[track_caller]
        pub fn cp(&mut self, src: impl Into<AluOperand>) -> &mut Self {
            self.emit(Instr::Cp { src: src.into() })
        }

        /// `cp a, value`
        pub fn cp_imm(&mut self, value: u8) -> &mut Self {
            self.cp(value)
        }

        /// `inc operand`: an 8-bit register, `[hl]` or a 16-bit register
        ///
        /// Anything else does not compile, a value (`inc 5`) or memory (`inc [wCount]`):
        /// ```compile_fail,E0277
        /// use rust_boy::gb_asm::Asm;
        ///
        /// Asm::new().inc(5);
        /// ```
        /// ```compile_fail,E0277
        /// use rust_boy::gb_asm::{Asm, Mem};
        ///
        /// Asm::new().inc(Mem::addr("wCount"));
        /// ```
        pub fn inc(&mut self, operand: impl Into<IncDec>) -> &mut Self {
            self.emit(Instr::Inc {
                operand: operand.into(),
            })
        }

        /// `dec operand`: an 8-bit register, `[hl]` or a 16-bit register
        pub fn dec(&mut self, operand: impl Into<IncDec>) -> &mut Self {
            self.emit(Instr::Dec {
                operand: operand.into(),
            })
        }

        // ============================================
        // 16-bit arithmetic
        // ============================================

        /// `add hl, src`
        pub fn add_hl(&mut self, src: R16) -> &mut Self {
            self.emit(Instr::AddHl { src })
        }

        /// `add sp, offset`
        pub fn add_sp(&mut self, offset: i8) -> &mut Self {
            self.emit(Instr::AddSp { offset })
        }

        // ============================================
        // Rotates, shifts and swap
        // ============================================

        /// `rlca`: rotate `a` left, bit 7 into the carry and bit 0 (Z reset)
        pub fn rlca(&mut self) -> &mut Self {
            self.emit(Instr::Rlca)
        }

        /// `rrca`: rotate `a` right, bit 0 into the carry and bit 7 (Z reset)
        pub fn rrca(&mut self) -> &mut Self {
            self.emit(Instr::Rrca)
        }

        /// `rla`: rotate `a` left through the carry (Z reset)
        pub fn rla(&mut self) -> &mut Self {
            self.emit(Instr::Rla)
        }

        /// `rra`: rotate `a` right through the carry (Z reset)
        pub fn rra(&mut self) -> &mut Self {
            self.emit(Instr::Rra)
        }

        /// `rlc operand`: rotate left, bit 7 into the carry and bit 0
        pub fn rlc(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Rlc { operand })
        }

        /// `rrc operand`: rotate right, bit 0 into the carry and bit 7
        pub fn rrc(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Rrc { operand })
        }

        /// `rl operand`: rotate left through the carry
        pub fn rl(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Rl { operand })
        }

        /// `rr operand`: rotate right through the carry
        pub fn rr(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Rr { operand })
        }

        /// `sla operand`: shift left, bit 7 into the carry
        pub fn sla(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Sla { operand })
        }

        /// `sra operand`: shift right, bit 0 into the carry, bit 7 kept (signed halving)
        pub fn sra(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Sra { operand })
        }

        /// `srl operand`: shift right, bit 0 into the carry, bit 7 reset
        pub fn srl(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Srl { operand })
        }

        /// `swap operand`: swap the two nibbles
        pub fn swap(&mut self, operand: R8) -> &mut Self {
            self.emit(Instr::Swap { operand })
        }

        // ============================================
        // Bit instructions
        // ============================================
        // `bit` is 0 to 7; anything else panics.

        /// `bit n, operand`: Z set when bit `n` is 0
        #[track_caller]
        pub fn bit(&mut self, bit: u8, operand: R8) -> &mut Self {
            self.emit(Instr::Bit { bit, operand })
        }

        /// `set n, operand`
        #[track_caller]
        pub fn set(&mut self, bit: u8, operand: R8) -> &mut Self {
            self.emit(Instr::Set { bit, operand })
        }

        /// `res n, operand`
        #[track_caller]
        pub fn res(&mut self, bit: u8, operand: R8) -> &mut Self {
            self.emit(Instr::Res { bit, operand })
        }

        // ============================================
        // Flags, accumulator and CPU control
        // ============================================

        pub fn daa(&mut self) -> &mut Self {
            self.emit(Instr::Daa)
        }

        /// `cpl`: `a = !a`
        pub fn cpl(&mut self) -> &mut Self {
            self.emit(Instr::Cpl)
        }

        /// `scf`: set the carry
        pub fn scf(&mut self) -> &mut Self {
            self.emit(Instr::Scf)
        }

        /// `ccf`: complement the carry
        pub fn ccf(&mut self) -> &mut Self {
            self.emit(Instr::Ccf)
        }

        pub fn nop(&mut self) -> &mut Self {
            self.emit(Instr::Nop)
        }

        pub fn halt(&mut self) -> &mut Self {
            self.emit(Instr::Halt)
        }

        pub fn stop(&mut self) -> &mut Self {
            self.emit(Instr::Stop)
        }

        /// `di`: disable interrupts
        pub fn di(&mut self) -> &mut Self {
            self.emit(Instr::Di)
        }

        /// `ei`: enable interrupts (after the next instruction)
        pub fn ei(&mut self) -> &mut Self {
            self.emit(Instr::Ei)
        }

        // ============================================
        // Jump instructions
        // ============================================

        pub fn jp(&mut self, label: &str) -> &mut Self {
            self.emit(Instr::Jp {
                target: JumpTarget::Label(label.to_string()),
            })
        }

        pub fn jp_cond(&mut self, condition: Condition, label: &str) -> &mut Self {
            self.emit(Instr::JpCond {
                condition,
                target: JumpTarget::Label(label.to_string()),
            })
        }

        /// `jp hl`: jump to the address in `hl`
        pub fn jp_hl(&mut self) -> &mut Self {
            self.emit(Instr::JpHl)
        }

        pub fn jr(&mut self, label: &str) -> &mut Self {
            self.emit(Instr::Jr {
                target: JumpTarget::Label(label.to_string()),
            })
        }

        pub fn jr_cond(&mut self, condition: Condition, label: &str) -> &mut Self {
            self.emit(Instr::JrCond {
                condition,
                target: JumpTarget::Label(label.to_string()),
            })
        }

        pub fn call(&mut self, label: &str) -> &mut Self {
            self.emit(Instr::Call {
                target: JumpTarget::Label(label.to_string()),
            })
        }

        /// `call condition, label`
        pub fn call_cond(&mut self, condition: Condition, label: &str) -> &mut Self {
            self.emit(Instr::CallCond {
                condition,
                target: JumpTarget::Label(label.to_string()),
            })
        }

        pub fn ret(&mut self) -> &mut Self {
            self.emit(Instr::Ret)
        }

        pub fn ret_cond(&mut self, condition: Condition) -> &mut Self {
            self.emit(Instr::RetCond { condition })
        }

        /// `reti`: return and enable interrupts
        pub fn reti(&mut self) -> &mut Self {
            self.emit(Instr::Reti)
        }

        /// `rst vector`: call one of the restart vectors `$00`, `$08`, …, `$38`
        ///
        /// # Panics
        /// Panics on any other vector.
        #[track_caller]
        pub fn rst(&mut self, vector: u8) -> &mut Self {
            self.emit(Instr::Rst { vector })
        }

        // ============================================
        // Assembler directives
        // ============================================

        /// `ds count`: reserve `count` bytes (an RGBDS expression: `4`, `$150 - @`), the way
        /// a RAM section takes room; in ROM, rgblink fills them with its padding value
        pub fn ds(&mut self, count: &str) -> &mut Self {
            self.emit(Instr::Ds {
                count: count.to_string(),
                fill: None,
            })
        }

        /// `ds count, fill`: `count` bytes of `fill` (ROM only: a RAM section holds no data)
        pub fn ds_fill(&mut self, count: &str, fill: &str) -> &mut Self {
            self.emit(Instr::Ds {
                count: count.to_string(),
                fill: Some(fill.to_string()),
            })
        }

        pub fn include_hardware(&mut self) -> &mut Self {
            self.emit(Instr::Include {
                file: "hardware.inc".to_string(),
            })
        }

        pub fn include(&mut self, file: &str) -> &mut Self {
            self.emit(Instr::Include {
                file: file.to_string(),
            })
        }

        pub fn incbin(&mut self, file: &str) -> &mut Self {
            self.emit(Instr::Incbin {
                file: file.to_string(),
                offset: None,
                length: None,
            })
        }

        pub fn incbin_range(&mut self, file: &str, offset: u32, length: u32) -> &mut Self {
            self.emit(Instr::Incbin {
                file: file.to_string(),
                offset: Some(offset),
                length: Some(length),
            })
        }

        pub fn incbin_offset(&mut self, file: &str, offset: u32) -> &mut Self {
            self.emit(Instr::Incbin {
                file: file.to_string(),
                offset: Some(offset),
                length: None,
            })
        }

        pub fn def<T: Display>(&mut self, label: &str, value: T) -> &mut Self {
            let value_str = format!("{}", value);
            self.emit(Instr::Def {
                label: label.to_string(),
                value: value_str,
            })
        }

        /// `SECTION ...`: the code and data that follow go in `section` (see [`Section`])
        ///
        /// ```
        /// use rust_boy::gb_asm::{Block, Section};
        ///
        /// let mut asm = Block::new();
        /// asm.section(Section::wram0("Variables")).label("wScore").ds("1");
        /// assert_eq!(asm[0].to_string(), r#"SECTION "Variables", WRAM0"#);
        /// ```
        pub fn section(&mut self, section: Section) -> &mut Self {
            self.emit(Instr::Section(section))
        }

        pub fn label(&mut self, name: &str) -> &mut Self {
            self.emit(Instr::Label {
                name: name.to_string(),
            })
        }

        pub fn comment(&mut self, text: &str) -> &mut Self {
            self.emit(Instr::Comment {
                text: text.to_string(),
            })
        }

        pub fn db(&mut self, values: &str) -> &mut Self {
            self.emit(Instr::Db {
                values: values.to_string(),
            })
        }

        pub fn dw(&mut self, value: &str) -> &mut Self {
            self.emit(Instr::Dw {
                value: value.to_string(),
            })
        }

        pub fn raw(&mut self, line: &str) -> &mut Self {
            self.emit(Instr::Raw {
                line: line.to_string(),
            })
        }
    };
}

pub(super) use instruction_builders;
