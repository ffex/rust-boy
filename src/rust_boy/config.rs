//! The settings of a `RustBoy` program: [`RustBoyConfig`]

use std::collections::BTreeSet;

use super::functions::BuiltinFunction;
use super::sprites::SpriteSize;
use super::tiles::TilemapArea;

/// The settings of a [`RustBoy`](super::RustBoy) program, given to
/// [`RustBoy::with_config`](super::RustBoy::with_config)
///
/// [`RustBoyConfig::default()`] is what [`RustBoy::new`](super::RustBoy::new) uses, and
/// gives the same program as before the configuration existed. Change it with the builder
/// methods (one per field), or set the fields of a default value; the `RustBoy` setters
/// (`set_sprite_size`, `set_background_tilemap`, `set_palettes`, `set_animation_delay`,
/// `use_function`) change the same settings later.
///
/// # Example
/// ```
/// use rust_boy::rust_boy::{
///     BuiltinFunction, Palettes, RustBoy, RustBoyConfig, SpriteSize, TilemapArea,
/// };
///
/// let config = RustBoyConfig::default()
///     .sprite_size(SpriteSize::Size8x16)
///     .background_tilemap(TilemapArea::Map9C00)
///     .palettes(Palettes::default().obp1(0b0001_1011))
///     .builtin(BuiltinFunction::Delay);
/// let gb = RustBoy::with_config(config);
/// let out = gb.build()?;
/// assert!(out.contains("LCDCF_OBJ16 | LCDCF_BG9C00"));
/// assert!(out.contains("Delay:"));
/// # Ok::<(), rust_boy::rust_boy::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RustBoyConfig {
    /// The size of every sprite (LCDC bit 2, `LCDCF_OBJ8` / `LCDCF_OBJ16`); 8x8 by
    /// default, as on the hardware. See
    /// [`RustBoy::set_sprite_size`](super::RustBoy::set_sprite_size).
    pub sprite_size: SpriteSize,
    /// The tilemap the background shows (LCDC bit 3, `LCDCF_BG9C00` for `$9C00`); `$9800`
    /// by default, as on the hardware
    pub background_tilemap: TilemapArea,
    /// The palettes the start-up code writes to `rBGP`, `rOBP0` and `rOBP1`
    pub palettes: Palettes,
    /// The other LCDC flags the start-up code sets when it turns the LCD on
    pub lcdc: Lcdc,
    /// Builtins emitted even if no code calls them (what
    /// [`RustBoy::use_function`](super::RustBoy::use_function) adds); none by default:
    /// `build()` emits the builtins the program uses
    pub builtins: BTreeSet<BuiltinFunction>,
    /// The animations go to their next frame every `animation_delay` frames; 8 by
    /// default (about 7.5 frames a second at 60 Hz)
    pub animation_delay: u8,
}

impl Default for RustBoyConfig {
    fn default() -> Self {
        RustBoyConfig {
            sprite_size: SpriteSize::default(),
            background_tilemap: TilemapArea::default(),
            palettes: Palettes::default(),
            lcdc: Lcdc::default(),
            builtins: BTreeSet::new(),
            animation_delay: 8,
        }
    }
}

impl RustBoyConfig {
    /// The default configuration ([`RustBoyConfig::default`])
    pub fn new() -> Self {
        Self::default()
    }

    /// With every sprite of size `size`
    pub fn sprite_size(mut self, size: SpriteSize) -> Self {
        self.sprite_size = size;
        self
    }

    /// With the background showing the tilemap at `area`
    pub fn background_tilemap(mut self, area: TilemapArea) -> Self {
        self.background_tilemap = area;
        self
    }

    /// With the palettes `palettes`
    pub fn palettes(mut self, palettes: Palettes) -> Self {
        self.palettes = palettes;
        self
    }

    /// With the LCDC flags `lcdc`
    pub fn lcdc(mut self, lcdc: Lcdc) -> Self {
        self.lcdc = lcdc;
        self
    }

    /// With the builtin `builtin` emitted even if no code calls it
    pub fn builtin(mut self, builtin: BuiltinFunction) -> Self {
        self.builtins.insert(builtin);
        self
    }

    /// With the animations going to their next frame every `delay` frames
    pub fn animation_delay(mut self, delay: u8) -> Self {
        self.animation_delay = delay;
        self
    }
}

/// The three palettes of the DMG, each a byte of four 2-bit shades (bits 1-0 for colour
/// 0, ..., bits 7-6 for colour 3; shade 0 is the lightest, 3 the darkest)
///
/// By default each is `%11100100`: colour i shows shade i, so the two object palettes look
/// like the background one until the program changes them. The start-up code writes them
/// before the user's `init()` code, which can change them again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palettes {
    /// `rBGP`, the background (and window) palette
    pub bgp: u8,
    /// `rOBP0`, the palette of the sprites without the `OAMF_PAL1` flag (colour 0 is
    /// transparent)
    pub obp0: u8,
    /// `rOBP1`, the palette of the sprites with the `OAMF_PAL1` flag
    pub obp1: u8,
}

impl Palettes {
    /// Colour i shows shade i: `%11100100`
    pub const IDENTITY: u8 = 0b1110_0100;

    /// The three palettes set to `palette`
    pub fn all(palette: u8) -> Self {
        Palettes {
            bgp: palette,
            obp0: palette,
            obp1: palette,
        }
    }

    /// With the background palette `palette`
    pub fn bgp(mut self, palette: u8) -> Self {
        self.bgp = palette;
        self
    }

    /// With the first object palette `palette`
    pub fn obp0(mut self, palette: u8) -> Self {
        self.obp0 = palette;
        self
    }

    /// With the second object palette `palette`
    pub fn obp1(mut self, palette: u8) -> Self {
        self.obp1 = palette;
        self
    }
}

impl Default for Palettes {
    /// [`Palettes::IDENTITY`] for all three
    fn default() -> Self {
        Palettes::all(Palettes::IDENTITY)
    }
}

/// The LCDC flags the start-up code sets when it turns the LCD on (`LCDCF_ON` always),
/// besides the sprite size and the background tilemap, which have their own settings
///
/// The tile data the background uses is not a setting: `RustBoy` puts the background
/// tiles at `$9000` (`LCDCF_BG8800`, LCDC bit 4 clear). The window has no setting yet
/// (Phase 3): it stays off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Lcdc {
    /// Show the background (`LCDCF_BGON`, LCDC bit 0); on by default
    pub background: bool,
    /// Show the sprites (`LCDCF_OBJON`, LCDC bit 1); on by default
    pub objects: bool,
}

impl Default for Lcdc {
    fn default() -> Self {
        Lcdc {
            background: true,
            objects: true,
        }
    }
}

impl Lcdc {
    /// With the background shown, or not
    pub fn background(mut self, on: bool) -> Self {
        self.background = on;
        self
    }

    /// With the sprites shown, or not
    pub fn objects(mut self, on: bool) -> Self {
        self.objects = on;
        self
    }
}
