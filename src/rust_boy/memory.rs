//! Memory allocation for Game Boy memory regions

/// Memory regions on the Game Boy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegion {
    /// Video RAM for tiles ($8000-$97FF for tile data)
    Vram,
    /// Work RAM ($C000-$DFFF)
    Wram,
    /// OAM for sprites ($FE00-$FE9F)
    Oam,
}

impl MemoryRegion {
    /// Get the start address of this memory region
    pub fn start_address(&self) -> u16 {
        match self {
            MemoryRegion::Vram => 0x8000,
            MemoryRegion::Wram => 0xC000,
            MemoryRegion::Oam => 0xFE00,
        }
    }

    /// Get the end address of this memory region (exclusive)
    pub fn end_address(&self) -> u16 {
        match self {
            MemoryRegion::Vram => 0x9800, // Tile data ends here, tilemap starts
            MemoryRegion::Wram => 0xE000,
            MemoryRegion::Oam => 0xFEA0,
        }
    }
}

/// Allocator for tracking memory usage in a region
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

    /// Get the current allocation pointer
    pub fn current_address(&self) -> u16 {
        self.next_address
    }

    /// Get how many bytes have been allocated
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
}
