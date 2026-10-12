//! Runtime axis-aligned bounding-box checks for byte-sized screen coordinates.

use crate::asm::{Block, Instr, R8};
use crate::stdlib::flow::{Emittable, If};

/// A rectangle whose coordinates are loaded into `a` by the supplied instruction snippets.
///
/// Coordinates are unsigned screen pixels. Width and height must be nonzero. Edge additions
/// wrap at 256, so this helper is for rectangles whose right and bottom edges stay within
/// the byte coordinate range.
#[derive(Clone)]
pub struct Aabb {
    x: Vec<Instr>,
    y: Vec<Instr>,
    width: u8,
    height: u8,
}

impl Aabb {
    /// Create a rectangle from snippets that each leave their coordinate in `a`.
    ///
    /// The snippets may read variables or sprite coordinates and must preserve the stack.
    ///
    /// # Panics
    /// If either dimension is zero.
    pub fn new(x: Vec<Instr>, y: Vec<Instr>, width: u8, height: u8) -> Self {
        assert!(width > 0, "Aabb width must be nonzero");
        assert!(height > 0, "Aabb height must be nonzero");
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn edge(code: &[Instr], size: u8) -> Block {
        let mut edge = Block::new();
        edge.emit_all(code.to_vec())
            .ld(R8::B, R8::A)
            .ld_a(size)
            .add(R8::B);
        edge
    }

    /// Run `body` when this rectangle and `other` overlap with positive area.
    ///
    /// Touching edges do not count as a collision. The comparison code uses unsigned
    /// coordinates and may clobber `a`, `b` and flags, as `If` does.
    pub fn overlaps(
        &self,
        other: &Aabb,
        body: impl Emittable + 'static,
    ) -> impl Emittable + 'static {
        If::gt(
            Self::edge(&self.x, self.width),
            other.x.clone(),
            If::gt(
                Self::edge(&other.x, other.width),
                self.x.clone(),
                If::gt(
                    Self::edge(&self.y, self.height),
                    other.y.clone(),
                    If::gt(Self::edge(&other.y, other.height), self.y.clone(), body),
                ),
            ),
        )
    }
}
