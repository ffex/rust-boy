use crate::gb_asm::{Asm, Condition, Instr};

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
    pub(crate) initially_enabled: bool, // Whether the animation starts enabled
}

impl Animation {
    pub(crate) fn generate_loop_func(&self) -> Vec<Instr> {
        let mut asm = Asm::new();

        let label = format!("updateSpriteIndex_{}", self.name);
        // OAM tile index is at offset: oam_index * 4 + 2 (Y=+0, X=+1, Tile=+2, Flags=+3)
        let oam_tile_addr = format!("_OAMRAM+{}", self.oam_index * 4 + 2);

        // Calculate absolute tile indices from base + relative frame
        let abs_start = self.base_tile + self.start_frame;
        let abs_end = self.base_tile + self.end_frame;

        asm.ld_a_addr_def(&oam_tile_addr) // load current sprite tile index
            .inc_label("a") // increment frame
            .cp_imm(abs_end + 1) // check if past the last frame
            .jr_cond(Condition::NZ, &label) // if still in range, skip reset
            .ld_a(abs_start) // reset to first frame
            .label(&label) // .updateSpriteIndex label
            .ld_addr_def_a(&oam_tile_addr); // store updated sprite tile index

        asm.get_main_instrs()
    }
}
