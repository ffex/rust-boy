use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

#[derive(Debug, Clone)]
pub enum AnimationType {
    Loop,
    PingPong,
    Once,
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub(crate) name: String,
    pub(crate) oam_index: u8,
    pub(crate) base_tile: u8,   // The sprite's base tile index in VRAM
    pub(crate) start_frame: u8, // Relative start frame (e.g., 0)
    pub(crate) end_frame: u8,   // Relative end frame (e.g., 6)
    pub(crate) anim_type: AnimationType,
    pub(crate) index: u8, // Index of this animation within the sprite (0, 1, 2, ...)
    pub(crate) frame_step: u8, // Tile increment per frame (1 for 8x8, 2 for 8x16)
}

impl Animation {
    pub(crate) fn generate_loop_func(&self) -> Vec<Instr> {
        let mut asm = Asm::new();

        let label_store = format!(".store_{}", self.name);
        // OAM tile index is at offset: oam_index * 4 + 2 (Y=+0, X=+1, Tile=+2, Flags=+3)
        let oam_tile_addr = format!("_OAMRAM+{}", self.oam_index * 4 + 2);

        // Calculate absolute tile indices from base + relative frame
        // For 8x16 sprites, frames are spaced by 2 tiles (frame_step = 2)
        let abs_start = self.base_tile + (self.start_frame * self.frame_step);
        let abs_end = self.base_tile + (self.end_frame * self.frame_step);

        asm.ld_a_addr_def(&oam_tile_addr); // load current sprite tile index

        // Increment by frame_step (1 for 8x8, 2 for 8x16)
        if self.frame_step == 1 {
            asm.inc_label("a");
        } else {
            asm.add(Operand::Reg(Register::A), Operand::Imm(self.frame_step));
        }

        // Check if tile index is within valid range [abs_start, abs_end]
        // If A < abs_start, reset (Carry set after cp means A < value)
        asm.cp_imm(abs_start);
        asm.jr_cond(Condition::C, &format!(".reset_{}", self.name)); // A < abs_start, reset

        // If A > abs_end, reset (A >= abs_end + frame_step means we've gone past)
        asm.cp_imm(abs_end + self.frame_step);
        asm.jr_cond(Condition::C, &label_store); // A < abs_end + frame_step, valid, go store

        // Reset to first frame (A >= abs_end + frame_step OR A < abs_start)
        asm.label(&format!(".reset_{}", self.name));
        asm.ld_a(abs_start);

        asm.label(&label_store);
        asm.ld_addr_def_a(&oam_tile_addr); // store updated sprite tile index

        asm.get_main_instrs()
    }
}
