//! Tile management with automatic VRAM allocation

use std::collections::BTreeMap;

use crate::gb_asm::Instr;
use crate::gb_std::graphics::utility::cp_in_memory;

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

    /// Calculate the size in bytes
    pub fn size_bytes(&self) -> u16 {
        match self {
            TileSource::Raw(tiles) => (tiles.len() * 16) as u16, // 16 bytes per tile
            TileSource::File(_, tile_count) => (*tile_count * 16) as u16, // 16 bytes per tile
        }
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
    pub is_tilemap: bool, // Tilemaps go to $9800
}

/// Manages tiles with automatic VRAM allocation
#[derive(Debug)]
pub struct TileManager {
    /// Tiles by id; ids are sequential, so iteration follows creation order
    tiles: BTreeMap<TileId, TileData>,
    next_id: usize,
    // Sprite tiles: $8000-$8FFF
    next_sprite_addr: u16,
    // Background tiles: $9000-$97FF
    next_bg_addr: u16,
}

impl TileManager {
    pub(crate) fn new() -> Self {
        Self {
            tiles: BTreeMap::new(),
            next_id: 0,
            next_sprite_addr: 0x8000,
            next_bg_addr: 0x9000,
        }
    }

    /// Add sprite tiles (allocated from $8000)
    pub fn add_sprite(&mut self, name: &str, source: TileSource) -> TileId {
        check_source(name, &source);
        let size = source.size_bytes();
        let addr = self.next_sprite_addr;
        self.next_sprite_addr += size;

        let id = TileId(self.next_id);
        self.next_id += 1;

        self.tiles.insert(
            id,
            TileData {
                name: name.to_string(),
                source,
                vram_address: addr,
                is_sprite: true,
                is_tilemap: false,
            },
        );

        id
    }

    /// Add background tiles (allocated from $9000)
    pub fn add_background(&mut self, name: &str, source: TileSource) -> TileId {
        check_source(name, &source);
        let size = source.size_bytes();
        let addr = self.next_bg_addr;
        self.next_bg_addr += size;

        let id = TileId(self.next_id);
        self.next_id += 1;

        self.tiles.insert(
            id,
            TileData {
                name: name.to_string(),
                source,
                vram_address: addr,
                is_sprite: false,
                is_tilemap: false,
            },
        );

        id
    }

    /// Add a tilemap (goes to $9800)
    pub fn add_tilemap(&mut self, name: &str, tilemap: &[[u8; 32]]) -> TileId {
        let id = TileId(self.next_id);
        self.next_id += 1;

        // Store tilemap data as raw bytes converted to hex
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

        self.tiles.insert(
            id,
            TileData {
                name: name.to_string(),
                source: TileSource::Raw(converted),
                vram_address: 0x9800,
                is_sprite: false,
                is_tilemap: true,
            },
        );

        id
    }

    /// Get the VRAM address for a tile
    pub fn get_address(&self, id: TileId) -> Option<u16> {
        self.tiles.get(&id).map(|t| t.vram_address)
    }

    /// Get the label name for a tile
    pub fn get_label(&self, id: TileId) -> Option<&str> {
        self.tiles.get(&id).map(|t| t.name.as_str())
    }

    /// Generate tile data instructions for the Tiles chunk
    pub(crate) fn generate_tile_data(&self) -> Vec<Instr> {
        use crate::gb_asm::Asm;

        let mut asm = Asm::new();

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

        asm.get_main_instrs()
    }

    /// Generate tilemap data instructions for the Tilemap chunk
    pub(crate) fn generate_tilemap_data(&self) -> Vec<Instr> {
        use crate::gb_asm::Asm;

        let mut asm = Asm::new();

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

        asm.get_main_instrs()
    }

    /// Generate the code that copies every blob to VRAM, in creation order
    ///
    /// An empty blob of raw data (`from_raw` with no tiles, a tilemap with no rows) is not
    /// copied: `Memcopy` copies at least one byte, so a length of 0 would copy 64 KiB
    /// (B27). Its labels are still emitted, with nothing between them. A file is always
    /// copied, whole (`from_file` rejects a tile count of 0; how many bytes the file holds
    /// is only known when it is assembled).
    pub(crate) fn generate_memcopy_calls(&self) -> Vec<Instr> {
        self.tiles
            .values()
            .filter(|tile| !matches!(&tile.source, TileSource::Raw(data) if data.is_empty()))
            .flat_map(|tile| cp_in_memory(&tile.name, &format!("${:04X}", tile.vram_address)))
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
