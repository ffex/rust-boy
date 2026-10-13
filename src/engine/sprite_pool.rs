//! A runtime pool of reserved OAM entries, created by RustBoy::create_sprite_pool.
use crate::asm::{Block, Instr};

/// A reserved shadow-OAM range with runtime allocation, release and visibility.
/// Methods emit calls; all operations belong in init/main (one producer).
/// Invalid/free relative indices or a full pool report carry, without writing memory.
#[derive(Debug, Clone)]
pub struct SpritePool {
    pub(crate) name: String,
    pub(crate) first: u8,
    pub(crate) count: u8,
}
impl SpritePool {
    /// Pool name used in generated routine/state labels.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// First absolute OAM index reserved by this pool.
    pub fn first_slot(&self) -> u8 {
        self.first
    }
    /// Number of reserved entries.
    pub fn capacity(&self) -> u8 {
        self.count
    }
    fn call(&self, operation: &str) -> Vec<Instr> {
        let mut code = Block::new();
        code.call(&format!("Pool{}{operation}", self.name));
        code.into_instrs()
    }
    /// Claim the lowest free slot. A=relative index, HL=cleared hidden entry, carry clear.
    /// Full: A=255, carry set. Changes AF/BC/HL, preserves DE and IME.
    pub fn allocate(&self) -> Vec<Instr> {
        self.call("Allocate")
    }
    /// Release A-th slot and clear its OAM entry; double release is rejected with carry.
    /// Changes AF/HL; preserves BC/DE and IME.
    pub fn release(&self) -> Vec<Instr> {
        self.call("Release")
    }
    /// Show/update A-th allocated slot: B=X, C=Y screen pixels, D=tile, E=flags.
    /// Rejects invalid coordinates and odd tiles in 8x16 mode. Writes Y last.
    /// Changes AF/HL; preserves BC/DE and IME. A multi-byte update is not atomic to DMA.
    pub fn write(&self) -> Vec<Instr> {
        self.call("Write")
    }
    /// Hide A-th allocated slot (Y=0), keeping it allocated and its X/tile/flags.
    /// Show it again with write(). Changes AF/HL; preserves BC/DE and IME.
    pub fn hide(&self) -> Vec<Instr> {
        self.call("Hide")
    }
}
