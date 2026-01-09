use super::instr::{Condition, Instr, JumpTarget, Operand, Register};
use std::collections::HashMap;
use std::fmt::Display;

pub struct Asm {
    pub(crate) chunks: HashMap<Chunk, Vec<Instr>>,
    current_chunk: Chunk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chunk {
    Main,
    Functions,
    Tiles,
    Tilemap,
    Data,
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
    pub fn emit(&mut self, instr: Instr) -> &mut Self {
        self.chunks
            .entry(self.current_chunk)
            .or_insert_with(Vec::new)
            .push(instr);
        self
    }

    /// Emit multiple instructions to the current chunk
    pub fn emit_all(&mut self, instrs: Vec<Instr>) -> &mut Self {
        self.chunks
            .entry(self.current_chunk)
            .or_insert_with(Vec::new)
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
    // ============================================
    // Arithmetic instructions
    // ============================================

    pub fn add(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Add { dst, src })
    }

    pub fn add_label(&mut self, reg_a: &str, reg_b: &str) -> &mut Self {
        self.add(
            Operand::Label(reg_a.to_string()),
            Operand::Label(reg_b.to_string()),
        )
    }

    pub fn adc_a(&mut self, operand: Operand) -> &mut Self {
        self.emit(Instr::AdcA { operand })
    }

    pub fn adc(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Adc { dst, src })
    }

    pub fn adc_label(&mut self, reg: &str) -> &mut Self {
        self.adc(Operand::Label(reg.to_string()))
    }

    pub fn sub(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Sub { dst, src })
    }

    pub fn sub_label(&mut self, reg_a: &str, reg_b: &str) -> &mut Self {
        self.sub(
            Operand::Label(reg_a.to_string()),
            Operand::Label(reg_b.to_string()),
        )
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
    // Logical instructions
    // ============================================

    pub fn and(&mut self, operand: Operand) -> &mut Self {
        //TODO redo maybe?
        self.emit(Instr::And { operand })
    }

    pub fn and_label(&mut self, value: &str) -> &mut Self {
        self.and(Operand::Label(value.to_string()))
    }

    pub fn or(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Or { dst, src })
    }

    pub fn or_label(&mut self, reg_a: &str, reg_b: &str) -> &mut Self {
        self.or(
            Operand::Label(reg_a.to_string()),
            Operand::Label(reg_b.to_string()),
        )
    }

    pub fn xor(&mut self, dst: Operand, src: Operand) -> &mut Self {
        self.emit(Instr::Xor { dst, src })
    }

    pub fn xor_label(&mut self, reg_a: &str, reg_b: &str) -> &mut Self {
        self.xor(
            Operand::Label(reg_a.to_string()),
            Operand::Label(reg_b.to_string()),
        )
    }

    pub fn cp(&mut self, operand: Operand) -> &mut Self {
        //TODO check this (cp a, 14)
        self.emit(Instr::Cp { operand })
    }

    pub fn cp_imm(&mut self, value: u8) -> &mut Self {
        self.cp(Operand::Imm(value))
    }

    pub fn cp_label(&mut self, value: &str) -> &mut Self {
        self.cp(Operand::Label(value.to_string()))
    }

    // ============================================
    // Bit shift instructions
    // ============================================

    pub fn srl(&mut self, operand: Operand) -> &mut Self {
        self.emit(Instr::Srl { operand })
    }

    pub fn srl_label(&mut self, register: &str) -> &mut Self {
        self.srl(Operand::Label(register.to_string()))
    }

    pub fn swap(&mut self, operand: Operand) -> &mut Self {
        self.emit(Instr::Swap { operand })
    }

    pub fn swap_label(&mut self, register: &str) -> &mut Self {
        self.swap(Operand::Label(register.to_string()))
    }

    // ============================================
    // Misc instructions
    // ============================================

    pub fn daa(&mut self) -> &mut Self {
        self.emit(Instr::Daa)
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

    pub fn ret(&mut self) -> &mut Self {
        self.emit(Instr::Ret)
    }

    pub fn ret_cond(&mut self, condition: Condition) -> &mut Self {
        self.emit(Instr::RetCond { condition })
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
