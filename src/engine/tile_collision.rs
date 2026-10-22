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
    /// accessible: with the LCD off, or during VBlank. The engine main loop starts in
    /// VBlank, but earlier work can consume that interval. Schedule this check in a short
    /// `RustBoy::on_vblank` handler and keep the complete handler within VBlank, or otherwise
    /// ensure that VRAM remains accessible when the tile is read.
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
            points: vec![(Box::new(x), Box::new(y))],
            body: Box::new(body),
            solid_tiles: self.solid_tiles.clone(),
            when: When::Solid,
        }
    }

    /// Check the tiles under several screen pixels and run `body` only when none of them
    /// is solid (for example, the pixels a sprite is about to step on).
    ///
    /// Each point is a pair of snippets that leave the unsigned screen `x` and `y` in `a`,
    /// as for [`TileCollision::at_screen_pixel`], with the same VRAM-access rule. The
    /// checks stop at the first solid pixel. With no points or no solid tiles, `body`
    /// always runs and no tile is read. The check clobbers `a`, `bc`, `d`, `hl` and flags;
    /// other registers are preserved except for changes made by the snippets and `body`.
    pub fn when_free<X, Y>(
        &self,
        points: impl IntoIterator<Item = (X, Y)>,
        body: impl Emittable + 'static,
    ) -> TileCollisionCheck
    where
        X: Emittable + 'static,
        Y: Emittable + 'static,
    {
        TileCollisionCheck {
            points: points
                .into_iter()
                .map(|(x, y)| -> (Box<dyn Emittable>, Box<dyn Emittable>) {
                    (Box::new(x), Box::new(y))
                })
                .collect(),
            body: Box::new(body),
            solid_tiles: self.solid_tiles.clone(),
            when: When::Free,
        }
    }
}

/// When a [`TileCollisionCheck`] runs its body
enum When {
    /// The (single) pixel is on a solid tile
    Solid,
    /// No pixel is on a solid tile
    Free,
}

/// An emitted screen-pixel background collision check.
pub struct TileCollisionCheck {
    points: Vec<(Box<dyn Emittable>, Box<dyn Emittable>)>,
    body: Box<dyn Emittable>,
    solid_tiles: Vec<u8>,
    when: When,
}

impl TileCollisionCheck {
    /// Look up the tile under one point: `a` = its ID, `hl` = its map address
    fn lookup(
        code: &mut Block,
        point: &mut (Box<dyn Emittable>, Box<dyn Emittable>),
        labels: &LabelAllocator,
    ) {
        code.emit_all(point.0.emit(labels))
            .ld(R8::B, R8::A)
            .push(R16Stack::BC)
            .emit_all(point.1.emit(labels))
            .ld(R8::D, R8::A)
            .pop(R16Stack::BC)
            .ld(R8::C, R8::D)
            .call("GetTileByScreenPixel");
    }
}

/// A rectangle of a sprite, in pixels from the sprite's top-left corner: the part that
/// collides. For a top-down character, the feet: `Hitbox::new(0, 8, 16, 8)` is the lower
/// half of a 16x16 character, so its head can overlap the tiles above an obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hitbox {
    /// Left edge, from the sprite's left edge
    pub x: u8,
    /// Top edge, from the sprite's top edge
    pub y: u8,
    /// Width in pixels (at least 1)
    pub width: u8,
    /// Height in pixels (at least 1)
    pub height: u8,
}

impl Hitbox {
    /// A `width` x `height` rectangle at (`x`, `y`) from the sprite's top-left corner.
    ///
    /// # Panics
    /// If `width` or `height` is zero.
    #[track_caller]
    pub fn new(x: u8, y: u8, width: u8, height: u8) -> Self {
        assert!(width > 0, "Hitbox width must be nonzero");
        assert!(height > 0, "Hitbox height must be nonzero");
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl Emittable for TileCollisionCheck {
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        let mut code = Block::new();
        match self.when {
            When::Solid => {
                let [hit, end] = labels.locals(["solid_tile_hit", "solid_tile_end"]);
                for point in &mut self.points {
                    Self::lookup(&mut code, point, labels);
                }
                for tile in &self.solid_tiles {
                    code.cp_imm(*tile).jr_cond(Condition::Z, &hit);
                }
                code.jp(&end)
                    .label(&hit)
                    .emit_all(self.body.emit(labels))
                    .label(&end);
            }
            When::Free => {
                if self.solid_tiles.is_empty() || self.points.is_empty() {
                    code.emit_all(self.body.emit(labels));
                    return code.into_instrs();
                }
                let [blocked] = labels.locals(["solid_tile_blocked"]);
                for point in &mut self.points {
                    Self::lookup(&mut code, point, labels);
                    for tile in &self.solid_tiles {
                        code.cp_imm(*tile).jp_cond(Condition::Z, &blocked);
                    }
                }
                code.emit_all(self.body.emit(labels)).label(&blocked);
            }
        }
        code.into_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(mut check: TileCollisionCheck) -> Vec<String> {
        check
            .emit(&LabelAllocator::new())
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn at(value: u8) -> Vec<Instr> {
        let mut code = Block::new();
        code.ld_a(value);
        code.into_instrs()
    }

    #[test]
    fn when_free_reads_every_point_and_skips_the_body_on_a_solid_tile() {
        let lines = text(
            TileCollision::new([3, 4])
                .when_free([(at(1), at(2)), (at(5), at(6))], vec![Instr::Ret]),
        );
        let calls: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains("call GetTileByScreenPixel"))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(calls.len(), 2, "{lines:#?}");
        // After each lookup, every solid tile jumps past the body
        for call in calls {
            assert!(lines[call + 1].contains("cp") && lines[call + 1].contains('3'));
            assert!(lines[call + 2].starts_with("jp z, .solid_tile_blocked"));
            assert!(lines[call + 3].contains("cp") && lines[call + 3].contains('4'));
            assert!(lines[call + 4].starts_with("jp z, .solid_tile_blocked"));
        }
        let body = lines
            .iter()
            .position(|line| line == "ret")
            .expect("the body");
        assert!(lines[body + 1].starts_with(".solid_tile_blocked"));
        assert_eq!(body + 2, lines.len());
    }

    #[test]
    fn when_free_without_points_or_solid_tiles_is_the_body() {
        for check in [
            TileCollision::new([]).when_free([(at(1), at(2))], vec![Instr::Ret]),
            TileCollision::new([7])
                .when_free(Vec::<(Vec<Instr>, Vec<Instr>)>::new(), vec![Instr::Ret]),
        ] {
            assert_eq!(text(check), ["ret"]);
        }
    }
}
