//! The settings of a `RustBoy` program: [`RustBoyConfig`]

use std::collections::BTreeSet;

use crate::stdlib::audio::AudioConfig;
use crate::stdlib::graphics::palette::Palette;
use crate::stdlib::interrupts::{Stat, Timer};

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
/// `use_function`, `set_soft_reset`, and `on_stat` / `on_timer` for `stat` / `timer`)
/// change the same settings later.
///
/// # Example
/// ```
/// use rust_boy::engine::{
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
/// # Ok::<(), rust_boy::engine::Error>(())
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
    /// Optional window configuration; disabled by default.
    pub window: Option<Window>,
    /// The other LCDC flags the start-up code sets when it turns the LCD on
    pub lcdc: Lcdc,
    /// Builtins emitted even if no code calls them (what
    /// [`RustBoy::use_function`](super::RustBoy::use_function) adds); none by default:
    /// `build()` emits the builtins the program uses
    pub builtins: BTreeSet<BuiltinFunction>,
    /// The animations go to their next frame every `animation_delay` frames; 8 by
    /// default (about 7.5 frames a second at 60 Hz)
    pub animation_delay: u8,
    /// Reset the game when A, B, Select and Start are held together (see
    /// [`RustBoy::set_soft_reset`](super::RustBoy::set_soft_reset)); off by default
    pub soft_reset: bool,
    /// The conditions of the STAT interrupt (`rSTAT`, `rLYC`) the start-up code selects;
    /// none by default. [`RustBoy::on_stat`](super::RustBoy::on_stat) sets it with its
    /// handler.
    pub stat: Option<Stat>,
    /// The timer the start-up code starts; stopped by default.
    /// [`RustBoy::on_timer`](super::RustBoy::on_timer) sets it with its handler.
    pub timer: Option<Timer>,
    /// APU startup policy; Inherit leaves legacy boot state unchanged.
    pub audio: AudioConfig,
}

impl Default for RustBoyConfig {
    fn default() -> Self {
        RustBoyConfig {
            sprite_size: SpriteSize::default(),
            background_tilemap: TilemapArea::default(),
            palettes: Palettes::default(),
            window: None,
            lcdc: Lcdc::default(),
            builtins: BTreeSet::new(),
            animation_delay: 8,
            soft_reset: false,
            stat: None,
            timer: None,
            audio: AudioConfig::Inherit,
        }
    }
}

impl RustBoyConfig {
    /// The default configuration ([`RustBoyConfig::default`])
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure audio before user init, including on soft reset.
    pub fn audio(mut self, audio: AudioConfig) -> Self {
        self.audio = audio;
        self
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

    /// Show a window at startup. Position writes precede user init code.
    pub fn window(mut self, window: Window) -> Self {
        self.window = Some(window);
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

    /// With the soft reset (A + B + Select + Start) on, or off
    pub fn soft_reset(mut self, on: bool) -> Self {
        self.soft_reset = on;
        self
    }

    /// With the STAT interrupt's conditions `stat` selected at start-up
    pub fn stat(mut self, stat: Stat) -> Self {
        self.stat = Some(stat);
        self
    }

    /// With `timer` started at start-up
    pub fn timer(mut self, timer: Timer) -> Self {
        self.timer = Some(timer);
        self
    }
}

/// The three palettes of the DMG, each a byte of four 2-bit shades (bits 1-0 for colour
/// 0, ..., bits 7-6 for colour 3; shade 0 is the lightest, 3 the darkest)
///
/// By default each is `%11100100`: colour i shows shade i, so the two object palettes look
/// like the background one until the program changes them. The start-up code writes them
/// before the user's `init()` code, which can change them again. Builders accept typed
/// [`Palette`] values or raw `u8` hardware bytes.
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
    pub fn all(palette: impl Into<Palette>) -> Self {
        let palette = palette.into().byte();
        Palettes {
            bgp: palette,
            obp0: palette,
            obp1: palette,
        }
    }

    /// With the background palette `palette`
    pub fn bgp(mut self, palette: impl Into<Palette>) -> Self {
        self.bgp = palette.into().byte();
        self
    }

    /// With the first object palette `palette`
    pub fn obp0(mut self, palette: impl Into<Palette>) -> Self {
        self.obp0 = palette.into().byte();
        self
    }

    /// With the second object palette `palette`
    pub fn obp1(mut self, palette: impl Into<Palette>) -> Self {
        self.obp1 = palette.into().byte();
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
/// tiles at `$9000` (`LCDCF_BG8800`, LCDC bit 4 clear). The window shares this tile data mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Lcdc {
    /// Show the background and window on DMG (`LCDCF_BGON`, LCDC bit 0); on by default
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

/// Startup window settings. Uses the background tiles and palette.
/// Populate its map with `tiles.add_tilemap_at`; the two layers may share a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// Window map, independent of the background map.
    pub tilemap: TilemapArea,
    /// Raw hardware X coordinate (screen X + 7).
    pub wx: u8,
    /// Raw hardware Y coordinate.
    pub wy: u8,
}

impl Window {
    /// Visible window at screen `(x, y)`. X must be 0..=158, Y 0..=143
    /// (see [`crate::stdlib::graphics::window::screen_to_raw`]).
    ///
    /// # Panics
    /// If X is not 0..=158 or Y not 0..=143.
    ///
    /// ```
    /// use rust_boy::prelude::*;
    /// let mut gb = RustBoy::with_config(RustBoyConfig::default()
    ///     .window(Window::new(TilemapArea::Map9C00, 0, 112)));
    /// gb.tiles.add_tilemap_at("Status", TilemapArea::Map9C00, &[[0; 32]]);
    /// // It starts at Y 112; VBlank code moves it to Y 120 and switches its map
    /// let mut frame = gb.set_window_position(0, 120);
    /// frame.extend(gb.select_window_tilemap(TilemapArea::Map9800));
    /// gb.on_vblank(frame);
    /// # gb.build().unwrap();
    /// ```
    #[track_caller]
    pub fn new(tilemap: TilemapArea, x: u8, y: u8) -> Self {
        let (wx, wy) = crate::stdlib::graphics::window::screen_to_raw(x, y);
        Self { tilemap, wx, wy }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_new_converts_and_checks_screen_coordinates() {
        let window = Window::new(TilemapArea::Map9C00, 158, 143);
        assert_eq!((window.wx, window.wy), (165, 143));
        assert_eq!(Window::new(TilemapArea::Map9800, 0, 0).wx, 7);
        for (x, y) in [(159, 0), (0, 144)] {
            assert!(std::panic::catch_unwind(|| Window::new(TilemapArea::Map9800, x, y)).is_err());
        }
    }

    #[test]
    fn startup_palettes_accept_typed_and_raw_values() {
        let typed = Palette::solid(crate::stdlib::graphics::palette::Shade::Dark);
        assert_eq!(Palettes::all(typed).bgp, typed.byte());
        assert_eq!(
            Palettes::default().bgp(0x1Bu8).obp1(Palette::from(0)).obp1,
            0
        );
    }
}
