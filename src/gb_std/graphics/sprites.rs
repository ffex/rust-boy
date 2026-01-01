use crate::gb_asm::{Asm, Condition, Instr};

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
    pub x: u8,
    pub y: u8,
    pub tile: u8,
    pub flags: u8,
}

impl Sprite {
    pub fn new(x: u8, y: u8, tile: u8, flags: u8) -> Self {
        Sprite { x, y, tile, flags }
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
    //TODO we have to implement movement
}
