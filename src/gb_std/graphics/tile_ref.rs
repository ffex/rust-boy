use crate::gb_asm::{Asm, Instr, Operand, Register};

/// A reference to a tile position in the tilemap.
///
/// This struct provides methods to manipulate tiles in the Game Boy's
/// tilemap (located at $9800-$9BFF for tilemap 0, $9C00-$9FFF for tilemap 1).
///
/// The tilemap is a 32x32 grid where each byte represents a tile index.
/// Screen visible area is 20x18 tiles (160x144 pixels).
pub struct TileRef {
    /// The tilemap address (e.g., $9800 + offset)
    pub tilemap_addr: u16,
}

impl TileRef {
    /// Create a new TileRef at the specified tilemap address.
    ///
    /// # Arguments
    /// * `tilemap_addr` - The VRAM address in the tilemap (typically $9800-$9BFF)
    pub fn new(tilemap_addr: u16) -> Self {
        TileRef { tilemap_addr }
    }

    /// Create a TileRef from X,Y coordinates in the tilemap.
    ///
    /// # Arguments
    /// * `x` - X position (0-31)
    /// * `y` - Y position (0-31)
    /// * `tilemap_base` - Base address of tilemap (default $9800)
    pub fn from_coords(x: u8, y: u8, tilemap_base: u16) -> Self {
        let offset = (y as u16 * 32) + x as u16;
        TileRef {
            tilemap_addr: tilemap_base + offset,
        }
    }

    /// Create a TileRef from X,Y coordinates using the default tilemap ($9800).
    pub fn from_xy(x: u8, y: u8) -> Self {
        Self::from_coords(x, y, 0x9800)
    }

    /// Load the tilemap address into HL register.
    /// This must be called before using other TileRef methods that operate on [HL].
    pub fn load_address(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_hl(self.tilemap_addr);
        asm.get_main_instrs()
    }

    /// Load the tilemap address into HL using a label.
    pub fn load_address_label(label: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_hl_label(label);
        asm.get_main_instrs()
    }

    /// Set the tile at the current HL address to a specific tile index.
    /// Assumes HL already contains the tilemap address.
    ///
    /// # Arguments
    /// * `tile_index` - The tile index to write (0-255)
    pub fn set_tile(tile_index: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a(tile_index)
            .ld(Operand::AddrReg(Register::HL), Operand::Reg(Register::A));
        asm.get_main_instrs()
    }

    /// Set the tile at the current HL address using a label/constant.
    /// Assumes HL already contains the tilemap address.
    ///
    /// # Arguments
    /// * `const_name` - The label or constant name for the tile index
    pub fn set_tile_label(const_name: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld(
            Operand::AddrReg(Register::HL),
            Operand::Label(const_name.to_owned()),
        );
        asm.get_main_instrs()
    }

    /// Set the tile at this TileRef's address to a specific tile index.
    /// This loads HL with the address and then sets the tile.
    ///
    /// # Arguments
    /// * `tile_index` - The tile index to write (0-255)
    pub fn set_tile_at(&self, tile_index: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_hl(self.tilemap_addr)
            .ld_a(tile_index)
            .ld(Operand::AddrReg(Register::HL), Operand::Reg(Register::A));
        asm.get_main_instrs()
    }

    /// Move to the next tile in the tilemap (increment HL).
    /// Used to move right in the tilemap or to the next row after 32 tiles.
    pub fn next_tile() -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.inc(Operand::Reg(Register::HL));
        asm.get_main_instrs()
    }

    /// Move to the previous tile in the tilemap (decrement HL).
    /// Used to move left in the tilemap.
    pub fn prev_tile() -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.dec(Operand::Reg(Register::HL));
        asm.get_main_instrs()
    }

    /// Get the current tile index at [HL] into register A.
    pub fn get_tile() -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_reg(Register::HL);
        asm.get_main_instrs()
    }

    /// Set tile and move to next (using ld [hl+], a pattern).
    /// Writes the tile index and increments HL in one operation.
    ///
    /// # Arguments
    /// * `tile_index` - The tile index to write (0-255)
    pub fn set_tile_and_next(tile_index: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a(tile_index)
            .ld(Operand::AddrRegInc(Register::HL), Operand::Reg(Register::A));
        asm.get_main_instrs()
    }

    /// Move down one row in the tilemap (add 32 to HL).
    /// The tilemap is 32 tiles wide, so adding 32 moves to the same X position
    /// on the next row.
    pub fn next_row() -> Vec<Instr> {
        let mut asm = Asm::new();
        // ld de, 32
        // add hl, de
        asm.ld_de(32)
            .add(Operand::Reg(Register::HL), Operand::Reg(Register::DE));
        asm.get_main_instrs()
    }

    /// Move up one row in the tilemap (subtract 32 from HL).
    pub fn prev_row() -> Vec<Instr> {
        let mut asm = Asm::new();
        // To subtract 32, we add -32 (0xFFE0 in 16-bit two's complement)
        // ld de, -32 (which is $FFE0)
        // add hl, de
        asm.ld_de(0xFFE0)
            .add(Operand::Reg(Register::HL), Operand::Reg(Register::DE));
        asm.get_main_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_ref_from_coords() {
        let tile_ref = TileRef::from_xy(5, 3);
        // y * 32 + x = 3 * 32 + 5 = 101
        // $9800 + 101 = $9865
        assert_eq!(tile_ref.tilemap_addr, 0x9865);
    }

    #[test]
    fn test_tile_ref_from_coords_origin() {
        let tile_ref = TileRef::from_xy(0, 0);
        assert_eq!(tile_ref.tilemap_addr, 0x9800);
    }
}
