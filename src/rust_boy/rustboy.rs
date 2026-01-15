//! Main RustBoy struct - the high-level Game Boy development API

use crate::gb_asm::{Asm, Chunk, Instr};

use super::functions::{BuiltinFunction, FunctionRegistry};
use super::sprites::SpriteManager;
use super::tiles::TileManager;
use super::variables::VariableManager;

/// High-level Game Boy development API
///
/// RustBoy completely hides assembly generation from the developer,
/// providing a clean, idiomatic Rust interface for Game Boy development.
///
/// # Example
/// ```ignore
/// let mut gb = RustBoy::new();
///
/// // Add tiles (auto-allocated to VRAM)
/// let paddle = gb.tiles.add_sprite("Paddle", TileSource::from_raw(&paddle_data));
///
/// // Add variables (auto-allocated to WRAM)
/// let score = gb.vars.create_u8("wScore", 0);
///
/// // Build the assembly
/// println!("{}", gb.build());
/// ```
pub struct RustBoy {
    /// Internal assembly generator (hidden from user)
    asm: Asm,

    /// Tile manager with automatic VRAM allocation
    pub tiles: TileManager,

    /// Variable manager with automatic WRAM allocation
    pub vars: VariableManager,

    /// Sprite manager with automatic OAM and tile handling
    pub sprites: SpriteManager,

    /// Function registry for auto-including builtin functions
    functions: FunctionRegistry,

    /// Counter for generating unique if-statement labels
    if_counter: usize,

    /// Counter for generating unique general-purpose labels
    label_counter: usize,

    /// Custom constants defined by the user
    constants: Vec<(String, String)>,

    /// Init code to run before the main loop
    init_code: Vec<Instr>,

    /// Main loop code
    main_loop_code: Vec<Instr>,
}

impl RustBoy {
    /// Create a new RustBoy instance
    pub fn new() -> Self {
        Self {
            asm: Asm::new(),
            tiles: TileManager::new(),
            vars: VariableManager::new(),
            sprites: SpriteManager::new(),
            functions: FunctionRegistry::new(),
            if_counter: 0,
            label_counter: 0,
            constants: Vec::new(),
            init_code: Vec::new(),
            main_loop_code: Vec::new(),
        }
    }

    /// Define a constant value
    pub fn define_const(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.constants
            .push((name.to_string(), format!("{}", value)));
        self
    }

    /// Define a constant with a hex value
    pub fn define_const_hex(&mut self, name: &str, value: u16) -> &mut Self {
        self.constants
            .push((name.to_string(), format!("${:04X}", value)));
        self
    }

    /// Get the next if-statement label counter (auto-increments)
    pub fn next_if_counter(&mut self) -> usize {
        let c = self.if_counter;
        self.if_counter += 1;
        c
    }

    /// Get the next general-purpose label counter (auto-increments)
    pub fn next_label_counter(&mut self) -> usize {
        let c = self.label_counter;
        self.label_counter += 1;
        c
    }

    /// Generate a unique label with prefix
    pub fn unique_label(&mut self, prefix: &str) -> String {
        format!("{}_{}", prefix, self.next_label_counter())
    }

    /// Add initialization code (runs once at startup)
    pub fn init(&mut self, code: Vec<Instr>) -> &mut Self {
        self.init_code.extend(code);
        self
    }

    /// Escape hatch: execute raw assembly operations
    ///
    /// This allows advanced users to mix high-level and low-level code.
    ///
    /// # Example
    /// ```ignore
    /// gb.raw(|asm| {
    ///     asm.ld_a(0x42);
    ///     asm.ret();
    /// });
    /// ```
    pub fn raw<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut Asm),
    {
        f(&mut self.asm);
        self
    }

    /// Mark a builtin function as used (will be auto-included)
    pub fn use_function(&mut self, func: BuiltinFunction) -> &mut Self {
        self.functions.use_function(func);
        self
    }

    /// Build the final assembly output
    pub fn build(&mut self) -> String {
        // Start fresh assembly
        let mut asm = Asm::new();

        // === HEADER CHUNK ===
        asm.chunk(Chunk::Header);
        asm.include_hardware();
        asm.emit_all(crate::gb_std::utility::header_section());

        // === CONSTANTS CHUNK ===
        asm.chunk(Chunk::Constants);
        for (name, value) in &self.constants {
            asm.def(name, value);
        }

        // === INIT CHUNK ===
        asm.chunk(Chunk::Init);

        // Entry point
        asm.label("EntryPoint");
        asm.call("WaitVBlank");
        self.functions.use_function(BuiltinFunction::WaitVBlank);

        // Turn off screen for safe VRAM access
        asm.ld_a(0);
        asm.ld_addr_def_a("rLCDC");

        // Generate tile memcopy calls (tiles need Memcopy function)
        if !self.tiles.is_empty() {
            self.functions.use_function(BuiltinFunction::Memcopy);
            asm.emit_all(self.tiles.generate_memcopy_calls());
        }

        // Initialize sprites (OAM setup)
        if !self.sprites.is_empty() {
            asm.emit_all(self.sprites.generate_init_code());
        }

        // Emit user init code
        asm.emit_all(self.init_code.clone());

        // Emit variable initialization
        asm.emit_all(self.vars.generate_init_code());

        // Turn on screen
        asm.ld_a_label("LCDCF_ON | LCDCF_BGON | LCDCF_OBJON");
        asm.ld_addr_def_a("rLCDC");

        // Set default palettes
        asm.ld_a(0b11100100);
        asm.ld_addr_def_a("rBGP");
        asm.ld_a(0b11100100);
        asm.ld_addr_def_a("rOBP0");

        // === MAIN LOOP CHUNK ===
        asm.chunk(Chunk::MainLoop);

        asm.label("Main");
        self.functions.use_function(BuiltinFunction::WaitNotVBlank);
        asm.call("WaitNotVBlank");
        asm.call("WaitVBlank");

        // Emit main loop code
        asm.emit_all(self.main_loop_code.clone());

        // Jump back to main loop
        asm.jp("Main");

        // === DATA CHUNK ===
        asm.chunk(Chunk::Data);
        asm.emit_all(self.vars.generate_sections());

        // === FUNCTIONS CHUNK ===
        asm.chunk(Chunk::Functions);
        asm.emit_all(self.functions.generate_all());

        // === TILES CHUNK ===
        asm.chunk(Chunk::Tiles);
        asm.emit_all(self.tiles.generate_tile_data());

        // === TILEMAP CHUNK ===
        asm.chunk(Chunk::Tilemap);
        asm.emit_all(self.tiles.generate_tilemap_data());

        // Include any raw assembly that was added (legacy Main chunk)
        let existing = self.asm.get_chunk(Chunk::Main).cloned().unwrap_or_default();
        if !existing.is_empty() {
            asm.chunk(Chunk::Main);
            asm.emit_all(existing);
        }

        asm.to_asm()
    }

    /// Add code to the main game loop
    pub fn add_to_main_loop(&mut self, code: Vec<Instr>) -> &mut Self {
        self.main_loop_code.extend(code);
        self
    }

    /// Add a sprite with its tile in one call
    /// Returns the sprite ID for later reference
    pub fn add_sprite(
        &mut self,
        name: &str,
        tile_source: super::tiles::TileSource,
        x: u8,
        y: u8,
        flags: u8,
    ) -> super::sprites::SpriteId {
        // Add the tile to the tile manager
        let tile_id = self.tiles.add_sprite(name, tile_source);

        // Add the sprite to the sprite manager
        let sprite_id = self.sprites.add(name, x, y, flags);

        // Link the tile ID to the sprite
        self.sprites.set_tile_id(sprite_id, tile_id);

        sprite_id
    }
}

impl Default for RustBoy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_rustboy() {
        let gb = RustBoy::new();
        assert!(gb.tiles.is_empty());
        assert!(gb.vars.is_empty());
    }

    #[test]
    fn test_if_counter() {
        let mut gb = RustBoy::new();

        assert_eq!(gb.next_if_counter(), 0);
        assert_eq!(gb.next_if_counter(), 1);
        assert_eq!(gb.next_if_counter(), 2);
    }

    #[test]
    fn test_unique_label() {
        let mut gb = RustBoy::new();

        assert_eq!(gb.unique_label("loop"), "loop_0");
        assert_eq!(gb.unique_label("loop"), "loop_1");
        assert_eq!(gb.unique_label("check"), "check_2");
    }

    #[test]
    fn test_define_const() {
        let mut gb = RustBoy::new();

        gb.define_const("BRICK_LEFT", "0x05");
        gb.define_const_hex("SCORE_ADDR", 0x9870);

        let output = gb.build();
        assert!(output.contains("DEF BRICK_LEFT EQU 0x05"));
        assert!(output.contains("DEF SCORE_ADDR EQU $9870"));
    }

    #[test]
    fn test_basic_build() {
        let mut gb = RustBoy::new();

        let output = gb.build();

        // Should contain basic structure
        assert!(output.contains("INCLUDE \"hardware.inc\""));
        assert!(output.contains("EntryPoint:"));
        assert!(output.contains("Main:"));
        assert!(output.contains("WaitVBlank:"));
    }
}
