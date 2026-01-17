//! Sprite management with automatic tile allocation and OAM handling

use std::collections::HashMap;

use super::tiles::{TileId, TileSource};
use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

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
    pub fn add(&mut self, name: &str, x: u8, y: u8, flags: u8) -> SpriteId {
        let tile_index = self.next_tile_index;
        let oam_index = self.next_oam_index;

        // We'll use a placeholder TileId - the actual tile ID will be set by RustBoy
        let tile_id = TileId(usize::MAX);

        let id = SpriteId(self.next_id);
        self.next_id += 1;
        self.next_oam_index += 1;
        self.next_tile_index += 1;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_sprite() {
        let mut sm = SpriteManager::new();

        let paddle = sm.add("Paddle", 16, 128, 0);
        let ball = sm.add("Ball", 32, 100, 0);

        assert_eq!(sm.get(paddle).unwrap().x, 16);
        assert_eq!(sm.get(paddle).unwrap().y, 128);
        assert_eq!(sm.get(ball).unwrap().x, 32);
        assert_eq!(sm.get(ball).unwrap().y, 100);
    }

    #[test]
    fn test_oam_indices() {
        let mut sm = SpriteManager::new();

        let paddle = sm.add("Paddle", 16, 128, 0);
        let ball = sm.add("Ball", 32, 100, 0);

        assert_eq!(sm.get(paddle).unwrap().oam_index, 0);
        assert_eq!(sm.get(ball).unwrap().oam_index, 1);
    }
}
