//! Sprite management with automatic tile allocation and OAM handling

use std::collections::HashMap;

use super::tiles::TileId;
use crate::{
    gb_asm::{Asm, Condition, Instr, Operand, Register},
    rust_boy::animations::Animation,
};

/// Unique identifier for a sprite instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteId(pub(crate) usize);

/// Internal sprite data
#[derive(Debug, Clone)]
pub(crate) struct SpriteData {
    pub name: String,
    pub tile_id: TileId,
    pub oam_index: u8,
    pub x: u8,
    pub y: u8,
    pub tile_index: u8,
    pub flags: u8,
    pub animations: Vec<Animation>,
}

/// Manages sprites with automatic tile allocation and OAM handling
#[derive(Debug)]
pub struct SpriteManager {
    sprites: HashMap<SpriteId, SpriteData>,
    next_id: usize,
    next_oam_index: u8,
    next_tile_index: u8,
}

impl SpriteManager {
    pub(crate) fn new() -> Self {
        Self {
            sprites: HashMap::new(),
            next_id: 0,
            next_oam_index: 0,
            next_tile_index: 0,
        }
    }

    /// Add a new sprite with tile data and initial position
    /// Returns both the sprite ID and tile ID for reference
    /// `tile_count` is the number of tiles this sprite uses (for proper tile index allocation)
    pub fn add(&mut self, name: &str, x: u8, y: u8, flags: u8, tile_count: u8) -> SpriteId {
        let tile_index = self.next_tile_index;
        let oam_index = self.next_oam_index;

        // We'll use a placeholder TileId - the actual tile ID will be set by RustBoy
        let tile_id = TileId(usize::MAX);

        let id = SpriteId(self.next_id);
        self.next_id += 1;
        self.next_oam_index += 1;
        self.next_tile_index += tile_count;

        self.sprites.insert(
            id,
            SpriteData {
                name: name.to_string(),
                tile_id,
                oam_index,
                x,
                y,
                tile_index,
                flags,
                animations: Vec::new(),
            },
        );

        id
    }

    /// Update the tile ID for a sprite (called internally by RustBoy)
    pub(crate) fn set_tile_id(&mut self, sprite_id: SpriteId, tile_id: TileId) {
        if let Some(sprite) = self.sprites.get_mut(&sprite_id) {
            sprite.tile_id = tile_id;
        }
    }

    /// Get sprite data
    pub fn get(&self, id: SpriteId) -> Option<&SpriteData> {
        self.sprites.get(&id)
    }

    /// Add an animation to a sprite
    /// - `name`: Animation name (used for label generation)
    /// - `start_frame`: Relative start frame index (e.g., 0)
    /// - `end_frame`: Relative end frame index (e.g., 6)
    /// - `anim_type`: Type of animation (Loop, PingPong, Once)
    pub fn add_animation(
        &mut self,
        sprite_id: SpriteId,
        name: &str,
        start_frame: u8,
        end_frame: u8,
        anim_type: super::animations::AnimationType,
    ) {
        if let Some(sprite) = self.sprites.get_mut(&sprite_id) {
            let animation = Animation {
                name: name.to_string(),
                oam_index: sprite.oam_index,
                base_tile: sprite.tile_index,
                start_frame,
                end_frame,
                anim_type,
            };
            sprite.animations.push(animation);
        }
    }

    /// Generate code to enable an animation
    /// Sets wAnim_[name]_Active to 1
    pub fn enable_animation(&self, name: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        let var_name = format!("wAnim_{}_Active", name);

        asm.ld_a(1);
        asm.ld_addr_def_a(&var_name);

        asm.get_main_instrs()
    }

    /// Generate code to disable an animation
    /// Sets wAnim_[name]_Active to 0
    pub fn disable_animation(&self, name: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        let var_name = format!("wAnim_{}_Active", name);

        asm.ld_a(0);
        asm.ld_addr_def_a(&var_name);

        asm.get_main_instrs()
    }

    /// Generate code to toggle an animation
    /// XORs wAnim_[name]_Active with 1
    pub fn toggle_animation(&self, name: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        let var_name = format!("wAnim_{}_Active", name);

        asm.ld_a_addr_def(&var_name);
        asm.xor(Operand::Reg(Register::A), Operand::Imm(1));
        asm.ld_addr_def_a(&var_name);

        asm.get_main_instrs()
    }

    /// Generate OAM initialization code
    pub(crate) fn generate_init_code(&self) -> Vec<Instr> {
        let mut asm = Asm::new();

        // Initialize OAM
        asm.ld_a(0);
        asm.ld_b(160);
        asm.ld_hl_label("_OAMRAM");

        // Clear OAM loop
        asm.label("ClearOam");
        asm.ld_hli_label("a");
        asm.dec_label("b");
        asm.jp_cond(Condition::NZ, "ClearOam");

        // Draw all sprites to OAM (sorted by oam_index to ensure correct order)
        asm.ld_hl_label("_OAMRAM");
        let mut sorted_sprites: Vec<_> = self.sprites.values().collect();
        sorted_sprites.sort_by_key(|s| s.oam_index);
        for sprite in sorted_sprites {
            // Y position (add 16 for screen offset)
            asm.ld_a(sprite.y + 16);
            asm.ld_hli_label("a");

            // X position (add 8 for screen offset)
            asm.ld_a(sprite.x + 8);
            asm.ld_hli_label("a");

            // Tile index
            asm.ld_a(sprite.tile_index);
            asm.ld_hli_label("a");

            // Flags
            asm.ld_a(sprite.flags);
            asm.ld_hli_label("a");
        }

        asm.get_main_instrs()
    }

    /// Generate movement code for a specific sprite
    pub fn move_x_var(&self, id: SpriteId, var_name: &str) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4 + 1;

            asm.ld_a_addr_def(var_name);
            asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));
            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));
            asm.add(Operand::Reg(Register::A), Operand::Reg(Register::B));
            asm.ld_addr_def_a(&format!("_OAMRAM+{}", oam_offset));

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Generate movement code for Y axis with variable
    pub fn move_y_var(&self, id: SpriteId, var_name: &str) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4;

            asm.ld_a_addr_def(var_name);
            asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));
            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));
            asm.add(Operand::Reg(Register::A), Operand::Reg(Register::B));
            asm.ld_addr_def_a(&format!("_OAMRAM+{}", oam_offset));

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Move sprite left with limit
    pub fn move_left_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4 + 1;
            let jump_label = format!("Sprite{}LeftLimitEnd", sprite.oam_index);

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));
            asm.sub(Operand::Reg(Register::A), Operand::Imm(distance));
            asm.cp(Operand::Imm(limit));
            asm.jp_cond(Condition::Z, &jump_label);
            asm.ld_addr_def_a(&format!("_OAMRAM+{}", oam_offset));
            asm.label(&jump_label);

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Move sprite right with limit
    pub fn move_right_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4 + 1;
            let jump_label = format!("Sprite{}RightLimitEnd", sprite.oam_index);

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));
            asm.add(Operand::Reg(Register::A), Operand::Imm(distance));
            asm.cp(Operand::Imm(limit));
            asm.jp_cond(Condition::Z, &jump_label);
            asm.ld_addr_def_a(&format!("_OAMRAM+{}", oam_offset));
            asm.label(&jump_label);

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Move sprite up with limit
    pub fn move_up_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4;
            let jump_label = format!("Sprite{}UpLimitEnd", sprite.oam_index);

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));
            asm.sub(Operand::Reg(Register::A), Operand::Imm(distance));
            asm.cp(Operand::Imm(limit));
            asm.jp_cond(Condition::Z, &jump_label);
            asm.ld_addr_def_a(&format!("_OAMRAM+{}", oam_offset));
            asm.label(&jump_label);

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Move sprite down with limit
    pub fn move_down_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4;
            let jump_label = format!("Sprite{}DownLimitEnd", sprite.oam_index);

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));
            asm.add(Operand::Reg(Register::A), Operand::Imm(distance));
            asm.cp(Operand::Imm(limit));
            asm.jp_cond(Condition::Z, &jump_label);
            asm.ld_addr_def_a(&format!("_OAMRAM+{}", oam_offset));
            asm.label(&jump_label);

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Get sprite pivot point (for collision detection)
    pub fn get_pivot(&self, id: SpriteId, x_offset: i16, y_offset: i16) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_y_offset = sprite.oam_index * 4;
            let oam_x_offset = sprite.oam_index * 4 + 1;

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_y_offset));
            asm.sub(
                Operand::Reg(Register::A),
                Operand::Imm(u8::try_from(16i16 + y_offset).unwrap_or(0)),
            );
            asm.ld(Operand::Reg(Register::C), Operand::Reg(Register::A));
            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_x_offset));
            asm.sub(
                Operand::Reg(Register::A),
                Operand::Imm(u8::try_from(8i16 + x_offset).unwrap_or(0)),
            );
            asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Get sprite Y position
    pub fn get_y(&self, id: SpriteId) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4;

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Get sprite X position
    pub fn get_x(&self, id: SpriteId) -> Vec<Instr> {
        if let Some(sprite) = self.sprites.get(&id) {
            let mut asm = Asm::new();
            let oam_offset = sprite.oam_index * 4 + 1;

            asm.ld_a_addr_def(&format!("_OAMRAM+{}", oam_offset));

            asm.get_main_instrs()
        } else {
            Vec::new()
        }
    }

    /// Check if any sprites have been added
    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    /// Check if any animations have been added to any sprite
    pub fn has_animations(&self) -> bool {
        self.sprites.values().any(|s| !s.animations.is_empty())
    }

    /// Generate animation functions for all sprites with animations
    /// Returns a list of (function_name, function_body) pairs
    pub(crate) fn generate_animation_functions(&self) -> Vec<(String, Vec<Instr>)> {
        let mut functions = Vec::new();

        for sprite in self.sprites.values() {
            for animation in &sprite.animations {
                let mut asm = Asm::new();
                let func_name = format!("Anim_{}", animation.name);

                asm.label(&func_name);
                asm.emit_all(animation.generate_loop_func());
                asm.ret();

                functions.push((func_name, asm.get_main_instrs()));
            }
        }

        functions
    }

    /// Generate the main loop animation code with frame-based timing
    /// Uses wFrameCounter and wAnimDelay variables for non-blocking animation updates
    /// - Increments frame counter each frame
    /// - Only updates animations when counter >= delay
    /// - Resets counter after animation update
    /// - Checks wAnim_[name]_Active flag before calling each animation
    pub(crate) fn generate_animation_calls(&self, delay_value: u8) -> Vec<Instr> {
        let mut asm = Asm::new();

        // Increment frame counter
        asm.ld_a_addr_def("wFrameCounter");
        asm.inc(Operand::Reg(Register::A));
        asm.ld_addr_def_a("wFrameCounter");

        // Compare with delay value
        asm.cp_imm(delay_value);
        asm.jr_cond(Condition::C, "AnimEnd"); // if counter < delay, skip animations

        // Reset frame counter
        asm.ld_a(0);
        asm.ld_addr_def_a("wFrameCounter");

        // Call all animation functions (only if enabled)
        for sprite in self.sprites.values() {
            for animation in &sprite.animations {
                let func_name = format!("Anim_{}", animation.name);
                let enabled_var = format!("wAnim_{}_Active", animation.name);
                let skip_label = format!(".skip_{}", animation.name);

                // Check if animation is active
                asm.ld_a_addr_def(&enabled_var);
                asm.cp_imm(0);
                asm.jr_cond(Condition::Z, &skip_label); // if disabled, skip

                asm.call(&func_name);

                asm.label(&skip_label);
            }
        }

        asm.label("AnimEnd");

        asm.get_main_instrs()
    }

    /// Get list of animation variable names (for auto-creating variables)
    /// Returns (name, initial_value) pairs
    pub(crate) fn get_animation_variables(&self) -> Vec<(String, u8)> {
        let mut vars = Vec::new();

        for sprite in self.sprites.values() {
            for animation in &sprite.animations {
                let var_name = format!("wAnim_{}_Active", animation.name);
                // Default: enabled (1)
                vars.push((var_name, 1));
            }
        }

        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_sprite() {
        let mut sm = SpriteManager::new();

        let paddle = sm.add("Paddle", 16, 128, 0, 1);
        let ball = sm.add("Ball", 32, 100, 0, 1);

        assert_eq!(sm.get(paddle).unwrap().x, 16);
        assert_eq!(sm.get(paddle).unwrap().y, 128);
        assert_eq!(sm.get(ball).unwrap().x, 32);
        assert_eq!(sm.get(ball).unwrap().y, 100);
    }

    #[test]
    fn test_oam_indices() {
        let mut sm = SpriteManager::new();

        let paddle = sm.add("Paddle", 16, 128, 0, 1);
        let ball = sm.add("Ball", 32, 100, 0, 1);

        assert_eq!(sm.get(paddle).unwrap().oam_index, 0);
        assert_eq!(sm.get(ball).unwrap().oam_index, 1);
    }
}
