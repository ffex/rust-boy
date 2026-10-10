use crate::asm::{Block, Condition, Expr, Instr, R8};
use crate::hw;
use crate::stdlib::graphics::sprites::oam_address;

/// How an animation goes on after its last frame
///
/// The frames are `start_frame..=end_frame`. Each update shows the next frame; a sprite
/// that shows none of the animation's frames (it was showing another animation, or its
/// initial tile) starts again on the first frame.
#[derive(Debug, Clone)]
pub enum AnimationType {
    /// After the last frame, start again on the first one: 0 1 2 3 0 1 2 3 ...
    Loop,
    /// Play forward, then backward, and repeat; the first and last frames are shown
    /// once per turn: 0 1 2 3 2 1 0 1 2 ... The direction is kept in a WRAM variable
    /// per sprite, `wAnim_{sprite name}_Dir` (0 = forward, 1 = backward); a one-frame
    /// `PingPong` needs none.
    PingPong,
    /// Play the frames once, then stay on the last one: 0 1 2 3 3 3 ...
    /// Enabling it again while the sprite shows its last frame does not replay it: the
    /// sprite must show another tile first (e.g. another animation).
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
    /// Whether this animation needs the sprite's direction variable: a `PingPong` of
    /// two frames or more (one frame has nowhere to go, so its code does not read it)
    pub(crate) fn needs_direction(&self) -> bool {
        let (abs_start, abs_end) = self.abs_frames();
        matches!(self.anim_type, AnimationType::PingPong) && abs_start != abs_end
    }

    /// The body of the function that shows the next frame (without its label and `ret`)
    ///
    /// `direction_var` is the sprite's direction variable, used by `PingPong` only. Only
    /// the register `a` and the flags are changed.
    pub(crate) fn generate_func(&self, direction_var: &str) -> Vec<Instr> {
        match self.anim_type {
            AnimationType::Loop => self.generate_loop_func(),
            AnimationType::PingPong => self.generate_ping_pong_func(direction_var),
            AnimationType::Once => self.generate_once_func(),
        }
    }

    /// OAM address of the sprite's tile index: byte 2 of its entry (Y, X, tile, flags)
    fn oam_tile_addr(&self) -> Expr {
        oam_address(self.oam_index, hw::OAMA_TILEID)
    }

    /// Absolute tile indices of the first and last frames; frames are `frame_step` tiles
    /// apart (2 for 8x16 sprites)
    fn abs_frames(&self) -> (u8, u8) {
        let abs_start = self.base_tile + (self.start_frame * self.frame_step);
        let abs_end = self.base_tile + (self.end_frame * self.frame_step);
        (abs_start, abs_end)
    }

    /// `a` = the next frame's tile
    fn step_forward(&self, asm: &mut Block) {
        if self.frame_step == 1 {
            asm.inc(R8::A);
        } else {
            asm.add(self.frame_step);
        }
    }

    /// `a` = the previous frame's tile
    fn step_backward(&self, asm: &mut Block) {
        if self.frame_step == 1 {
            asm.dec(R8::A);
        } else {
            asm.sub(self.frame_step);
        }
    }

    /// Forward to the last frame, then the first one again
    ///
    /// It compares the current tile with the first and the last frame themselves, like
    /// `Once` and `PingPong`: it used to step first and compare with `last frame +
    /// frame_step`, which is 256 when the last frame is tile 255 (8x8) or 254 (8x16), so
    /// the animation froze on its first frame (B17). The code has the same size as before.
    fn generate_loop_func(&self) -> Vec<Instr> {
        let mut asm = Block::new();

        let label_reset = format!(".reset_{}", self.name);
        let label_next = format!(".next_{}", self.name);
        let oam_tile_addr = self.oam_tile_addr();
        let (abs_start, abs_end) = self.abs_frames();

        asm.ld_a_addr_def(&oam_tile_addr); // load current sprite tile index
        asm.cp_imm(abs_start);
        asm.jr_cond(Condition::C, &label_reset); // before the first frame: start
        asm.cp_imm(abs_end);
        asm.jr_cond(Condition::C, &label_next); // before the last frame: next frame

        // On the last frame or past it: start again. The step below lands on the first
        // frame (modulo 256: from frame 0, `a` = 255 or 254, then 0)
        asm.label(&label_reset);
        asm.ld_a(abs_start.wrapping_sub(self.frame_step));

        // Increment by frame_step (1 for 8x8, 2 for 8x16)
        asm.label(&label_next);
        self.step_forward(&mut asm);
        asm.ld_addr_def_a(&oam_tile_addr); // store updated sprite tile index

        asm.into_instrs()
    }

    /// Forward to the last frame, which stays (B10)
    fn generate_once_func(&self) -> Vec<Instr> {
        let mut asm = Block::new();

        let label_reset = format!(".reset_{}", self.name);
        let label_store = format!(".store_{}", self.name);
        let oam_tile_addr = self.oam_tile_addr();
        let (abs_start, abs_end) = self.abs_frames();

        asm.ld_a_addr_def(&oam_tile_addr);
        asm.cp_imm(abs_start);
        asm.jr_cond(Condition::C, &label_reset); // before the first frame: start
        asm.cp_imm(abs_end);
        asm.ret_cond(Condition::Z); // on the last frame: stay there
        asm.jr_cond(Condition::NC, &label_reset); // past the last frame: start
        self.step_forward(&mut asm);
        asm.jr(&label_store);

        asm.label(&label_reset);
        asm.ld_a(abs_start);
        asm.label(&label_store);
        asm.ld_addr_def_a(&oam_tile_addr);

        asm.into_instrs()
    }

    /// Forward to the last frame, backward to the first one, and again (B10)
    ///
    /// The direction is only followed between the two ends: the first frame always goes
    /// forward and the last one backward, so the ends are shown once per turn and a
    /// direction left over from another animation does no harm.
    fn generate_ping_pong_func(&self, direction_var: &str) -> Vec<Instr> {
        let mut asm = Block::new();

        let oam_tile_addr = self.oam_tile_addr();
        let (abs_start, abs_end) = self.abs_frames();

        if abs_start == abs_end {
            // One frame: nowhere to go
            asm.ld_a(abs_start);
            asm.ld_addr_def_a(&oam_tile_addr);
            return asm.into_instrs();
        }

        let label = |stem: &str| format!(".{}_{}", stem, self.name);
        let (turn_forward, forward) = (label("turn_forward"), label("forward"));
        let (turn_backward, backward) = (label("turn_backward"), label("backward"));
        let (reset, store) = (label("reset"), label("store"));

        asm.ld_a_addr_def(&oam_tile_addr);
        asm.cp_imm(abs_start);
        asm.jr_cond(Condition::C, &reset); // before the first frame: start
        asm.jr_cond(Condition::Z, &turn_forward); // on the first frame: go forward
        asm.cp_imm(abs_end);
        asm.jr_cond(Condition::Z, &turn_backward); // on the last frame: go back
        asm.jr_cond(Condition::NC, &reset); // past the last frame: start
        // Between the two ends: keep going in the same direction
        asm.ld_a_addr_def(direction_var);
        asm.and(R8::A);
        asm.jr_cond(Condition::NZ, &backward);
        asm.jr(&forward);

        asm.label(&turn_forward);
        asm.ld_a(0);
        asm.ld_addr_def_a(direction_var);
        asm.label(&forward);
        asm.ld_a_addr_def(&oam_tile_addr);
        self.step_forward(&mut asm);
        asm.jr(&store);

        asm.label(&turn_backward);
        asm.ld_a(1);
        asm.ld_addr_def_a(direction_var);
        asm.label(&backward);
        asm.ld_a_addr_def(&oam_tile_addr);
        self.step_backward(&mut asm);
        asm.jr(&store);

        asm.label(&reset);
        asm.ld_a(abs_start);
        asm.label(&store);
        asm.ld_addr_def_a(&oam_tile_addr);

        asm.into_instrs()
    }
}
