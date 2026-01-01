use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

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
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.get_main_instrs()
    }

    pub fn move_right(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.get_main_instrs()
    }

    pub fn move_up(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 2))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 2));

        asm.get_main_instrs()
    }

    pub fn move_down(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 2))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 2));

        asm.get_main_instrs()
    }

    //TODO we have to implement movement with limits
}
