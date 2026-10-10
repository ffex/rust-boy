//! Memory allocation for Game Boy memory regions

use crate::hw;

use super::error::Error;

/// The end (exclusive) of the HRAM `RustBoy` gives to variables: $FF80-$FFBF (64 bytes)
///
/// The stack is where the boot ROM puts it, `SP` = $FFFE (`RustBoy` does not move it), and
/// it grows down into HRAM: the top 63 bytes ($FFC0-$FFFE) are left to it, 31 nested
/// calls or pushes.
pub const HRAM_VARIABLES_END: u16 = hw::HRAM.value + 64;

/// Memory regions on the Game Boy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemoryRegion {
    /// Video RAM for tiles ($8000-$97FF for tile data)
    Vram,
    /// Work RAM ($C000-$DFFF)
    Wram,
    /// OAM for sprites ($FE00-$FE9F)
    Oam,
    /// The sprite tiles in VRAM ($8000-$8FFF, tile indices 0 to 255)
    SpriteTiles,
    /// The background tiles in VRAM as `RustBoy` uses them ($9000-$97FF, indices 0 to
    /// 127), before the tilemaps
    BackgroundTiles,
    /// WRAM bank 0 ($C000-$CFFF): a `WRAM0` section must fit in it
    Wram0,
    /// HRAM as `RustBoy` gives it to variables ($FF80-$FFBF): the stack has the rest
    /// (see [`HRAM_VARIABLES_END`])
    Hram,
}

impl MemoryRegion {
    /// Get the start address of this memory region
    pub fn start_address(&self) -> u16 {
        match self {
            MemoryRegion::Vram | MemoryRegion::SpriteTiles => hw::VRAM_OBJ_TILES,
            MemoryRegion::BackgroundTiles => hw::VRAM_BG_TILES,
            MemoryRegion::Wram | MemoryRegion::Wram0 => hw::RAM.value,
            MemoryRegion::Oam => hw::OAMRAM.value,
            MemoryRegion::Hram => hw::HRAM.value,
        }
    }

    /// Get the end address of this memory region (exclusive)
    pub fn end_address(&self) -> u16 {
        match self {
            // Tile data ends here, tilemap starts
            MemoryRegion::Vram | MemoryRegion::BackgroundTiles => hw::VRAM_BG_TILES_END,
            MemoryRegion::SpriteTiles => hw::VRAM_OBJ_TILES_END,
            MemoryRegion::Wram => hw::WRAM_END,
            MemoryRegion::Wram0 => hw::WRAM0_END,
            MemoryRegion::Oam => hw::OAM_END,
            MemoryRegion::Hram => HRAM_VARIABLES_END,
        }
    }

    /// The size of the region in bytes
    pub fn size(&self) -> u16 {
        self.end_address() - self.start_address()
    }
}

/// Allocator for tracking memory usage in a region
///
/// The tile manager allocates the sprite and background tiles with it and the sprite
/// manager the OAM entries, when they are added (B17); `build()` lays out the WRAM0 and
/// HRAM variables with it, and puts their sections at the addresses it gives.
#[derive(Debug)]
pub struct MemoryAllocator {
    region: MemoryRegion,
    next_address: u16,
}

impl MemoryAllocator {
    /// Create a new allocator for the given region
    pub fn new(region: MemoryRegion) -> Self {
        Self {
            region,
            next_address: region.start_address(),
        }
    }

    /// Allocate bytes and return the start address
    /// Returns None if allocation would exceed region bounds
    pub fn allocate(&mut self, size: u16) -> Option<u16> {
        let addr = self.next_address;
        let new_next = self.next_address.checked_add(size)?;

        if new_next > self.region.end_address() {
            return None;
        }

        self.next_address = new_next;
        Some(addr)
    }

    /// Allocate `size` bytes for `what` (e.g. ``variable `wScore` (2 bytes)``) and return
    /// the start address, or [`Error::MemoryFull`] (naming `what`, the region and how many
    /// bytes are left) if they do not fit; nothing is allocated then
    pub(crate) fn try_allocate(&mut self, size: usize, what: &str) -> Result<u16, Error> {
        u16::try_from(size)
            .ok()
            .and_then(|size| self.allocate(size))
            .ok_or_else(|| Error::MemoryFull {
                region: self.region,
                what: what.to_string(),
                needed: size,
                available: self.bytes_remaining().into(),
            })
    }

    /// [`try_allocate`](Self::try_allocate) for an allocation the call that asks for it
    /// needs at once (VRAM tiles, OAM entries)
    ///
    /// # Panics
    /// If they do not fit in the region, with the message of [`Error::MemoryFull`].
    pub(crate) fn allocate_or_panic(&mut self, size: usize, what: &str) -> u16 {
        self.try_allocate(size, what)
            .unwrap_or_else(|error| panic!("{}", error))
    }

    /// Get the current allocation pointer
    pub fn current_address(&self) -> u16 {
        self.next_address
    }

    /// Get how many bytes have been allocated
    #[allow(dead_code)] // kept for the Phase 2 allocators
    pub fn bytes_allocated(&self) -> u16 {
        self.next_address - self.region.start_address()
    }

    /// Get how many bytes remain available
    pub fn bytes_remaining(&self) -> u16 {
        self.region.end_address() - self.next_address
    }

    /// Format address as hex string for assembly
    pub fn format_address(addr: u16) -> String {
        format!("${:04X}", addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vram_allocation() {
        let mut alloc = MemoryAllocator::new(MemoryRegion::Vram);

        // Allocate 16 bytes (one tile)
        let addr1 = alloc.allocate(16).unwrap();
        assert_eq!(addr1, 0x8000);

        let addr2 = alloc.allocate(16).unwrap();
        assert_eq!(addr2, 0x8010);

        assert_eq!(alloc.bytes_allocated(), 32);
    }

    #[test]
    fn test_wram_allocation() {
        let mut alloc = MemoryAllocator::new(MemoryRegion::Wram);

        let addr = alloc.allocate(1).unwrap();
        assert_eq!(addr, 0xC000);

        let addr2 = alloc.allocate(2).unwrap();
        assert_eq!(addr2, 0xC001);
    }

    #[test]
    fn test_format_address() {
        assert_eq!(MemoryAllocator::format_address(0x8000), "$8000");
        assert_eq!(MemoryAllocator::format_address(0x9000), "$9000");
    }

    #[test]
    fn test_regions() {
        let regions = [
            (MemoryRegion::SpriteTiles, 0x8000, 0x1000),
            (MemoryRegion::BackgroundTiles, 0x9000, 0x800),
            (MemoryRegion::Wram0, 0xC000, 0x1000),
            (MemoryRegion::Oam, 0xFE00, 0xA0),
            (MemoryRegion::Hram, 0xFF80, 0x40),
        ];
        for (region, start, size) in regions {
            assert_eq!((region.start_address(), region.size()), (start, size));
        }
    }

    #[test]
    fn test_allocation_stops_at_the_end_of_the_region() {
        let mut alloc = MemoryAllocator::new(MemoryRegion::Oam);
        for i in 0..40 {
            assert_eq!(alloc.allocate(4), Some(0xFE00 + 4 * i));
        }
        assert_eq!(alloc.allocate(1), None);
        assert_eq!(alloc.bytes_remaining(), 0);
        // A size past u16 never fits
        let mut alloc = MemoryAllocator::new(MemoryRegion::Wram);
        let message = crate::engine::panic_message(|| alloc.allocate_or_panic(70_000, "data"));
        assert!(
            message.contains("no room for data: 70000 bytes needed"),
            "{}",
            message
        );
    }
}
