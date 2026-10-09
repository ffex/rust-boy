use super::instr::{Condition, Instr, JumpTarget, Operand, R8, R16, R16Stack, Register};
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

    /// Emit multiple instructions to the current chunk
    ///
    /// # Panics
    /// Panics if [`Instr::check`] rejects an operand of one of them.
    #[track_caller]
    pub fn emit_all(&mut self, instrs: Vec<Instr>) -> &mut Self {
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

    // ============================================
    // Load instructions
    // ============================================

    pub fn ld(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Ld { dst, src })
    }

    pub fn ld_a(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::A), Operand::Imm(value))
    }

    pub fn ld_b(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::B), Operand::Imm(value))
    }

    pub fn ld_c(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::C), Operand::Imm(value))
    }

    pub fn ld_d(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::D), Operand::Imm(value))
    }

    pub fn ld_e(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::E), Operand::Imm(value))
    }

    pub fn ld_h(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::H), Operand::Imm(value))
    }

    pub fn ld_l(&mut self, value: u8) -> &mut Self {
        self.ld(Operand::Reg(Register::L), Operand::Imm(value))
    }

    pub fn ld_bc(&mut self, value: u16) -> &mut Self {
        self.ld(Operand::Reg(Register::BC), Operand::Imm16(value))
    }

    pub fn ld_de(&mut self, value: u16) -> &mut Self {
        self.ld(Operand::Reg(Register::DE), Operand::Imm16(value))
    }

    pub fn ld_hl(&mut self, value: u16) -> &mut Self {
        self.ld(Operand::Reg(Register::HL), Operand::Imm16(value))
    }

    pub fn ld_a_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::A), Operand::Label(label.to_string()))
    }

    pub fn ld_bc_label(&mut self, label: &str) -> &mut Self {
        self.ld(
            Operand::Reg(Register::BC),
            Operand::Label(label.to_string()),
        )
    }

    pub fn ld_de_label(&mut self, label: &str) -> &mut Self {
        self.ld(
            Operand::Reg(Register::DE),
            Operand::Label(label.to_string()),
        )
    }

    pub fn ld_hl_label(&mut self, label: &str) -> &mut Self {
        self.ld(
            Operand::Reg(Register::HL),
            Operand::Label(label.to_string()),
        )
    }

    pub fn ld_hli_label(&mut self, label: &str) -> &mut Self {
        self.ld(
            Operand::AddrRegInc(Register::HL),
            Operand::Label(label.to_string()),
        )
    }

    pub fn ld_b_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::B), Operand::Label(label.to_string()))
    }

    pub fn ld_c_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::C), Operand::Label(label.to_string()))
    }

    pub fn ld_d_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::D), Operand::Label(label.to_string()))
    }

    pub fn ld_e_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::E), Operand::Label(label.to_string()))
    }

    pub fn ld_h_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::H), Operand::Label(label.to_string()))
    }

    pub fn ld_l_label(&mut self, label: &str) -> &mut Self {
        self.ld(Operand::Reg(Register::L), Operand::Label(label.to_string()))
    }

    pub fn ld_addr_label_a(&mut self, address: &str) -> &mut Self {
        self.ld(
            Operand::Label(address.to_string()),
            Operand::Reg(Register::A),
        )
    }

    pub fn ldh(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Ldh { dst, src })
    }

    pub fn ldh_label(&mut self, dest: &str, src: &str) -> &mut Self {
        self.ldh(
            Operand::Label(dest.to_string()),
            Operand::Label(src.to_string()),
        )
    }

    pub fn ld_a_addr_def(&mut self, def_name: &str) -> &mut Self {
        self.ld(
            Operand::Reg(Register::A),
            Operand::AddrDef(def_name.to_string()),
        )
    }

    pub fn ld_addr_def_a(&mut self, def_name: &str) -> &mut Self {
        self.ld(
            Operand::AddrDef(def_name.to_string()),
            Operand::Reg(Register::A),
        )
    }

    pub fn ld_a_addr_reg(&mut self, reg: Register) -> &mut Self {
        self.ld(Operand::Reg(Register::A), Operand::AddrReg(reg))
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
    // `src` is an 8-bit register, `[hl]` (`Operand::AddrReg(Register::HL)`), a number
    // or an expression (`Operand::Label`); anything else panics.

    /// `add a, src`
    #[track_caller]
    pub fn add(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Add { src })
    }

    /// `add a, src` or `add hl, src`, both as text: `add_label("a", "5")`,
    /// `add_label("hl", "bc")`
    ///
    /// # Panics
    /// Panics if `dst` is not `a` or `hl`, or for `hl` if `src` is not `bc`, `de`, `hl`
    /// or `sp`.
    #[track_caller]
    pub fn add_label(&mut self, dst: &str, src: &str) -> &mut Self {
        match dst.trim().to_ascii_lowercase().as_str() {
            "a" => self.add(Operand::Label(src.to_string())),
            "hl" => self.add_hl(R16::from_name(src)),
            _ => panic!(
                "add_label({:?}, {:?}): the destination of add must be a or hl \
                 (add_sp for sp)",
                dst, src
            ),
        }
    }

    /// `adc a, src`: `a + src + carry`
    #[track_caller]
    pub fn adc(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Adc { src })
    }

    /// `adc a, src`, the source as text
    #[track_caller]
    pub fn adc_label(&mut self, src: &str) -> &mut Self {
        self.adc(Operand::Label(src.to_string()))
    }

    /// `sub a, src`
    #[track_caller]
    pub fn sub(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Sub { src })
    }

    /// `sub a, src`, both as text: `sub_label("a", "8")`
    ///
    /// # Panics
    /// Panics if `dst` is not `a`.
    #[track_caller]
    pub fn sub_label(&mut self, dst: &str, src: &str) -> &mut Self {
        check_dst_a("sub", dst, src);
        self.sub(Operand::Label(src.to_string()))
    }

    /// `sbc a, src`: `a - src - carry`
    #[track_caller]
    pub fn sbc(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Sbc { src })
    }

    /// `and a, src`
    #[track_caller]
    pub fn and(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::And { src })
    }

    /// `and a, src`, the source as text
    #[track_caller]
    pub fn and_label(&mut self, src: &str) -> &mut Self {
        self.and(Operand::Label(src.to_string()))
    }

    /// `xor a, src`
    #[track_caller]
    pub fn xor(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Xor { src })
    }

    /// `xor a, src`, both as text: `xor_label("a", "b")`
    ///
    /// # Panics
    /// Panics if `dst` is not `a`.
    #[track_caller]
    pub fn xor_label(&mut self, dst: &str, src: &str) -> &mut Self {
        check_dst_a("xor", dst, src);
        self.xor(Operand::Label(src.to_string()))
    }

    /// `or a, src`
    #[track_caller]
    pub fn or(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Or { src })
    }

    /// `or a, src`, both as text: `or_label("a", "c")`
    ///
    /// # Panics
    /// Panics if `dst` is not `a`.
    #[track_caller]
    pub fn or_label(&mut self, dst: &str, src: &str) -> &mut Self {
        check_dst_a("or", dst, src);
        self.or(Operand::Label(src.to_string()))
    }

    /// `cp a, src`: the flags of `a - src`
    #[track_caller]
    pub fn cp(&mut self, src: Operand) -> &mut Self {
        self.emit(Instr::Cp { src })
    }

    /// `cp a, value`
    pub fn cp_imm(&mut self, value: u8) -> &mut Self {
        self.cp(Operand::Imm(value))
    }

    /// `cp a, src`, the source as text
    #[track_caller]
    pub fn cp_label(&mut self, src: &str) -> &mut Self {
        self.cp(Operand::Label(src.to_string()))
    }

    pub fn inc(&mut self, operand: Operand) -> &mut Self {
        self.emit(Instr::Inc { operand })
    }

    pub fn inc_label(&mut self, register: &str) -> &mut Self {
        self.inc(Operand::Label(register.to_string()))
    }

    pub fn dec(&mut self, operand: Operand) -> &mut Self {
        self.emit(Instr::Dec { operand })
    }

    pub fn dec_label(&mut self, register: &str) -> &mut Self {
        self.dec(Operand::Label(register.to_string()))
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

    /// `srl operand`, the operand as text (`"a"`, `"[hl]"`)
    ///
    /// # Panics
    /// Panics if `register` is not an 8-bit register or `[hl]`.
    #[track_caller]
    pub fn srl_label(&mut self, register: &str) -> &mut Self {
        self.srl(R8::from_name(register))
    }

    /// `swap operand`: swap the two nibbles
    pub fn swap(&mut self, operand: R8) -> &mut Self {
        self.emit(Instr::Swap { operand })
    }

    /// `swap operand`, the operand as text (`"a"`, `"[hl]"`)
    ///
    /// # Panics
    /// Panics if `register` is not an 8-bit register or `[hl]`.
    #[track_caller]
    pub fn swap_label(&mut self, register: &str) -> &mut Self {
        self.swap(R8::from_name(register))
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

    pub fn ds(&mut self, num_bytes: &str, starter_point: &str) -> &mut Self {
        self.emit(Instr::Ds {
            num_bytes: num_bytes.to_string(),
            starter_point: starter_point.to_string(),
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

    pub fn section(&mut self, name: &str, mem_type: &str) -> &mut Self {
        self.emit(Instr::Section {
            name: name.to_string(),
            mem_type: mem_type.to_string(),
        })
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
}

/// Panics unless `dst`, the destination of an 8-bit ALU instruction written as text, is `a`
#[track_caller]
fn check_dst_a(mnemonic: &str, dst: &str, src: &str) {
    assert!(
        dst.trim().eq_ignore_ascii_case("a"),
        "{}_label({:?}, {:?}): the destination of {} must be a",
        mnemonic,
        dst,
        src,
        mnemonic
    );
}
