use crate::gb_asm::{Asm, Condition, Instr, JumpTarget, Operand, Register};

pub fn clear_objects_screen() -> Vec<Instr> {
    let mut asm = Asm::new();
    // Clear OAM loop
    asm.label("ClearOam")
        .ld_hli_label("a")
        .dec_label("b")
        .jp_cond(Condition::NZ, "ClearOam");
    asm.get_main_instrs()
}

pub fn initialize_objects_screen() -> Vec<Instr> {
    let mut asm = Asm::new();
    // Initialize OAM
    asm.ld_a(0).ld_b(160).ld_hl_label("_OAMRAM");
    asm.get_main_instrs()
}

pub struct Sprite {
    pub id: u8,
    pub x: u8,
    pub y: u8,
    pub tile: u8,
    pub flags: u8,
}

impl Sprite {
    pub fn new(id: u8, x: u8, y: u8, tile: u8, flags: u8) -> Self {
        Sprite {
            id,
            x,
            y,
            tile,
            flags,
        }
    }
    pub fn set_x(&mut self, x: u8) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: u8) {
        self.y = y;
    }
    pub fn draw(&self) -> Vec<Instr> {
        let mut asm = Asm::new();

        asm.ld_hl_label("_OAMRAM")
            .ld_a(self.x + 16)
            .ld_hli_label("a")
            .ld_a(self.y + 8)
            .ld_hli_label("a")
            .ld_a(self.tile)
            .ld_hli_label("a")
            .ld_a(self.flags)
            .ld_hli_label("a");
        asm.get_main_instrs()
    }
    pub fn move_left(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.label("Left");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.label("LeftEnd");
        asm.get_main_instrs()
    }

    pub fn move_right(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.label("Right");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.label("RightEnd");
        asm.get_main_instrs()
    }

    pub fn move_up(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.label("Up");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 2))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 2));

        asm.label("UpEnd");
        asm.get_main_instrs()
    }

    pub fn move_down(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.label("Down");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 2))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 2));

        asm.label("DownEnd");
        asm.get_main_instrs()
    }

    pub fn move_left_limit(&mut self, distance: u8, limit: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        let jump_label = "LeftLimitEnd";
        asm.label("LeftLimit");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .cp(Operand::Imm(limit))
            .jp_cond(Condition::Z, jump_label)
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.label(jump_label);
        asm.get_main_instrs()
    }

    pub fn move_right_limit(&mut self, distance: u8, limit: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        let jump_label = "RightLimitEnd";
        asm.label("RightLimit");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .cp(Operand::Imm(limit))
            .jp_cond(Condition::Z, jump_label)
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.label(jump_label);
        asm.get_main_instrs()
    }

    pub fn move_up_limit(&mut self, distance: u8, limit: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        let jump_label = "UpLimitEnd";
        asm.label("UpLimit");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 2))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .cp(Operand::Imm(limit))
            .jp_cond(Condition::Z, jump_label)
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 2));

        asm.label(jump_label);
        asm.get_main_instrs()
    }

    pub fn move_down_limit(&mut self, distance: u8, limit: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        let jump_label = "DownLimitEnd";
        asm.label("DownLimit");
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 2))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .cp(Operand::Imm(limit))
            .jp_cond(Condition::Z, jump_label)
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 2));

        asm.label(jump_label);
        asm.get_main_instrs()
    }
    pub fn get_pivot(&self, x_offset: u8, y_offset: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4))
            .sub(Operand::Reg(Register::A), Operand::Imm(16 + y_offset))
            .ld(Operand::Reg(Register::C), Operand::Reg(Register::A))
            .ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .sub(Operand::Reg(Register::A), Operand::Imm(8 + x_offset))
            .ld(Operand::Reg(Register::B), Operand::Reg(Register::A));

        asm.get_main_instrs()
    }
}
