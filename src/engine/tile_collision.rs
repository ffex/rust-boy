//! Checks whether a screen pixel falls on one of a configured set of solid map tiles.

use crate::asm::{Block, Condition, Emittable, Instr, LabelAllocator, R8, R16Stack};

/// Solid background tile IDs used by a [`TileCollision`] check.
#[derive(Clone, Debug)]
pub struct TileCollision {
    solid_tiles: Vec<u8>,
}

impl TileCollision {
    /// Create a check for a set of solid background tile IDs.
    ///
    /// An empty set is valid and never collides.
    pub fn new(solid_tiles: impl IntoIterator<Item = u8>) -> Self {
        Self {
            solid_tiles: solid_tiles.into_iter().collect(),
        }
    }

    /// Check the tile under a screen pixel and run `body` when it is solid.
    ///
    /// `x` and `y` each leave their unsigned screen coordinate in `a`. The active camera
    /// scroll is added before looking up the current background map. Use while VRAM is
    /// accessible (the engine main loop runs during VBlank).
    ///
    /// When `body` runs, `a` is the tile ID and `hl` is its map address. The check clobbers
    /// `a`, `bc`, `d`, `hl` and flags; other registers are preserved except for changes made by
    /// the coordinate snippets and `body`.
    pub fn at_screen_pixel(
        &self,
        x: impl Emittable + 'static,
        y: impl Emittable + 'static,
        body: impl Emittable + 'static,
    ) -> TileCollisionCheck {
        TileCollisionCheck {
            x: Box::new(x),
            y: Box::new(y),
            body: Box::new(body),
            solid_tiles: self.solid_tiles.clone(),
        }
    }
}

/// An emitted screen-pixel background collision check.
pub struct TileCollisionCheck {
    x: Box<dyn Emittable>,
    y: Box<dyn Emittable>,
    body: Box<dyn Emittable>,
    solid_tiles: Vec<u8>,
}

impl Emittable for TileCollisionCheck {
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        let [hit, end] = labels.locals(["solid_tile_hit", "solid_tile_end"]);
        let mut code = Block::new();
        code.emit_all(self.x.emit(labels))
            .ld(R8::B, R8::A)
            .push(R16Stack::BC)
            .emit_all(self.y.emit(labels))
            .ld(R8::D, R8::A)
            .pop(R16Stack::BC)
            .ld(R8::C, R8::D)
            .call("GetTileByScreenPixel");
        for tile in &self.solid_tiles {
            code.cp_imm(*tile).jr_cond(Condition::Z, &hit);
        }
        code.jp(&end)
            .label(&hit)
            .emit_all(self.body.emit(labels))
            .label(&end);
        code.into_instrs()
    }
}
