//! Tile management with automatic VRAM allocation

use std::collections::BTreeMap;

use crate::gb_asm::{Expr, Instr};
use crate::gb_std::graphics::utility::cp_in_memory;
use crate::hw;

use super::memory::{MemoryAllocator, MemoryRegion};

/// Unique identifier for a tile or tileset
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId(pub(crate) usize);

/// Source data for tiles
#[derive(Debug, Clone)]
pub enum TileSource {
    /// Raw tile data as 2D array of hex strings (legacy format)
    Raw(Vec<[String; 8]>),
    /// Binary file path (.2bpp format) with tile count
    File(String, usize),
}

impl TileSource {
    /// Create from the legacy format used in unbricked
    pub fn from_raw(data: &[[&str; 8]]) -> Self {
        let converted: Vec<[String; 8]> = data
            .iter()
            .map(|tile| {
                let mut arr: [String; 8] = Default::default();
                for (i, line) in tile.iter().enumerate() {
                    arr[i] = line.to_string();
                }
                arr
            })
            .collect();
        TileSource::Raw(converted)
    }

    /// Create from a .2bpp file path with tile count
    /// Each tile is 16 bytes in 2bpp format
    ///
    /// # Panics
    /// If `tile_count` is 0: the whole file is included and copied to VRAM, so its tile
    /// count must be the number of tiles in it (VRAM is allocated from the count).
    pub fn from_file(path: &str, tile_count: usize) -> Self {
        assert!(
            tile_count > 0,
            "TileSource::from_file(\"{}\", 0): a tile file needs a tile count of at least 1 \
             (the number of 16-byte tiles in the file)",
            path
        );
        TileSource::File(path.to_string(), tile_count)
    }

    /// Calculate the size in bytes (16 bytes per tile)
    ///
    /// # Panics
    /// If it is more than 65535 bytes (4096 tiles or more): no VRAM region holds that.
    pub fn size_bytes(&self) -> u16 {
        u16::try_from(self.byte_len()).unwrap_or_else(|_| {
            panic!(
                "{} tiles are {} bytes, more than any VRAM region",
                self.tile_count(),
                self.byte_len()
            )
        })
    }

    /// The size in bytes, without a limit
    fn byte_len(&self) -> usize {
        self.tile_count() * usize::from(hw::TILE_SIZE)
    }

    /// Get the number of tiles
    pub fn tile_count(&self) -> usize {
        match self {
            TileSource::Raw(tiles) => tiles.len(),
            TileSource::File(_, tile_count) => *tile_count,
        }
    }
}

/// Panics if `source` is a file with a tile count of 0 (also built directly as
/// `TileSource::File`): a file is copied whole, so its count cannot say "empty" (B27)
fn check_source(name: &str, source: &TileSource) {
    // `build()` writes the name as a label and as a symbol (`ld de, name`)
    super::sprites::check_label("tile", name);
    if let TileSource::File(path, 0) = source {
        panic!(
            "tiles \"{}\": the file \"{}\" has a tile count of 0; a tile file needs a tile \
             count of at least 1 (the number of 16-byte tiles in the file)",
            name, path
        );
    }
}

/// Internal tile data stored by TileManager
#[derive(Debug, Clone)]
pub(crate) struct TileData {
    pub name: String,
    pub source: TileSource,
    pub vram_address: u16,
    pub is_sprite: bool,  // Sprites go to $8000, background to $9000
    pub is_tilemap: bool, // Tilemaps go to $9800 or $9C00
}

/// One of the two background tilemaps in VRAM, 32 x 32 tiles each (B19)
///
/// The background shows one of them (LCDC bit 3, `RustBoy::set_background_tilemap`);
/// the window layer, not supported yet, can show the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TilemapArea {
    /// The map at `$9800` (`_SCRN0`), the one the background shows by default
    #[default]
    Map9800,
    /// The map at `$9C00` (`_SCRN1`)
    Map9C00,
}

impl TilemapArea {
    /// The VRAM address of the map's first tile
    pub fn address(self) -> u16 {
        match self {
            TilemapArea::Map9800 => hw::SCRN0.value,
            TilemapArea::Map9C00 => hw::SCRN1.value,
        }
    }

    /// The `hardware.inc` LCDC flag that makes the background show this map
    pub(crate) fn lcdc_bg_flag(self) -> hw::Symbol<u8> {
        match self {
            TilemapArea::Map9800 => hw::LCDCF_BG9800,
            TilemapArea::Map9C00 => hw::LCDCF_BG9C00,
        }
    }
}

impl std::fmt::Display for TilemapArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", MemoryAllocator::format_address(self.address()))
    }
}

/// Manages tiles with automatic VRAM allocation
#[derive(Debug)]
pub struct TileManager {
    /// Tiles by id; ids are sequential, so iteration follows creation order
    tiles: BTreeMap<TileId, TileData>,
    next_id: usize,
    /// Sprite tiles: $8000-$8FFF, 256 tiles (B17)
    sprite_tiles: MemoryAllocator,
    /// Background tiles: $9000-$97FF, 128 tiles, then the tilemaps (B17)
    background_tiles: MemoryAllocator,
}

impl TileManager {
    pub(crate) fn new() -> Self {
        Self {
            tiles: BTreeMap::new(),
            next_id: 0,
            sprite_tiles: MemoryAllocator::new(MemoryRegion::SpriteTiles),
            background_tiles: MemoryAllocator::new(MemoryRegion::BackgroundTiles),
        }
    }

    /// Add sprite tiles (allocated from $8000)
    ///
    /// # Panics
    /// If `name` cannot be a label (an RGBDS identifier, not a register or keyword), if
    /// they do not fit in the 256 sprite tiles ($8000-$8FFF) with the ones already added,
    /// or `source` is a file with a tile count of 0.
    pub fn add_sprite(&mut self, name: &str, source: TileSource) -> TileId {
        check_source(name, &source);
        let what = format!("sprite tiles \"{}\" ({} tiles)", name, source.tile_count());
        let addr = self
            .sprite_tiles
            .allocate_or_panic(source.byte_len(), &what);
        self.insert(name, source, addr, true, false)
    }

    /// Add background tiles (allocated from $9000)
    ///
    /// # Panics
    /// If `name` cannot be a label (an RGBDS identifier, not a register or keyword), if
    /// they do not fit in the 128 background tiles ($9000-$97FF, before the tilemaps) with
    /// the ones already added, or `source` is a file with a tile count of 0.
    pub fn add_background(&mut self, name: &str, source: TileSource) -> TileId {
        check_source(name, &source);
        let what = format!(
            "background tiles \"{}\" ({} tiles)",
            name,
            source.tile_count()
        );
        let addr = self
            .background_tiles
            .allocate_or_panic(source.byte_len(), &what);
        self.insert(name, source, addr, false, false)
    }

    /// Store a blob under a new id
    fn insert(
        &mut self,
        name: &str,
        source: TileSource,
        vram_address: u16,
        is_sprite: bool,
        is_tilemap: bool,
    ) -> TileId {
        let id = TileId(self.next_id);
        self.next_id += 1;
        self.tiles.insert(
            id,
            TileData {
                name: name.to_string(),
                source,
                vram_address,
                is_sprite,
                is_tilemap,
            },
        );
        id
    }

    /// Add a tilemap at `$9800`, the map the background shows by default; see
    /// [`TileManager::add_tilemap_at`]
    ///
    /// # Panics
    /// If `$9800` already has a tilemap, or `tilemap` has more than 32 rows.
    pub fn add_tilemap(&mut self, name: &str, tilemap: &[[u8; 32]]) -> TileId {
        self.add_tilemap_at(name, TilemapArea::Map9800, tilemap)
    }

    /// Add a tilemap at `area`: `$9800` or `$9C00` (B19)
    ///
    /// The start-up code copies its rows to the start of that map, row `r` to
    /// `area + 32 * r`. Which map the background shows is set with
    /// `RustBoy::set_background_tilemap` (`$9800` by default).
    ///
    /// # Panics
    /// - If `name` cannot be a label (an RGBDS identifier, not a register or keyword).
    /// - If `area` already has a tilemap: each one is copied to the start of the map, so
    ///   the second would replace the first (before, every tilemap went to `$9800` and the
    ///   last one created won).
    /// - If `tilemap` has more than 32 rows: a map is 32 x 32 tiles, so a 33rd row would
    ///   run into the next map, or out of VRAM.
    pub fn add_tilemap_at(
        &mut self,
        name: &str,
        area: TilemapArea,
        tilemap: &[[u8; 32]],
    ) -> TileId {
        super::sprites::check_label("tile", name);
        if tilemap.len() > usize::from(hw::SCRN_VY_B.value) {
            panic!(
                "tilemap \"{}\" has {} rows, but a map has {}: the rows past it would run \
                 past {}",
                name,
                tilemap.len(),
                hw::SCRN_VY_B.value,
                area
            );
        }
        if let Some(other) = self
            .tiles
            .values()
            .find(|tile| tile.is_tilemap && tile.vram_address == area.address())
        {
            panic!(
                "tilemap \"{}\": the map at {} already has the tilemap \"{}\", and both would be \
                 copied there; put one of them at the other map with add_tilemap_at",
                name, area, other.name
            );
        }

        // Store tilemap data as raw bytes converted to hex, 8 rows per entry
        let converted: Vec<[String; 8]> = tilemap
            .chunks(8)
            .map(|chunk| {
                let mut arr: [String; 8] = Default::default();
                for (i, row) in chunk.iter().enumerate() {
                    let hex_values: Vec<String> =
                        row.iter().map(|&val| format!("${:02X}", val)).collect();
                    arr[i] = hex_values.join(", ");
                }
                arr
            })
            .collect();

        self.insert(
            name,
            TileSource::Raw(converted),
            area.address(),
            false,
            true,
        )
    }

    /// Get the VRAM address for a tile
    pub fn get_address(&self, id: TileId) -> Option<u16> {
        self.tiles.get(&id).map(|t| t.vram_address)
    }

    /// Get the label name for a tile
    pub fn get_label(&self, id: TileId) -> Option<&str> {
        self.tiles.get(&id).map(|t| t.name.as_str())
    }

    /// The tile index the next sprite tiles will get (256 when the sprite tiles are full)
    pub(crate) fn next_sprite_tile(&self) -> u16 {
        (self.sprite_tiles.current_address() - hw::VRAM_OBJ_TILES) / hw::TILE_SIZE
    }

    /// The tile index of the first tile of the sprite tiles `id`, from their VRAM
    /// address (`$8000 + index * 16`): the one source of sprite tile indices (B18)
    ///
    /// # Panics
    /// If `id` is not sprite tiles of this manager.
    pub(crate) fn sprite_tile_index(&self, id: TileId) -> u8 {
        let tile = self
            .tiles
            .get(&id)
            .filter(|tile| tile.is_sprite)
            .unwrap_or_else(|| panic!("tiles {:?} are not sprite tiles of this program", id));
        let index = (tile.vram_address - hw::VRAM_OBJ_TILES) / hw::TILE_SIZE;
        u8::try_from(index).unwrap_or_else(|_| {
            panic!(
                "sprite tiles \"{}\" start on tile {}, past the 256 sprite tiles",
                tile.name, index
            )
        })
    }

    /// Generate tile data instructions for the Tiles chunk
    pub(crate) fn generate_tile_data(&self) -> Vec<Instr> {
        use crate::gb_asm::Block;

        let mut asm = Block::new();

        // Generate sprite tiles first
        for tile in self.tiles.values().filter(|t| t.is_sprite && !t.is_tilemap) {
            asm.label(&tile.name);
            match &tile.source {
                TileSource::Raw(data) => {
                    for tile_data in data {
                        for line in tile_data {
                            asm.dw(line);
                        }
                    }
                }
                TileSource::File(path, _) => {
                    asm.incbin(path);
                }
            }
            asm.label(&format!("{}End", tile.name));
        }

        // Then background tiles
        for tile in self
            .tiles
            .values()
            .filter(|t| !t.is_sprite && !t.is_tilemap)
        {
            asm.label(&tile.name);
            match &tile.source {
                TileSource::Raw(data) => {
                    for tile_data in data {
                        for line in tile_data {
                            asm.dw(line);
                        }
                    }
                }
                TileSource::File(path, _) => {
                    asm.incbin(path);
                }
            }
            asm.label(&format!("{}End", tile.name));
        }

        asm.into_instrs()
    }

    /// Generate tilemap data instructions for the Tilemap chunk
    pub(crate) fn generate_tilemap_data(&self) -> Vec<Instr> {
        use crate::gb_asm::Block;

        let mut asm = Block::new();

        for tile in self.tiles.values().filter(|t| t.is_tilemap) {
            asm.label(&tile.name);
            if let TileSource::Raw(data) = &tile.source {
                for row in data {
                    for line in row {
                        if !line.is_empty() {
                            asm.db(line);
                        }
                    }
                }
            }
            asm.label(&format!("{}End", tile.name));
        }

        asm.into_instrs()
    }

    /// Generate the code that copies every blob to VRAM, in creation order
    ///
    /// An empty blob of raw data (`from_raw` with no tiles, a tilemap with no rows) is not
    /// copied: there is nothing to copy, so no code is spent on it (`Memcopy` itself copies
    /// nothing for a length of 0, B27). Its labels are still emitted, with nothing between
    /// them. A file is always copied, whole (`from_file` rejects a tile count of 0; how
    /// many bytes the file holds is only known when it is assembled).
    pub(crate) fn generate_memcopy_calls(&self) -> Vec<Instr> {
        self.tiles
            .values()
            .filter(|tile| !matches!(&tile.source, TileSource::Raw(data) if data.is_empty()))
            .flat_map(|tile| cp_in_memory(&tile.name, Expr::hex(tile.vram_address)))
            .collect()
    }

    /// Check if any tiles have been added
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_allocation() {
        let mut tm = TileManager::new();

        // Add a sprite with 1 tile (16 bytes)
        let paddle_data: [[&str; 8]; 1] =
            [["$FF", "$00", "$FF", "$00", "$FF", "$00", "$FF", "$00"]];
        let id = tm.add_sprite("Paddle", TileSource::from_raw(&paddle_data));

        assert_eq!(tm.get_address(id), Some(0x8000));
        assert_eq!(tm.get_label(id), Some("Paddle"));
    }

    #[test]
    fn test_background_allocation() {
        let mut tm = TileManager::new();

        let tiles_data: [[&str; 8]; 1] = [["$00", "$00", "$00", "$00", "$00", "$00", "$00", "$00"]];
        let id = tm.add_background("Tiles", TileSource::from_raw(&tiles_data));

        assert_eq!(tm.get_address(id), Some(0x9000));
    }
}
