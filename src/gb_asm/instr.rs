#[derive(Clone)]
pub enum Instr {
    // Load instructions
    Ld {
        dst: Operand,
        src: Operand,
    },
    Ldh {
        dst: Operand,
        src: Operand,
    },

    // Arithmetic instructions
    Add {
        dst: Operand,
        src: Operand,
    },
    Adc {
        operand: Operand,
    },
    Sub {
        dst: Operand,
        src: Operand,
    },
    Inc {
        operand: Operand,
    },
    Dec {
        operand: Operand,
    },

    // Logical instructions
    And {
        operand: Operand,
    },
    Or {
        dst: Operand,
        src: Operand,
    },
    Xor {
        dst: Operand,
        src: Operand,
    },
    Cp {
        operand: Operand,
    },

    // Bit shift instructions
    Srl {
        operand: Operand,
    },
    Swap {
        operand: Operand,
    },

    // Misc instructions
    Daa,

    // Jump instructions
    Jp {
        target: JumpTarget,
    },
    JpCond {
        condition: Condition,
        target: JumpTarget,
    },
    Jr {
        target: JumpTarget,
    },
    JrCond {
        condition: Condition,
        target: JumpTarget,
    },
    Call {
        target: JumpTarget,
    },
    Ret,
    RetCond {
        condition: Condition,
    },

    // Assembler directives
    Ds {
        num_bytes: String,
        starter_point: String,
    },
    Include {
        file: String,
    },
    Incbin {
        file: String,
        offset: Option<u32>,
        length: Option<u32>,
    },
    Def {
        label: String,
        value: String,
    },
    Section {
        name: String,
        mem_type: String,
    },
    Label {
        name: String,
    },
    Comment {
        text: String,
    },
    Db {
        values: String,
    },
    Dw {
        value: String,
    },
    Raw {
        line: String,
    },
}

#[derive(Clone, Debug)]
pub enum Register {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    SP,
    PC,
    AF,
    BC,
    DE,
    HL,
}

#[derive(Clone, Debug)]
pub enum Operand {
    Reg(Register),
    Imm(u8),
    Imm16(u16),
    Addr(u16),
    AddrDef(String),
    AddrReg(Register),
    AddrRegInc(Register), // [HLI] - address at register with post-increment
    Label(String),
}

#[derive(Clone)]
pub enum JumpTarget {
    Label(String),
    Addr(u16),
}

#[derive(Clone)]
pub enum Condition {
    Z,  // Zero
    NZ, // Not Zero
    C,  // Carry
    NC, // Not Carry
}
