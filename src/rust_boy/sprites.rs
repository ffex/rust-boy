//! Sprite management with automatic tile allocation and OAM handling

use std::collections::BTreeMap;

use super::memory::{MemoryAllocator, MemoryRegion};
use super::tiles::TileId;
use crate::{
    gb_asm::{Block, Condition, Expr, Instr, LabelAllocator, R8, is_identifier},
    gb_std::graphics::sprites::{
        MoveDir, Sprite, draw_sprites, move_coord_limit, move_coord_var, oam_address, pivot,
    },
    hw,
    rust_boy::animations::Animation,
};

/// Panics unless `name`, given by the user for a `kind` ("sprite", "animation", ...), is
/// a valid RGBDS identifier: it becomes part of assembly labels (B25)
pub(crate) fn check_name(kind: &str, name: &str) {
    if !is_identifier(name) {
        panic!(
            "invalid {} name \"{}\": it becomes part of assembly labels, so it must start \
             with a letter or '_' and contain only letters, digits, '_', '#', '$' or '@'",
            kind, name
        );
    }
}

/// Panics unless `name`, given by the user for a `kind` ("tile", ...), can be a global
/// label of its own: a valid RGBDS identifier that is not a register or keyword name (the
/// generated code names it as a symbol, `ld de, name`)
#[track_caller]
pub(crate) fn check_label(kind: &str, name: &str) {
    if !is_identifier(name) || crate::gb_asm::expr::check_symbol(name).is_err() {
        panic!(
            "invalid {} name \"{}\": it becomes an assembly label, so it must start with a \
             letter or '_', contain only letters, digits, '_', '#', '$' or '@', and not be a \
             register or keyword name",
            kind, name
        );
    }
}

/// Label of the function that plays animation `animation` of sprite `sprite`; the
/// sprite name keeps two sprites with an animation of the same name apart (B25)
fn animation_label(sprite: &str, animation: &str) -> String {
    format!("Anim_{}_{}", sprite, animation)
}

/// WRAM variable holding the direction of a sprite's `PingPong` animation
/// (0 = forward, 1 = backward); only sprites with such an animation of two frames or
/// more have one (B10)
fn direction_var(sprite: &str) -> String {
    format!("wAnim_{}_Dir", sprite)
}

/// Unique identifier for a sprite instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpriteId(pub(crate) usize);

/// Unique identifier for a composite sprite (group of sprites that move together)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompositeSpriteId(pub(crate) usize);

/// A composite sprite made of multiple hardware sprites that move together
#[derive(Debug, Clone)]
pub(crate) struct CompositeSpriteData {
    /// The composite's name; its sprites are named after it (`{name}_left`, ...)
    pub name: String,
    /// The individual sprite IDs that make up this composite
    pub sprites: Vec<SpriteId>,
    /// Animation names for this composite (every sprite of it has an animation of that name)
    pub animation_names: Vec<String>,
}

/// No animation active (255 = disabled)
pub const ANIM_DISABLED: u8 = 255;

/// Size of every hardware sprite (LCDC bit 2); one setting for all sprites.
///
/// In 8x16 mode a sprite shows two stacked tiles: the hardware ignores bit 0 of its
/// tile index and draws tile `n & $FE` on top and `n | 1` below. So every sprite has an
/// even number of tiles, starts on an even tile index, and one animation frame is two tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpriteSize {
    /// 8x8 pixels, one tile per sprite (the hardware default)
    #[default]
    Size8x8,
    /// 8x16 pixels, two tiles per sprite: an even tile on top, the next one below
    Size8x16,
}

impl SpriteSize {
    /// Tiles one sprite (one animation frame) uses: 1 for 8x8, 2 for 8x16
    pub fn tiles_per_sprite(self) -> u8 {
        match self {
            SpriteSize::Size8x8 => 1,
            SpriteSize::Size8x16 => 2,
        }
    }

    /// The `hardware.inc` LCDC flag that selects this size
    pub(crate) fn lcdc_flag(self) -> hw::Symbol<u8> {
        match self {
            SpriteSize::Size8x8 => hw::LCDCF_OBJ8,
            SpriteSize::Size8x16 => hw::LCDCF_OBJ16,
        }
    }
}

/// The sprite tiles a sprite shows, as the tile manager placed them in VRAM
#[derive(Debug, Clone, Copy)]
pub(crate) struct SpriteTiles {
    /// The tiles in the tile manager
    pub id: TileId,
    /// Tile index of the first one (its VRAM address is `$8000 + first * 16`)
    pub first: u8,
    /// How many tiles (1 to 256)
    pub count: u16,
}

/// Internal sprite data
#[derive(Debug, Clone)]
pub(crate) struct SpriteData {
    pub name: String,
    #[cfg_attr(not(test), allow(dead_code))] // the tests check it against tile_index
    pub tile_id: TileId,
    pub oam_index: u8,
    pub x: u8,
    pub y: u8,
    /// Tile index of the sprite's first tile, from the tile manager (B18)
    pub tile_index: u8,
    /// Number of tiles of the sprite (its frames are among them)
    pub tile_count: u16,
    pub flags: u8,
    pub animations: Vec<Animation>,
    pub initial_animation: u8, // Index of initially active animation, or ANIM_DISABLED
}

impl SpriteData {
    /// The WRAM variable holding the index of the sprite's current animation
    /// (`ANIM_DISABLED` for none); only a sprite with animations has one
    fn current_var(&self) -> String {
        format!("wAnim_{}_Current", self.name)
    }

    /// The index of the animation `name` (B20)
    ///
    /// # Panics
    /// If the sprite has no animation of that name.
    fn animation_index(&self, name: &str) -> u8 {
        match self.animations.iter().position(|anim| anim.name == name) {
            Some(index) => index as u8,
            None => {
                let names: Vec<&str> = self.animations.iter().map(|a| a.name.as_str()).collect();
                let has = if names.is_empty() {
                    "it has no animations".to_string()
                } else {
                    format!("its animations: {}", names.join(", "))
                };
                panic!(
                    "sprite \"{}\" has no animation \"{}\" ({})",
                    self.name, name, has
                );
            }
        }
    }

    /// Panics unless the sprite has the animation `index` (B20)
    fn check_animation_index(&self, index: u8) {
        let count = self.animations.len();
        if usize::from(index) >= count {
            let has = match count {
                0 => "it has no animations".to_string(),
                _ => format!("it has {}: 0 to {}", count, count - 1),
            };
            panic!(
                "sprite \"{}\" has no animation {} ({})",
                self.name, index, has
            );
        }
    }

    /// Panics unless the sprite has animations, so its animation variable exists
    fn check_has_animations(&self, what: &str) {
        if self.animations.is_empty() {
            panic!(
                "{}: sprite \"{}\" has no animations, so it has no animation variable ({} is \
                 not created); add an animation first",
                what,
                self.name,
                self.current_var()
            );
        }
    }
}

/// Panics: `id` is not a sprite of this manager (B20)
fn unknown_sprite(id: SpriteId) -> ! {
    panic!(
        "unknown sprite id {}: no sprite of this program has it (sprite ids come from \
         RustBoy::add_sprite, for this RustBoy)",
        id.0
    );
}

/// Panics: `id` is not a composite sprite of this manager (B20)
fn unknown_composite(id: CompositeSpriteId) -> ! {
    panic!(
        "unknown composite sprite id {}: no composite sprite of this program has it \
         (composite ids come from RustBoy::add_sprite_16x16, for this RustBoy)",
        id.0
    );
}

/// The axis a sprite moves along
#[derive(Debug, Clone, Copy)]
enum Axis {
    X,
    Y,
}

impl Axis {
    /// The sprite's initial position on this axis (screen coordinates)
    fn position(self, sprite: &SpriteData) -> u8 {
        match self {
            Axis::X => sprite.x,
            Axis::Y => sprite.y,
        }
    }

    /// The address of the sprite's coordinate in OAM: Y is byte 0 of its entry, X byte 1
    fn oam_address(self, sprite: &SpriteData) -> Expr {
        let byte = match self {
            Axis::Y => hw::OAMA_Y,
            Axis::X => hw::OAMA_X,
        };
        oam_address(sprite.oam_index, byte)
    }
}

/// Manages sprites with automatic tile allocation and OAM handling
#[derive(Debug)]
pub struct SpriteManager {
    /// Sprites by id; ids are sequential, so iteration follows creation order
    sprites: BTreeMap<SpriteId, SpriteData>,
    composite_sprites: BTreeMap<CompositeSpriteId, CompositeSpriteData>,
    next_id: usize,
    next_composite_id: usize,
    /// OAM entries: 40 sprites of 4 bytes (B17)
    oam: MemoryAllocator,
    size: SpriteSize,
    /// Numbers the local labels of the moves; shared with the rest of the program (B7)
    labels: LabelAllocator,
}

impl SpriteManager {
    /// A manager whose moves take their labels from `labels`
    pub(crate) fn new(labels: LabelAllocator) -> Self {
        Self {
            sprites: BTreeMap::new(),
            composite_sprites: BTreeMap::new(),
            next_id: 0,
            next_composite_id: 0,
            oam: MemoryAllocator::new(MemoryRegion::Oam),
            size: SpriteSize::default(),
            labels,
        }
    }

    /// Size of every sprite (see `RustBoy::set_sprite_size`)
    pub(crate) fn size(&self) -> SpriteSize {
        self.size
    }

    /// Set the size of every sprite (see `RustBoy::set_sprite_size`)
    ///
    /// # Panics
    /// If sprites were already added with another size: their tile indices depend on it.
    pub(crate) fn set_size(&mut self, size: SpriteSize) {
        if size != self.size && !self.sprites.is_empty() {
            panic!(
                "set_sprite_size({:?}) must be called before the first sprite is added \
                 (the sprites already added use {:?})",
                size, self.size
            );
        }
        self.size = size;
    }

    /// Add a new sprite, shown with the sprite tiles `tiles` (already in the tile manager)
    /// at its initial position; `RustBoy::add_sprite` is the public way, it adds the
    /// tiles and the sprite together
    ///
    /// The tile index comes from where the tile manager put the tiles in VRAM: the tile
    /// manager is the one source of the sprite tile indices (B18).
    ///
    /// # Panics
    /// - If `name` is not a valid RGBDS identifier, or another sprite has it: the name
    ///   becomes part of labels (the sprite's tiles, its animations).
    /// - In 8x16 mode, if the sprite has an odd number of tiles (the bottom half of the
    ///   last frame would be the next tile in VRAM, which is not this sprite's), or starts
    ///   on an odd tile (the hardware ignores bit 0 of the index).
    /// - If `x` is above 247 or `y` above 239: OAM X = x + 8 and OAM Y = y + 16 are bytes.
    /// - If OAM is full: it holds 40 sprites.
    pub(crate) fn add(
        &mut self,
        name: &str,
        tiles: SpriteTiles,
        x: u8,
        y: u8,
        flags: u8,
    ) -> SpriteId {
        check_name("sprite", name);
        if self.sprites.values().any(|sprite| sprite.name == name) {
            panic!(
                "sprite name \"{}\" is already used: sprite names become labels (tiles, \
                 animations), so each sprite needs its own",
                name
            );
        }
        if self.size == SpriteSize::Size8x16 && tiles.count % 2 != 0 {
            panic!(
                "sprite \"{}\" has {} tiles, but in 8x16 mode every sprite frame is two tiles \
                 (top and bottom), so the tile count must be even",
                name, tiles.count
            );
        }
        if self.size == SpriteSize::Size8x16 && tiles.first % 2 != 0 {
            panic!(
                "sprite \"{}\" would start on tile {}, an odd one, but in 8x16 mode the \
                 hardware ignores bit 0 of the tile index: the tiles added before it with \
                 `tiles.add_sprite` must be an even number",
                name, tiles.first
            );
        }
        // OAM X = x + 8 and OAM Y = y + 16 are bytes (B17)
        for (axis, value, offset) in [("x", x, hw::OAM_X_OFFSET), ("y", y, hw::OAM_Y_OFFSET)] {
            if value.checked_add(offset).is_none() {
                panic!(
                    "sprite \"{}\": {} = {} is too large: its OAM coordinate, {} + {}, must fit \
                     in a byte, so {} is at most {}",
                    name,
                    axis,
                    value,
                    axis,
                    offset,
                    axis,
                    u8::MAX - offset
                );
            }
        }
        let entry = self
            .oam
            .allocate(hw::OAM_ENTRY_SIZE.value.into())
            .unwrap_or_else(|| {
                panic!(
                    "sprite \"{}\" does not fit in OAM, which holds {} sprites",
                    name,
                    hw::OAM_COUNT.value
                )
            });
        let oam_index = ((entry - hw::OAMRAM.value) / u16::from(hw::OAM_ENTRY_SIZE.value)) as u8;

        let id = SpriteId(self.next_id);
        self.next_id += 1;

        self.sprites.insert(
            id,
            SpriteData {
                name: name.to_string(),
                tile_id: tiles.id,
                oam_index,
                x,
                y,
                tile_index: tiles.first,
                tile_count: tiles.count,
                flags,
                animations: Vec::new(),
                initial_animation: ANIM_DISABLED, // No animation by default
            },
        );

        id
    }

    /// Add a sprite whose tiles follow those of the sprites already added (tests that
    /// use a sprite manager without a tile manager)
    #[cfg(test)]
    pub(crate) fn add_for_test(
        &mut self,
        name: &str,
        x: u8,
        y: u8,
        flags: u8,
        tile_count: u8,
    ) -> SpriteId {
        let first = self
            .sprites
            .values()
            .map(|sprite| u16::from(sprite.tile_index) + sprite.tile_count)
            .max()
            .unwrap_or(0);
        let tiles = SpriteTiles {
            id: TileId(usize::MAX),
            first: u8::try_from(first).expect("too many tiles for a test"),
            count: tile_count.into(),
        };
        self.add(name, tiles, x, y, flags)
    }

    /// The tile right after the last tile of sprite `id`, where more tiles must go to
    /// belong to it (see `RustBoy::add_sprite_tiles`), and the sprite's name
    pub(crate) fn tile_after(&self, id: SpriteId) -> (u16, &str) {
        let sprite = self.sprite(id);
        (
            u16::from(sprite.tile_index) + sprite.tile_count,
            sprite.name.as_str(),
        )
    }

    /// Add `count` tiles to the end of sprite `id`'s tiles (`RustBoy::add_sprite_tiles`
    /// has put them right after them in VRAM)
    ///
    /// # Panics
    /// In 8x16 mode, if `count` is odd: frames are two tiles.
    pub(crate) fn extend_tiles(&mut self, id: SpriteId, count: u16) {
        let size = self.size;
        let sprite = self.sprite_mut(id);
        if size == SpriteSize::Size8x16 && count % 2 != 0 {
            panic!(
                "add_sprite_tiles(sprite \"{}\"): {} tiles, but in 8x16 mode every frame is two \
                 tiles, so the tile count must be even",
                sprite.name, count
            );
        }
        sprite.tile_count += count;
    }

    /// Get sprite data (used by the tests)
    #[cfg(test)]
    pub(crate) fn get(&self, id: SpriteId) -> Option<&SpriteData> {
        self.sprites.get(&id)
    }

    /// The sprite `id`; panics if this manager has none (B20)
    fn sprite(&self, id: SpriteId) -> &SpriteData {
        self.sprites.get(&id).unwrap_or_else(|| unknown_sprite(id))
    }

    /// The sprite `id`, to change it; panics if this manager has none (B20)
    fn sprite_mut(&mut self, id: SpriteId) -> &mut SpriteData {
        self.sprites
            .get_mut(&id)
            .unwrap_or_else(|| unknown_sprite(id))
    }

    /// The composite sprite `id`; panics if this manager has none (B20)
    fn composite(&self, id: CompositeSpriteId) -> &CompositeSpriteData {
        self.composite_sprites
            .get(&id)
            .unwrap_or_else(|| unknown_composite(id))
    }

    /// Add an animation to a sprite; a frame is one sprite's worth of tiles
    /// (1 tile in 8x8 mode, 2 tiles in 8x16 mode)
    /// - `name`: Animation name, unique for this sprite; the animation is played by the
    ///   function `Anim_{sprite name}_{name}`
    /// - `start_frame`: Relative start frame index (e.g., 0)
    /// - `end_frame`: Relative end frame index (e.g., 6)
    /// - `anim_type`: Type of animation (Loop, PingPong, Once)
    ///
    /// Returns the animation index within this sprite
    ///
    /// # Panics
    /// See [`SpriteManager::add_animation_with_step`].
    pub fn add_animation(
        &mut self,
        sprite_id: SpriteId,
        name: &str,
        start_frame: u8,
        end_frame: u8,
        anim_type: super::animations::AnimationType,
    ) -> u8 {
        let frame_step = self.size.tiles_per_sprite();
        self.add_animation_with_step(
            sprite_id,
            name,
            start_frame,
            end_frame,
            anim_type,
            frame_step,
        )
    }

    /// Add an animation to a sprite with custom frame step (tiles between two frames)
    /// Returns the animation index within this sprite
    ///
    /// # Panics
    /// - If there is no sprite `sprite_id`.
    /// - If `name` is not a valid RGBDS identifier, or the sprite already has an animation
    ///   of that name: it becomes part of labels (`Anim_{sprite name}_{name}`).
    /// - If that label is another animation's: "Big_Coin" + "Spin" and "Big" + "Coin_Spin"
    ///   are both `Anim_Big_Coin_Spin`.
    /// - In 8x16 mode, if `frame_step` is odd: every frame must start on an even tile.
    /// - If the sprite already has 255 animations: the index is a `u8`, and 255 is
    ///   [`ANIM_DISABLED`].
    /// - If `start_frame` is after `end_frame`, `frame_step` is 0, or a frame is not among
    ///   the sprite's tiles: frame `f` shows the tiles from `f * frame_step` on (one tile
    ///   in 8x8 mode, two in 8x16 mode), counted from the sprite's first tile, and they
    ///   must all be the sprite's (B17).
    pub fn add_animation_with_step(
        &mut self,
        sprite_id: SpriteId,
        name: &str,
        start_frame: u8,
        end_frame: u8,
        anim_type: super::animations::AnimationType,
        frame_step: u8,
    ) -> u8 {
        check_name("animation", name);
        if self.size == SpriteSize::Size8x16 && frame_step % 2 != 0 {
            panic!(
                "animation \"{}\": frame_step {} is odd, but in 8x16 mode every frame is two \
                 tiles, so frame_step must be even",
                name, frame_step
            );
        }
        let sprite = self.sprite(sprite_id);
        self.check_animation_label(sprite, name);
        self.check_frames(sprite, name, start_frame, end_frame, frame_step);
        if sprite.animations.len() >= usize::from(ANIM_DISABLED) {
            panic!(
                "sprite \"{}\" already has 255 animations, the most a sprite can have: \
                 animation indices are 0 to 254, and 255 is ANIM_DISABLED",
                sprite.name
            );
        }
        let sprite = self.sprite_mut(sprite_id);
        let index = sprite.animations.len() as u8;
        let animation = Animation {
            name: name.to_string(),
            oam_index: sprite.oam_index,
            base_tile: sprite.tile_index,
            start_frame,
            end_frame,
            anim_type,
            index,
            frame_step,
        };
        sprite.animations.push(animation);
        index
    }

    /// Panics unless the frames `start..=end`, `step` tiles apart, are all among the
    /// tiles of `sprite` (B17): a frame past them showed another sprite's tiles, and past
    /// tile 255 the frame arithmetic overflowed
    fn check_frames(&self, sprite: &SpriteData, name: &str, start: u8, end: u8, step: u8) {
        let what = format!("animation \"{}\" of sprite \"{}\"", name, sprite.name);
        if start > end {
            panic!("{}: start_frame {} is after end_frame {}", what, start, end);
        }
        if step == 0 {
            panic!("{}: frame_step must be at least 1", what);
        }
        // The tiles of the last frame, from the sprite's first tile
        let per_frame = u16::from(self.size.tiles_per_sprite());
        let last_tile = u16::from(end) * u16::from(step) + per_frame - 1;
        if last_tile >= sprite.tile_count {
            panic!(
                "{}: frame {} would show tile {} of the sprite, but it has {} tiles (frame f \
                 starts on tile f * {}, and is {} tile(s)); to give the sprite more tiles from \
                 another source (another .2bpp file), use RustBoy::add_sprite_tiles before \
                 adding the animation",
                what, end, last_tile, sprite.tile_count, step, per_frame
            );
        }
    }

    /// Panics if animation `name` of `sprite` would get the label of an existing animation
    fn check_animation_label(&self, sprite: &SpriteData, name: &str) {
        if sprite.animations.iter().any(|anim| anim.name == name) {
            panic!(
                "sprite \"{}\" already has an animation \"{}\"",
                sprite.name, name
            );
        }
        let label = animation_label(&sprite.name, name);
        for other in self.sprites.values() {
            for anim in &other.animations {
                if animation_label(&other.name, &anim.name) == label {
                    panic!(
                        "animation \"{}\" of sprite \"{}\" would get the label {}, which \
                         animation \"{}\" of sprite \"{}\" already has: rename one of them",
                        name, sprite.name, label, anim.name, other.name
                    );
                }
            }
        }
    }

    /// Set the initial animation for a sprite by animation index
    /// Use ANIM_DISABLED (255) to start with no animation
    ///
    /// # Panics
    /// If there is no sprite `sprite_id`, or it has no animation `animation_index` (and
    /// it is not `ANIM_DISABLED`): add the animations first.
    pub fn set_initial_animation(&mut self, sprite_id: SpriteId, animation_index: u8) {
        let sprite = self.sprite_mut(sprite_id);
        if animation_index != ANIM_DISABLED {
            sprite.check_animation_index(animation_index);
        }
        sprite.initial_animation = animation_index;
    }

    /// Set the initial animation for a sprite by animation name
    ///
    /// # Panics
    /// If there is no sprite `sprite_id`, or it has no animation `name`.
    pub fn set_initial_animation_by_name(&mut self, sprite_id: SpriteId, name: &str) {
        let sprite = self.sprite_mut(sprite_id);
        sprite.initial_animation = sprite.animation_index(name);
    }

    /// Generate code to enable an animation by index for a sprite
    /// Sets wAnim_[sprite_name]_Current to the animation index
    ///
    /// # Panics
    /// If there is no sprite `sprite_id`, or it has no animation `animation_index`
    /// (to stop the animation, use [`SpriteManager::disable_animation`]).
    pub fn enable_animation(&self, sprite_id: SpriteId, animation_index: u8) -> Vec<Instr> {
        let sprite = self.sprite(sprite_id);
        if animation_index == ANIM_DISABLED {
            panic!(
                "enable_animation(sprite \"{}\", ANIM_DISABLED): to stop the animation, use \
                 disable_animation",
                sprite.name
            );
        }
        sprite.check_animation_index(animation_index);
        let mut asm = Block::new();
        asm.ld_a(animation_index);
        asm.ld_addr_def_a(sprite.current_var());
        asm.into_instrs()
    }

    /// Generate code to enable an animation by name for a sprite
    ///
    /// # Panics
    /// If there is no sprite `sprite_id`, or it has no animation `name`.
    pub fn enable_animation_by_name(&self, sprite_id: SpriteId, name: &str) -> Vec<Instr> {
        let index = self.sprite(sprite_id).animation_index(name);
        self.enable_animation(sprite_id, index)
    }

    /// Generate code to disable all animations for a sprite
    /// Sets wAnim_[sprite_name]_Current to ANIM_DISABLED (255)
    ///
    /// # Panics
    /// If there is no sprite `sprite_id`, or it has no animations (it has no animation
    /// variable to set).
    pub fn disable_animation(&self, sprite_id: SpriteId) -> Vec<Instr> {
        let sprite = self.sprite(sprite_id);
        sprite.check_has_animations("disable_animation");
        let mut asm = Block::new();
        asm.ld_a(ANIM_DISABLED);
        asm.ld_addr_def_a(sprite.current_var());
        asm.into_instrs()
    }

    // ==================== Composite Sprite Methods ====================

    /// Create a composite sprite from multiple sprite IDs
    /// This groups sprites together so they can be moved/animated as one unit
    pub(crate) fn create_composite(
        &mut self,
        name: &str,
        sprites: Vec<SpriteId>,
    ) -> CompositeSpriteId {
        let id = CompositeSpriteId(self.next_composite_id);
        self.next_composite_id += 1;

        self.composite_sprites.insert(
            id,
            CompositeSpriteData {
                name: name.to_string(),
                sprites,
                animation_names: Vec::new(),
            },
        );

        id
    }

    /// Get the sprite IDs that make up a composite sprite (`None` for an unknown id)
    pub fn get_composite_sprites(&self, id: CompositeSpriteId) -> Option<&Vec<SpriteId>> {
        self.composite_sprites.get(&id).map(|c| &c.sprites)
    }

    /// Add an animation to all sprites in a composite
    /// Each sprite gets an animation called `name`, played by `Anim_{sprite name}_{name}`
    /// (e.g. `Anim_player_left_Walk` and `Anim_player_right_Walk`)
    /// Composites are made of 8x16 sprites, so a frame is two tiles
    /// Returns the animation index (same for all sprites in the composite)
    ///
    /// # Panics
    /// If there is no composite `composite_id`, if `name` is not a valid RGBDS
    /// identifier, or the composite already has an animation of that name (see
    /// [`SpriteManager::add_animation_with_step`]).
    pub fn add_composite_animation(
        &mut self,
        composite_id: CompositeSpriteId,
        name: &str,
        start_frame: u8,
        end_frame: u8,
        anim_type: super::animations::AnimationType,
    ) -> u8 {
        let mut anim_index = 0u8;
        check_name("animation", name);

        let composite = self.composite(composite_id);
        if composite.animation_names.iter().any(|anim| anim == name) {
            panic!(
                "composite sprite \"{}\" already has an animation \"{}\"",
                composite.name, name
            );
        }
        let sprite_ids = composite.sprites.clone();
        if let Some(composite) = self.composite_sprites.get_mut(&composite_id) {
            composite.animation_names.push(name.to_string());
        }
        for sprite_id in sprite_ids {
            anim_index =
                self.add_animation(sprite_id, name, start_frame, end_frame, anim_type.clone());
        }

        anim_index
    }

    /// Set the initial animation for all sprites in a composite by animation index
    ///
    /// # Panics
    /// See [`SpriteManager::set_initial_animation`]; also if there is no composite
    /// `composite_id`.
    pub fn set_composite_initial_animation(
        &mut self,
        composite_id: CompositeSpriteId,
        animation_index: u8,
    ) {
        for sprite_id in self.composite(composite_id).sprites.clone() {
            self.set_initial_animation(sprite_id, animation_index);
        }
    }

    /// Generate code to enable a composite animation by index
    /// Sets all sprites in the composite to the same animation index
    ///
    /// # Panics
    /// See [`SpriteManager::enable_animation`]; also if there is no composite
    /// `composite_id`.
    pub fn enable_composite_animation(
        &self,
        composite_id: CompositeSpriteId,
        animation_index: u8,
    ) -> Vec<Instr> {
        self.composite(composite_id)
            .sprites
            .iter()
            .flat_map(|sprite_id| self.enable_animation(*sprite_id, animation_index))
            .collect()
    }

    /// Generate code to disable all animations for a composite sprite
    /// Sets all sprites to ANIM_DISABLED (255)
    ///
    /// # Panics
    /// See [`SpriteManager::disable_animation`]; also if there is no composite
    /// `composite_id`.
    pub fn disable_composite_animation(&self, composite_id: CompositeSpriteId) -> Vec<Instr> {
        self.composite(composite_id)
            .sprites
            .iter()
            .flat_map(|sprite_id| self.disable_animation(*sprite_id))
            .collect()
    }

    /// Move a composite sprite left by `distance` pixels, as one block: no sprite of the
    /// composite goes left of `limit` (an OAM X, screen x + 8)
    ///
    /// The sprite that reaches the limit first (the leftmost one) is tested and stops
    /// exactly on the limit, the others keep their offsets from it: the composite never
    /// splits at a screen edge. See [`SpriteManager::move_left_limit`] for the limit rules.
    pub fn move_composite_left_limit(
        &self,
        id: CompositeSpriteId,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_composite_limit(id, Axis::X, MoveDir::Decrease, "left", distance, limit)
    }

    /// Move a composite sprite right by `distance` pixels, as one block: no sprite of the
    /// composite goes right of `limit` (an OAM X, screen x + 8); the rightmost one is tested,
    /// see [`SpriteManager::move_composite_left_limit`]
    pub fn move_composite_right_limit(
        &self,
        id: CompositeSpriteId,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_composite_limit(id, Axis::X, MoveDir::Increase, "right", distance, limit)
    }

    /// Move a composite sprite up by `distance` pixels, as one block: no sprite of the
    /// composite goes above `limit` (an OAM Y, screen y + 16); the topmost one is tested,
    /// see [`SpriteManager::move_composite_left_limit`]
    pub fn move_composite_up_limit(
        &self,
        id: CompositeSpriteId,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_composite_limit(id, Axis::Y, MoveDir::Decrease, "up", distance, limit)
    }

    /// Move a composite sprite down by `distance` pixels, as one block: no sprite of the
    /// composite goes below `limit` (an OAM Y, screen y + 16); the lowest one is tested,
    /// see [`SpriteManager::move_composite_left_limit`]
    pub fn move_composite_down_limit(
        &self,
        id: CompositeSpriteId,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_composite_limit(id, Axis::Y, MoveDir::Increase, "down", distance, limit)
    }

    /// Limited move of a composite (B6): the member that reaches the limit first leads,
    /// the others follow at the offsets they were created with
    fn move_composite_limit(
        &self,
        id: CompositeSpriteId,
        axis: Axis,
        dir: MoveDir,
        name: &str,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        let members: Vec<&SpriteData> = self
            .composite(id)
            .sprites
            .iter()
            .map(|sprite_id| self.sprite(*sprite_id))
            .collect();
        let pos = |sprite: &SpriteData| i16::from(axis.position(sprite));

        // The leftmost / rightmost / topmost / lowest member; the first one on a tie
        let Some(lead) = members.iter().copied().reduce(|lead, sprite| {
            let ahead = match dir {
                MoveDir::Decrease => pos(sprite) < pos(lead),
                MoveDir::Increase => pos(sprite) > pos(lead),
            };
            if ahead { sprite } else { lead }
        }) else {
            return Vec::new();
        };
        let followers: Vec<(Expr, i16)> = members
            .iter()
            .filter(|sprite| sprite.oam_index != lead.oam_index)
            .map(|sprite| (axis.oam_address(sprite), pos(sprite) - pos(lead)))
            .collect();

        move_coord_limit(
            &self.labels,
            &format!("sprite{}_{}_limit", lead.oam_index, name),
            &axis.oam_address(lead),
            &followers,
            dir,
            distance,
            limit,
        )
    }

    /// Generate the code that writes every sprite to OAM, at start-up: `gb_std`'s
    /// [`draw_sprites`], in OAM order (the OAM was cleared before, with
    /// `gb_std::graphics::sprites::clear_objects_screen`)
    pub(crate) fn generate_init_code(&self) -> Vec<Instr> {
        let mut sorted_sprites: Vec<_> = self.sprites.values().collect();
        sorted_sprites.sort_by_key(|s| s.oam_index);
        // `add` checked that each position fits in OAM (B17)
        let sprites: Vec<Sprite> = sorted_sprites
            .into_iter()
            .map(|s| Sprite::new(s.oam_index, s.x, s.y, s.tile_index, s.flags))
            .collect();
        draw_sprites(&sprites)
    }

    /// Generate movement code for a specific sprite: add the value of the variable
    /// `var_name` to its X (changes `a`, `b` and the flags)
    ///
    /// # Panics
    /// If there is no sprite `id`.
    pub fn move_x_var(&self, id: SpriteId, var_name: &str) -> Vec<Instr> {
        self.move_var(id, Axis::X, var_name)
    }

    /// Generate movement code for Y axis with variable: add the value of the variable
    /// `var_name` to the sprite's Y (changes `a`, `b` and the flags)
    ///
    /// # Panics
    /// If there is no sprite `id`.
    pub fn move_y_var(&self, id: SpriteId, var_name: &str) -> Vec<Instr> {
        self.move_var(id, Axis::Y, var_name)
    }

    /// Add the variable `var_name` to the sprite's coordinate on `axis` (`gb_std`'s code)
    fn move_var(&self, id: SpriteId, axis: Axis, var_name: &str) -> Vec<Instr> {
        move_coord_var(&axis.oam_address(self.sprite(id)), var_name)
    }

    /// Move a sprite left by `distance` pixels, but never left of `limit`
    ///
    /// `limit` is an OAM X (screen x + 8) and is included: the sprite can stand on it.
    /// - A step that would go past the limit stops exactly on it, so the sprite reaches
    ///   the same edge whatever the distance, and never wraps around 0 / 255.
    /// - A sprite already past the limit does not move: the move never takes it further,
    ///   and does not pull it back either.
    ///
    /// # Panics
    /// If there is no sprite `id` (the same for every move and getter of a sprite).
    pub fn move_left_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        self.move_limit(id, Axis::X, MoveDir::Decrease, "left", distance, limit)
    }

    /// Move a sprite right by `distance` pixels, but never right of `limit` (an OAM X,
    /// screen x + 8); see [`SpriteManager::move_left_limit`] for the limit rules
    pub fn move_right_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        self.move_limit(id, Axis::X, MoveDir::Increase, "right", distance, limit)
    }

    /// Move a sprite up by `distance` pixels, but never above `limit` (an OAM Y,
    /// screen y + 16); see [`SpriteManager::move_left_limit`] for the limit rules
    pub fn move_up_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        self.move_limit(id, Axis::Y, MoveDir::Decrease, "up", distance, limit)
    }

    /// Move a sprite down by `distance` pixels, but never below `limit` (an OAM Y,
    /// screen y + 16); see [`SpriteManager::move_left_limit`] for the limit rules
    pub fn move_down_limit(&self, id: SpriteId, distance: u8, limit: u8) -> Vec<Instr> {
        self.move_limit(id, Axis::Y, MoveDir::Increase, "down", distance, limit)
    }

    /// Limited move of one sprite (B8); its local labels are numbered by the shared
    /// allocator, so the same move can be emitted many times, and inside an `If` (B7)
    fn move_limit(
        &self,
        id: SpriteId,
        axis: Axis,
        dir: MoveDir,
        name: &str,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        let sprite = self.sprite(id);
        move_coord_limit(
            &self.labels,
            &format!("sprite{}_{}_limit", sprite.oam_index, name),
            &axis.oam_address(sprite),
            &[],
            dir,
            distance,
            limit,
        )
    }

    /// Get sprite pivot point (for collision detection): the sprite's pixel offset by
    /// (`x_offset`, `y_offset`) into `b` (x) and `c` (y), as `GetTileByPixel` takes them
    ///
    /// A positive offset goes left / up: `(0, 1)` is the pixel above the sprite's
    /// top-left one, `(-1, 0)` the one to its right. The result wraps around the 256-pixel
    /// background map. Changes `a`, `b`, `c` and the flags.
    ///
    /// # Panics
    /// If there is no sprite `id`, or an offset is out of -255 to 255 (on a 256-pixel map,
    /// 256 is 0: a mistake).
    pub fn get_pivot(&self, id: SpriteId, x_offset: i16, y_offset: i16) -> Vec<Instr> {
        pivot(self.sprite(id).oam_index, x_offset, y_offset)
    }

    /// Get sprite Y position (its OAM Y, into `a`)
    ///
    /// # Panics
    /// If there is no sprite `id`.
    pub fn get_y(&self, id: SpriteId) -> Vec<Instr> {
        let mut asm = Block::new();
        asm.ld_a_addr_def(Axis::Y.oam_address(self.sprite(id)));
        asm.into_instrs()
    }

    /// Get sprite X position (its OAM X, into `a`)
    ///
    /// # Panics
    /// If there is no sprite `id`.
    pub fn get_x(&self, id: SpriteId) -> Vec<Instr> {
        let mut asm = Block::new();
        asm.ld_a_addr_def(Axis::X.oam_address(self.sprite(id)));
        asm.into_instrs()
    }

    /// Check if any sprites have been added
    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    /// Check if any animations have been added to any sprite
    pub fn has_animations(&self) -> bool {
        self.sprites.values().any(|s| !s.animations.is_empty())
    }

    /// The names of the animation functions `build()` generates, in their order
    pub(crate) fn animation_function_names(&self) -> Vec<String> {
        self.sprites
            .values()
            .flat_map(|sprite| {
                sprite
                    .animations
                    .iter()
                    .map(|animation| animation_label(&sprite.name, &animation.name))
            })
            .collect()
    }

    /// Generate animation functions for all sprites with animations
    /// Returns a list of (function_name, function_body) pairs
    pub(crate) fn generate_animation_functions(&self) -> Vec<(String, Vec<Instr>)> {
        let mut functions = Vec::new();

        for sprite in self.sprites.values() {
            for animation in &sprite.animations {
                let mut asm = Block::new();
                let func_name = animation_label(&sprite.name, &animation.name);

                asm.label(&func_name);
                asm.emit_all(animation.generate_func(&direction_var(&sprite.name)));
                asm.ret();

                functions.push((func_name, asm.into_instrs()));
            }
        }

        functions
    }

    /// Generate the main loop animation code with frame-based timing
    /// Uses wFrameCounter for non-blocking animation updates
    /// - Increments frame counter each frame
    /// - Only updates animations when counter >= delay
    /// - Resets counter after animation update
    /// - Checks wAnim_[sprite_name]_Current to call only the active animation
    ///
    /// The code grows with every animation, so each jump over a part whose size depends
    /// on the number of sprites or animations is a `jp`: a `jr` reaches only 127 bytes
    /// ahead (B9). The only `jr` left skips one `call` and one `jp`, 6 bytes.
    ///
    /// Its labels are local and come from `labels`: `.anim_end_N` after the whole
    /// dispatcher (it was the global label `AnimEnd`), `.anim_{sprite}_end_N` after a
    /// sprite's part and `.skip_{sprite}_{animation}_N` after an animation's call.
    pub(crate) fn generate_animation_calls(
        &self,
        labels: &LabelAllocator,
        delay_value: u8,
    ) -> Vec<Instr> {
        let mut asm = Block::new();
        let anim_end = labels.local("anim_end");

        // Increment frame counter
        asm.ld_a_addr_def("wFrameCounter");
        asm.inc(R8::A);
        asm.ld_addr_def_a("wFrameCounter");

        // Compare with delay value
        asm.cp_imm(delay_value);
        asm.jp_cond(Condition::C, &anim_end); // if counter < delay, skip animations

        // Reset frame counter
        asm.ld_a(0);
        asm.ld_addr_def_a("wFrameCounter");

        // For each sprite, check its current animation index and call the right animation
        for sprite in self.sprites.values() {
            if sprite.animations.is_empty() {
                continue;
            }

            let current_var = format!("wAnim_{}_Current", sprite.name);
            let sprite_end_label = labels.local(&format!("anim_{}_end", sprite.name));

            // Load current animation index
            asm.ld_a_addr_def(&current_var);

            // Check if disabled (255)
            asm.cp_imm(ANIM_DISABLED);
            asm.jp_cond(Condition::Z, &sprite_end_label);

            // For each animation, check if it's the current one
            for animation in &sprite.animations {
                let func_name = animation_label(&sprite.name, &animation.name);
                let skip_label = labels.local(&format!("skip_{}_{}", sprite.name, animation.name));

                // Check if this animation index is selected (the skip is 6 bytes)
                asm.cp_imm(animation.index);
                asm.jr_cond(Condition::NZ, &skip_label);

                // Call this animation
                asm.call(&func_name);
                asm.jp(&sprite_end_label);

                asm.label(&skip_label);
            }

            asm.label(&sprite_end_label);
        }

        asm.label(&anim_end);

        asm.into_instrs()
    }

    /// Get list of animation variable names (for auto-creating variables)
    /// Returns (name, initial_value) pairs
    /// Creates one variable per sprite: wAnim_[sprite_name]_Current, and
    /// wAnim_[sprite_name]_Dir (0 = forward) if the sprite has a `PingPong` animation
    /// of two frames or more
    pub(crate) fn get_animation_variables(&self) -> Vec<(String, u8)> {
        let mut vars = Vec::new();

        for sprite in self.sprites.values() {
            if !sprite.animations.is_empty() {
                let var_name = format!("wAnim_{}_Current", sprite.name);
                vars.push((var_name, sprite.initial_animation));
            }
            if sprite.animations.iter().any(|anim| anim.needs_direction()) {
                vars.push((direction_var(&sprite.name), 0));
            }
        }

        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::test_cpu::TestCpu;
    use crate::gb_std::graphics::sprites::MoveDir;
    use crate::gb_std::graphics::sprites::tests::{DISTANCES, LIMITS, expected_move};
    use crate::rust_boy::{RustBoy, TileSource};

    /// A test CPU whose OAM holds every sprite at its initial position
    fn cpu_with_oam(sm: &SpriteManager) -> TestCpu {
        let mut cpu = TestCpu::default();
        for sprite in sm.sprites.values() {
            let entry = sprite.oam_index * 4;
            cpu.mem.insert(format!("_OAMRAM+{}", entry), sprite.y + 16);
            cpu.mem
                .insert(format!("_OAMRAM+{}", entry + 1), sprite.x + 8);
        }
        cpu
    }

    type LimitMove = fn(&SpriteManager, SpriteId, u8, u8) -> Vec<Instr>;

    #[test]
    fn test_limit_moves_stop_exactly_on_the_limit() {
        // (name, method, byte of the OAM entry it moves (Y = 0, X = 1), direction)
        let moves: [(&str, LimitMove, u8, MoveDir); 4] = [
            ("left", SpriteManager::move_left_limit, 1, MoveDir::Decrease),
            (
                "right",
                SpriteManager::move_right_limit,
                1,
                MoveDir::Increase,
            ),
            ("up", SpriteManager::move_up_limit, 0, MoveDir::Decrease),
            ("down", SpriteManager::move_down_limit, 0, MoveDir::Increase),
        ];
        let mut sm = SpriteManager::new(LabelAllocator::new());
        sm.add_for_test("Paddle", 16, 128, 0, 1);
        // OAM entry 1: Y at _OAMRAM+4, X at _OAMRAM+5
        let ball = sm.add_for_test("Ball", 32, 100, 0, 1);

        for (name, method, byte, dir) in moves {
            let moved = format!("_OAMRAM+{}", 4 + byte);
            for distance in DISTANCES {
                for limit in LIMITS {
                    let code = method(&sm, ball, distance, limit);
                    for pos in 0..=255 {
                        let mut cpu = cpu_with_oam(&sm);
                        cpu.mem.insert(moved.clone(), pos);
                        let mut want = cpu.mem.clone();
                        want.insert(moved.clone(), expected_move(dir, pos, distance, limit));
                        cpu.run(&code);
                        // Only the moved coordinate changes, and it lands where it must
                        assert_eq!(
                            cpu.mem, want,
                            "move_{}_limit({}, {}) from {}",
                            name, distance, limit, pos
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_limit_move_never_passes_the_limit() {
        // B8: with a step of 2 from X 24 towards the limit 15, the sprite went
        // 22, 20, 18, 16, 14, ... and wrapped around through 0 / 255
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let ball = sm.add_for_test("Ball", 16, 100, 0, 1); // X 24 in OAM
        let left = sm.move_left_limit(ball, 2, 15);
        let mut cpu = cpu_with_oam(&sm);
        for frame in 0..300 {
            cpu.run(&left);
            let x = cpu.mem["_OAMRAM+1"];
            assert!(x >= 15, "frame {}: X {} is past the limit 15", frame, x);
        }
        assert_eq!(cpu.mem["_OAMRAM+1"], 15, "the sprite stops on the limit");

        // A sprite already past the limit is not pushed further (it went 9, 8, ..., 0, 255, ...)
        cpu.mem.insert("_OAMRAM+1".to_string(), 10);
        for _ in 0..300 {
            cpu.run(&left);
        }
        assert_eq!(cpu.mem["_OAMRAM+1"], 10);

        // Same to the right: step 3 from X 24 towards 100 goes ..., 96, 99, then stops on 100
        let right = sm.move_right_limit(ball, 3, 100);
        cpu.mem.insert("_OAMRAM+1".to_string(), 24);
        for frame in 0..300 {
            cpu.run(&right);
            let x = cpu.mem["_OAMRAM+1"];
            assert!(x <= 100, "frame {}: X {} is past the limit 100", frame, x);
        }
        assert_eq!(cpu.mem["_OAMRAM+1"], 100);
    }

    type CompositeMove = fn(&SpriteManager, CompositeSpriteId, u8, u8) -> Vec<Instr>;

    #[test]
    fn test_composite_moves_as_one_block() {
        // B6: each 8x16 half used to stop at the limit on its own, so at a screen edge
        // the leading half stopped while the other kept going and the 16x16 collapsed
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let player = gb.add_sprite_16x16(
            "player",
            TileSource::from_file("left.2bpp", 2),
            TileSource::from_file("right.2bpp", 2),
            80,
            72,
            0,
        );
        // OAM entry 0 is the left half (Y 88, X 88), entry 1 the right half (Y 88, X 96)
        let halves = |cpu: &TestCpu| {
            let m = |n: u8| cpu.mem[&format!("_OAMRAM+{}", n)];
            ((m(0), m(1)), (m(4), m(5)))
        };
        // (name, method, direction, axis byte in the OAM entry, limit) as in the fosdem example
        let moves: [(&str, CompositeMove, MoveDir, u8, u8); 4] = [
            (
                "left",
                SpriteManager::move_composite_left_limit,
                MoveDir::Decrease,
                1,
                1,
            ),
            (
                "right",
                SpriteManager::move_composite_right_limit,
                MoveDir::Increase,
                1,
                149,
            ),
            (
                "up",
                SpriteManager::move_composite_up_limit,
                MoveDir::Decrease,
                0,
                1,
            ),
            (
                "down",
                SpriteManager::move_composite_down_limit,
                MoveDir::Increase,
                0,
                149,
            ),
        ];

        for (name, method, dir, byte, limit) in moves {
            for distance in [1, 3, 8] {
                let code = method(&gb.sprites, player, distance, limit);
                let mut cpu = cpu_with_oam(&gb.sprites);
                for frame in 0..300 {
                    cpu.run(&code);
                    let ((ly, lx), (ry, rx)) = halves(&cpu);
                    let what = format!(
                        "move_composite_{}_limit({}, {}), frame {}: left half at ({}, {}), right half at ({}, {})",
                        name, distance, limit, frame, lx, ly, rx, ry
                    );
                    assert_eq!(
                        (ry, rx),
                        (ly, lx.wrapping_add(8)),
                        "{}: the halves split",
                        what
                    );
                    let coords = if byte == 1 { [lx, rx] } else { [ly, ry] };
                    for coord in coords {
                        match dir {
                            MoveDir::Decrease => {
                                assert!(coord >= limit, "{}: past the limit", what)
                            }
                            MoveDir::Increase => {
                                assert!(coord <= limit, "{}: past the limit", what)
                            }
                        }
                    }
                }
                // The leading half ends exactly on the limit
                let ((ly, lx), (ry, rx)) = halves(&cpu);
                let lead = match (byte, dir) {
                    (1, MoveDir::Decrease) => lx,
                    (1, MoveDir::Increase) => rx,
                    (_, MoveDir::Decrease) => ly,
                    (_, MoveDir::Increase) => ry,
                };
                assert_eq!(
                    lead, limit,
                    "move_composite_{}_limit({}, {})",
                    name, distance, limit
                );
            }
        }
    }

    #[test]
    fn test_get_pivot_handles_every_offset() {
        // B22: out-of-range offsets were clamped to `sub 0`
        let mut sm = SpriteManager::new(LabelAllocator::new());
        sm.add_for_test("Paddle", 16, 128, 0, 1);
        let ball = sm.add_for_test("Ball", 32, 100, 0, 1);
        crate::gb_std::graphics::sprites::tests::check_pivot(
            |x, y| sm.get_pivot(ball, x, y),
            "_OAMRAM+4",
            "_OAMRAM+5",
        );
    }

    #[test]
    #[should_panic(expected = "get_pivot: the offset -256 is out of range")]
    fn test_get_pivot_rejects_an_offset_past_the_map() {
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let ball = sm.add_for_test("Ball", 32, 100, 0, 1);
        sm.get_pivot(ball, 0, -256);
    }

    // ==================== Loud failures (B20) ====================

    use crate::rust_boy::panic_message;

    /// A manager with one sprite, "Coin" (4 tiles, two animations, "Spin" and "Flip"),
    /// one without animations, "Ball", and a 16x16 composite, "Player"
    fn coin_and_ball() -> (SpriteManager, SpriteId, SpriteId, CompositeSpriteId) {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let coin = gb.add_sprite("Coin", tiles(4), 16, 16, 0);
        gb.sprites
            .add_animation(coin, "Spin", 0, 1, AnimationType::Loop);
        gb.sprites
            .add_animation(coin, "Flip", 1, 1, AnimationType::Loop);
        let ball = gb.add_sprite("Ball", tiles(2), 32, 16, 0);
        let player = gb.add_sprite_16x16("Player", tiles(2), tiles(2), 80, 72, 0);
        (gb.sprites, coin, ball, player)
    }

    type SpriteOp = Box<dyn Fn(&mut SpriteManager, SpriteId)>;

    #[test]
    fn test_an_unknown_sprite_id_panics() {
        // B20: every method on an unknown id returned no code (or 0), so the program
        // silently did nothing
        let ops: Vec<(&str, SpriteOp)> = vec![
            (
                "move_left_limit",
                Box::new(|sm, id| drop(sm.move_left_limit(id, 1, 8))),
            ),
            (
                "move_right_limit",
                Box::new(|sm, id| drop(sm.move_right_limit(id, 1, 8))),
            ),
            (
                "move_up_limit",
                Box::new(|sm, id| drop(sm.move_up_limit(id, 1, 8))),
            ),
            (
                "move_down_limit",
                Box::new(|sm, id| drop(sm.move_down_limit(id, 1, 8))),
            ),
            (
                "move_x_var",
                Box::new(|sm, id| drop(sm.move_x_var(id, "wDx"))),
            ),
            (
                "move_y_var",
                Box::new(|sm, id| drop(sm.move_y_var(id, "wDy"))),
            ),
            ("get_pivot", Box::new(|sm, id| drop(sm.get_pivot(id, 0, 1)))),
            ("get_x", Box::new(|sm, id| drop(sm.get_x(id)))),
            ("get_y", Box::new(|sm, id| drop(sm.get_y(id)))),
            (
                "enable_animation",
                Box::new(|sm, id| drop(sm.enable_animation(id, 0))),
            ),
            (
                "enable_animation_by_name",
                Box::new(|sm, id| drop(sm.enable_animation_by_name(id, "Spin"))),
            ),
            (
                "disable_animation",
                Box::new(|sm, id| drop(sm.disable_animation(id))),
            ),
            (
                "add_animation",
                Box::new(|sm, id| {
                    sm.add_animation(id, "Spin", 0, 0, AnimationType::Loop);
                }),
            ),
            (
                "set_initial_animation",
                Box::new(|sm, id| sm.set_initial_animation(id, 0)),
            ),
            (
                "set_initial_animation_by_name",
                Box::new(|sm, id| sm.set_initial_animation_by_name(id, "Spin")),
            ),
        ];
        for (name, op) in ops {
            let (mut sm, ..) = coin_and_ball();
            let message = panic_message(|| op(&mut sm, SpriteId(99)));
            assert!(
                message.contains("unknown sprite id 99"),
                "{}: {}",
                name,
                message
            );
        }
    }

    type CompositeOp = Box<dyn Fn(&mut SpriteManager, CompositeSpriteId)>;

    #[test]
    fn test_an_unknown_composite_id_panics() {
        let ops: Vec<(&str, CompositeOp)> = vec![
            (
                "move_composite_left_limit",
                Box::new(|sm, id| drop(sm.move_composite_left_limit(id, 1, 8))),
            ),
            (
                "move_composite_right_limit",
                Box::new(|sm, id| drop(sm.move_composite_right_limit(id, 1, 8))),
            ),
            (
                "move_composite_up_limit",
                Box::new(|sm, id| drop(sm.move_composite_up_limit(id, 1, 8))),
            ),
            (
                "move_composite_down_limit",
                Box::new(|sm, id| drop(sm.move_composite_down_limit(id, 1, 8))),
            ),
            (
                "add_composite_animation",
                Box::new(|sm, id| {
                    sm.add_composite_animation(id, "Walk", 0, 0, AnimationType::Loop);
                }),
            ),
            (
                "set_composite_initial_animation",
                Box::new(|sm, id| sm.set_composite_initial_animation(id, 0)),
            ),
            (
                "enable_composite_animation",
                Box::new(|sm, id| drop(sm.enable_composite_animation(id, 0))),
            ),
            (
                "disable_composite_animation",
                Box::new(|sm, id| drop(sm.disable_composite_animation(id))),
            ),
        ];
        for (name, op) in ops {
            let (mut sm, ..) = coin_and_ball();
            let message = panic_message(|| op(&mut sm, CompositeSpriteId(7)));
            assert!(
                message.contains("unknown composite sprite id 7"),
                "{}: {}",
                name,
                message
            );
        }
    }

    #[test]
    fn test_an_unknown_animation_name_panics() {
        // B20: a typo did nothing (no code, no initial animation)
        let (mut sm, coin, ..) = coin_and_ball();
        let message = panic_message(|| sm.enable_animation_by_name(coin, "Spn"));
        assert!(
            message.contains("sprite \"Coin\" has no animation \"Spn\"")
                && message.contains("Spin, Flip"),
            "{}",
            message
        );
        let message = panic_message(|| sm.set_initial_animation_by_name(coin, "spin"));
        assert!(message.contains("no animation \"spin\""), "{}", message);
        // The right names work
        let text =
            |code: Vec<Instr>| -> Vec<String> { code.iter().map(Instr::to_string).collect() };
        assert_eq!(
            text(sm.enable_animation_by_name(coin, "Flip")),
            text(sm.enable_animation(coin, 1))
        );
        sm.set_initial_animation_by_name(coin, "Flip");
        assert_eq!(sm.get(coin).unwrap().initial_animation, 1);
    }

    #[test]
    fn test_an_unknown_animation_index_panics() {
        let (mut sm, coin, ball, player) = coin_and_ball();
        // Coin has the animations 0 and 1
        let message = panic_message(|| sm.enable_animation(coin, 2));
        assert!(
            message.contains("sprite \"Coin\" has no animation 2 (it has 2: 0 to 1)"),
            "{}",
            message
        );
        let message = panic_message(|| sm.set_initial_animation(coin, 2));
        assert!(message.contains("no animation 2"), "{}", message);
        let message = panic_message(|| sm.enable_composite_animation(player, 0));
        assert!(message.contains("no animation 0"), "{}", message);
        // ANIM_DISABLED is the initial "no animation"; enable_animation needs a real one
        sm.set_initial_animation(coin, ANIM_DISABLED);
        let message = panic_message(|| sm.enable_animation(coin, ANIM_DISABLED));
        assert!(message.contains("disable_animation"), "{}", message);

        // A sprite without animations has no animation variable to set
        // (`wAnim_Ball_Current` would be an undefined symbol)
        for message in [
            panic_message(|| sm.enable_animation(ball, 0)),
            panic_message(|| sm.disable_animation(ball)),
            panic_message(|| sm.disable_composite_animation(player)),
        ] {
            assert!(message.contains("has no animations"), "{}", message);
        }
    }

    #[test]
    fn test_at_most_255_animations_per_sprite() {
        // B20: the index is a u8 and 255 means "disabled" (ANIM_DISABLED): the 256th
        // animation got index 255, so enabling it disabled the sprite, and the one after
        // wrapped to index 0
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let coin = sm.add_for_test("Coin", 0, 0, 0, 1);
        for i in 0..255 {
            let index = sm.add_animation(coin, &format!("A{}", i), 0, 0, AnimationType::Loop);
            assert_eq!(index, i as u8);
        }
        let message = panic_message(|| sm.add_animation(coin, "A255", 0, 0, AnimationType::Loop));
        assert!(
            message.contains("sprite \"Coin\" already has 255 animations"),
            "{}",
            message
        );
    }

    #[test]
    fn test_add_sprite() {
        let mut sm = SpriteManager::new(LabelAllocator::new());

        let paddle = sm.add_for_test("Paddle", 16, 128, 0, 1);
        let ball = sm.add_for_test("Ball", 32, 100, 0, 1);

        assert_eq!(sm.get(paddle).unwrap().x, 16);
        assert_eq!(sm.get(paddle).unwrap().y, 128);
        assert_eq!(sm.get(ball).unwrap().x, 32);
        assert_eq!(sm.get(ball).unwrap().y, 100);
    }

    #[test]
    fn test_oam_indices() {
        let mut sm = SpriteManager::new(LabelAllocator::new());

        let paddle = sm.add_for_test("Paddle", 16, 128, 0, 1);
        let ball = sm.add_for_test("Ball", 32, 100, 0, 1);

        assert_eq!(sm.get(paddle).unwrap().oam_index, 0);
        assert_eq!(sm.get(ball).unwrap().oam_index, 1);
    }

    // ==================== Animations (B9, B10) ====================

    use crate::gb_asm::label_check::{assert_labels_ok, jr_range_errors};
    use crate::rust_boy::AnimationType;

    /// The animation code as the main loop runs it each frame: the dispatcher, a `ret`
    /// that ends the frame on the test CPU, then every animation function
    fn animation_program(sm: &SpriteManager, delay: u8) -> Vec<Instr> {
        let mut code = sm.generate_animation_calls(&LabelAllocator::new(), delay);
        code.push(Instr::Ret);
        for (_, body) in sm.generate_animation_functions() {
            code.extend(body);
        }
        code
    }

    /// A test CPU with each sprite on its initial tile and the animation variables
    /// set as `build()` initialises them
    fn animation_cpu(sm: &SpriteManager) -> TestCpu {
        let mut cpu = TestCpu::default();
        for sprite in sm.sprites.values() {
            let tile = format!("_OAMRAM+{}", sprite.oam_index * 4 + 2);
            cpu.mem.insert(tile, sprite.tile_index);
        }
        cpu.mem.insert("wFrameCounter".to_string(), 0);
        for (name, value) in sm.get_animation_variables() {
            cpu.mem.insert(name, value);
        }
        cpu
    }

    /// The frame `sprite` shows (its tile, counted in frames from its first tile)
    fn shown_frame(sm: &SpriteManager, cpu: &TestCpu, sprite: SpriteId) -> u8 {
        let data = sm.get(sprite).unwrap();
        let tile = cpu.mem[&format!("_OAMRAM+{}", data.oam_index * 4 + 2)];
        (tile - data.tile_index) / sm.size().tiles_per_sprite()
    }

    /// Run `frames` frames of the animation code, one update per frame, and return the
    /// frame `sprite` shows after each one
    fn play(sm: &SpriteManager, cpu: &mut TestCpu, sprite: SpriteId, frames: usize) -> Vec<u8> {
        let code = animation_program(sm, 1);
        (0..frames)
            .map(|_| {
                cpu.run(&code);
                shown_frame(sm, cpu, sprite)
            })
            .collect()
    }

    /// A sprite after another one (so its tiles do not start at 0) with 6 frames, and
    /// one animation of frames `start..=end`, enabled
    fn animated_sprite(
        size: SpriteSize,
        anim_type: AnimationType,
        start: u8,
        end: u8,
    ) -> (SpriteManager, SpriteId) {
        let per_frame = size.tiles_per_sprite();
        let mut sm = SpriteManager::new(LabelAllocator::new());
        sm.set_size(size);
        sm.add_for_test("Other", 0, 0, 0, per_frame);
        let coin = sm.add_for_test("Coin", 16, 16, 0, 6 * per_frame);
        let spin = sm.add_animation(coin, "Spin", start, end, anim_type);
        sm.set_initial_animation(coin, spin);
        (sm, coin)
    }

    const SIZES: [SpriteSize; 2] = [SpriteSize::Size8x8, SpriteSize::Size8x16];

    #[test]
    fn test_loop_animation_ending_on_the_last_sprite_tile() {
        // B17: the Loop code compared with `last frame + frame_step`, which is 256 when
        // the last frame is tile 255 (8x8) or 254 (8x16): it overflowed (a panic when
        // generated in debug, `cp 0` in release: the animation froze on its first frame)
        for size in SIZES {
            let per_frame = size.tiles_per_sprite();
            let mut sm = SpriteManager::new(LabelAllocator::new());
            sm.set_size(size);
            // 248 (8x8) or 240 (8x16) tiles before the coin's 8 frames
            let other = (256 - 8 * u16::from(per_frame)) as u8;
            sm.add_for_test("Other", 0, 0, 0, other);
            let coin = sm.add_for_test("Coin", 16, 16, 0, 8 * per_frame);
            let spin = sm.add_animation(coin, "Spin", 0, 7, AnimationType::Loop);
            sm.set_initial_animation(coin, spin);
            // The last frame is tile 255 (8x8) or 254 and 255 (8x16)
            let last_tile = sm.get(coin).unwrap().tile_index + 7 * per_frame;
            assert_eq!(u16::from(last_tile), 256 - u16::from(per_frame));
            let mut cpu = animation_cpu(&sm);
            assert_eq!(
                play(&sm, &mut cpu, coin, 10),
                [1, 2, 3, 4, 5, 6, 7, 0, 1, 2],
                "{:?}",
                size
            );
        }
    }

    #[test]
    fn test_animation_frames_must_be_the_sprite_tiles() {
        // B17: frames past the sprite's tiles showed the next sprite's tiles, and past
        // tile 255 the frame arithmetic overflowed
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let coin = sm.add_for_test("Coin", 0, 0, 0, 4);
        sm.add_animation(coin, "All", 0, 3, AnimationType::Loop);
        let message = panic_message(|| {
            sm.add_animation(coin, "Past", 2, 4, AnimationType::Loop);
        });
        assert!(
            message.contains("animation \"Past\" of sprite \"Coin\"")
                && message.contains("frame 4")
                && message.contains("4 tiles"),
            "{}",
            message
        );
        let message = panic_message(|| {
            sm.add_animation(coin, "Backward", 3, 1, AnimationType::Loop);
        });
        assert!(
            message.contains("start_frame 3 is after end_frame 1"),
            "{}",
            message
        );
        let message = panic_message(|| {
            sm.add_animation_with_step(coin, "Still", 0, 2, AnimationType::Loop, 0);
        });
        assert!(
            message.contains("frame_step must be at least 1"),
            "{}",
            message
        );
        // A step of 2 in 8x8 mode: frames 0 and 2, 4 would be past the 4 tiles
        sm.add_animation_with_step(coin, "Even", 0, 1, AnimationType::Loop, 2);
        let message = panic_message(|| {
            sm.add_animation_with_step(coin, "TooFar", 0, 2, AnimationType::Loop, 2);
        });
        assert!(message.contains("frame 2"), "{}", message);

        // In 8x16 mode a frame is two tiles: frame 1 of a 2-tile sprite is past it
        let mut sm = SpriteManager::new(LabelAllocator::new());
        sm.set_size(SpriteSize::Size8x16);
        let player = sm.add_for_test("Player", 0, 0, 0, 2);
        let message = panic_message(|| {
            sm.add_animation(player, "Walk", 0, 1, AnimationType::Loop);
        });
        assert!(message.contains("frame 1"), "{}", message);
    }

    #[test]
    fn test_loop_animation_frames() {
        for size in SIZES {
            // The sprite starts on frame 0, outside the animation: it goes to frame 1
            let (sm, coin) = animated_sprite(size, AnimationType::Loop, 1, 4);
            let mut cpu = animation_cpu(&sm);
            assert_eq!(
                play(&sm, &mut cpu, coin, 10),
                [1, 2, 3, 4, 1, 2, 3, 4, 1, 2],
                "{:?}",
                size
            );
        }
    }

    #[test]
    fn test_ping_pong_animation_frames() {
        // B10: PingPong played as a Loop (1 2 3 4 1 2 ...)
        for size in SIZES {
            let (sm, coin) = animated_sprite(size, AnimationType::PingPong, 1, 4);
            let mut cpu = animation_cpu(&sm);
            // Forward, then backward; the ends are not shown twice in a row
            assert_eq!(
                play(&sm, &mut cpu, coin, 13),
                [1, 2, 3, 4, 3, 2, 1, 2, 3, 4, 3, 2, 1],
                "{:?}",
                size
            );
        }
    }

    #[test]
    fn test_once_animation_frames() {
        // B10: Once played as a Loop (1 2 3 4 1 2 ...)
        for size in SIZES {
            let (sm, coin) = animated_sprite(size, AnimationType::Once, 1, 4);
            let mut cpu = animation_cpu(&sm);
            // To the last frame, which stays
            assert_eq!(
                play(&sm, &mut cpu, coin, 8),
                [1, 2, 3, 4, 4, 4, 4, 4],
                "{:?}",
                size
            );
        }
    }

    #[test]
    fn test_animation_starting_on_its_first_frame() {
        // The sprite's initial tile is the animation's first frame (0)
        let expected: [(AnimationType, &[u8]); 3] = [
            (AnimationType::Loop, &[1, 2, 3, 0, 1, 2, 3, 0]),
            (AnimationType::PingPong, &[1, 2, 3, 2, 1, 0, 1, 2]),
            (AnimationType::Once, &[1, 2, 3, 3, 3, 3, 3, 3]),
        ];
        for (anim_type, frames) in expected {
            let (sm, coin) = animated_sprite(SpriteSize::Size8x8, anim_type.clone(), 0, 3);
            let mut cpu = animation_cpu(&sm);
            assert_eq!(play(&sm, &mut cpu, coin, 8), frames, "{:?}", anim_type);
        }
    }

    #[test]
    fn test_two_frame_ping_pong_alternates() {
        for size in SIZES {
            let (sm, coin) = animated_sprite(size, AnimationType::PingPong, 1, 2);
            let mut cpu = animation_cpu(&sm);
            assert_eq!(
                play(&sm, &mut cpu, coin, 7),
                [1, 2, 1, 2, 1, 2, 1],
                "{:?}",
                size
            );
        }
    }

    #[test]
    fn test_one_frame_animation_stays_on_it() {
        for anim_type in [
            AnimationType::Loop,
            AnimationType::PingPong,
            AnimationType::Once,
        ] {
            let (sm, coin) = animated_sprite(SpriteSize::Size8x8, anim_type.clone(), 2, 2);
            let mut cpu = animation_cpu(&sm);
            assert_eq!(
                play(&sm, &mut cpu, coin, 4),
                [2, 2, 2, 2],
                "{:?}",
                anim_type
            );
        }
    }

    #[test]
    fn test_switching_between_ping_pong_animations() {
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let coin = sm.add_for_test("Coin", 16, 16, 0, 6);
        let small = sm.add_animation(coin, "Small", 0, 2, AnimationType::PingPong);
        let big = sm.add_animation(coin, "Big", 3, 5, AnimationType::PingPong);
        sm.set_initial_animation(coin, small);
        let mut cpu = animation_cpu(&sm);
        let current = "wAnim_Coin_Current".to_string();

        // Stop while going backward
        assert_eq!(play(&sm, &mut cpu, coin, 3), [1, 2, 1]);
        // The other animation starts on its first frame and goes forward
        cpu.mem.insert(current.clone(), big);
        assert_eq!(play(&sm, &mut cpu, coin, 6), [3, 4, 5, 4, 3, 4]);
        // Disabled: the sprite keeps its frame
        cpu.mem.insert(current.clone(), ANIM_DISABLED);
        assert_eq!(play(&sm, &mut cpu, coin, 2), [4, 4]);
        // Back to the first one, from its first frame
        cpu.mem.insert(current, small);
        assert_eq!(play(&sm, &mut cpu, coin, 5), [0, 1, 2, 1, 0]);
    }

    #[test]
    fn test_once_after_another_animation_plays_again() {
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let coin = sm.add_for_test("Coin", 16, 16, 0, 6);
        let jump = sm.add_animation(coin, "Jump", 0, 2, AnimationType::Once);
        let idle = sm.add_animation(coin, "Idle", 3, 4, AnimationType::Loop);
        sm.set_initial_animation(coin, jump);
        let mut cpu = animation_cpu(&sm);
        let current = "wAnim_Coin_Current".to_string();

        assert_eq!(play(&sm, &mut cpu, coin, 4), [1, 2, 2, 2]);
        cpu.mem.insert(current.clone(), idle);
        assert_eq!(play(&sm, &mut cpu, coin, 3), [3, 4, 3]);
        cpu.mem.insert(current, jump);
        assert_eq!(play(&sm, &mut cpu, coin, 4), [0, 1, 2, 2]);
    }

    #[test]
    fn test_animations_wait_for_the_delay() {
        let (sm, coin) = animated_sprite(SpriteSize::Size8x8, AnimationType::PingPong, 0, 2);
        let mut cpu = animation_cpu(&sm);
        let code = animation_program(&sm, 3);
        let frames: Vec<u8> = (0..12)
            .map(|_| {
                cpu.run(&code);
                shown_frame(&sm, &cpu, coin)
            })
            .collect();
        assert_eq!(frames, [0, 0, 1, 1, 1, 2, 2, 2, 1, 1, 1, 0]);
    }

    #[test]
    fn test_ping_pong_composite_halves_stay_together() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let player = gb.add_sprite_16x16("Player", tiles(8), tiles(8), 80, 72, 0);
        gb.sprites
            .add_composite_animation(player, "Walk", 0, 3, AnimationType::PingPong);
        gb.sprites.set_composite_initial_animation(player, 0);
        let halves = gb.sprites.get_composite_sprites(player).unwrap().clone();

        let sm = &gb.sprites;
        let mut cpu = animation_cpu(sm);
        let code = animation_program(sm, 1);
        for want in [1, 2, 3, 2, 1, 0, 1] {
            cpu.run(&code);
            for &half in &halves {
                assert_eq!(shown_frame(sm, &cpu, half), want);
            }
        }
    }

    #[test]
    fn test_only_ping_pong_sprites_get_a_direction_variable() {
        let mut sm = SpriteManager::new(LabelAllocator::new());
        let coin = sm.add_for_test("Coin", 0, 0, 0, 4);
        sm.add_animation(coin, "Spin", 0, 3, AnimationType::Loop);
        sm.add_animation(coin, "Fall", 0, 3, AnimationType::Once);
        let gem = sm.add_for_test("Gem", 0, 0, 0, 4);
        sm.add_animation(gem, "Spin", 0, 3, AnimationType::Loop);
        sm.add_animation(gem, "Shine", 0, 3, AnimationType::PingPong);
        // A one-frame PingPong never reads the direction: no variable
        let star = sm.add_for_test("Star", 0, 0, 0, 4);
        sm.add_animation(star, "Twinkle", 2, 2, AnimationType::PingPong);
        // ... unless the sprite also has a longer one
        let moon = sm.add_for_test("Moon", 0, 0, 0, 4);
        sm.add_animation(moon, "Still", 1, 1, AnimationType::PingPong);
        sm.add_animation(moon, "Wax", 0, 1, AnimationType::PingPong);
        assert_eq!(
            sm.get_animation_variables(),
            [
                ("wAnim_Coin_Current".to_string(), ANIM_DISABLED),
                ("wAnim_Gem_Current".to_string(), ANIM_DISABLED),
                ("wAnim_Gem_Dir".to_string(), 0),
                ("wAnim_Star_Current".to_string(), ANIM_DISABLED),
                ("wAnim_Moon_Current".to_string(), ANIM_DISABLED),
                ("wAnim_Moon_Dir".to_string(), 0),
            ]
        );
    }

    /// `count` 8x8 sprite tiles
    fn tiles(count: usize) -> TileSource {
        TileSource::from_raw(&vec![["$FF"; 8]; count])
    }

    #[test]
    fn test_animation_dispatch_jumps_stay_in_range() {
        // B9: `jr c, AnimEnd` jumped over the whole dispatcher (5 bytes + 7 per animated
        // sprite + 9 per animation): with 3 sprites of 4 animations it is 134 bytes away,
        // and rgblink fails. A sprite's own jumps (disabled, and after the call) span its
        // animations: with 16 of them they went out of range too.
        let mut gb = RustBoy::new();
        let anim_types = [
            AnimationType::Loop,
            AnimationType::PingPong,
            AnimationType::Once,
            AnimationType::Loop,
        ];
        for name in ["Alpha", "Bravo", "Charlie"] {
            let sprite = gb.add_sprite(name, tiles(4), 16, 16, 0);
            for (anim, anim_type) in ["Up", "Down", "Left", "Right"].iter().zip(&anim_types) {
                gb.sprites
                    .add_animation(sprite, anim, 0, 3, anim_type.clone());
            }
        }
        let delta = gb.add_sprite("Delta", tiles(4), 32, 16, 0);
        for i in 0..16 {
            gb.sprites
                .add_animation(delta, &format!("Pose{}", i), 0, 3, AnimationType::Loop);
        }

        let dispatch = gb.sprites.generate_animation_calls(gb.labels(), 8);
        assert_eq!(jr_range_errors(&dispatch), Vec::<String>::new());
        for (name, body) in gb.sprites.generate_animation_functions() {
            assert_eq!(jr_range_errors(&body), Vec::<String>::new(), "{}", name);
        }
        assert_labels_ok(&gb.build().unwrap());
    }
}
