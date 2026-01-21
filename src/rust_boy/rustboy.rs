//! Main RustBoy struct - the high-level Game Boy development API

use crate::gb_asm::{Asm, Chunk, Instr, JumpTarget};
use crate::gb_std::flow::Emittable;

use super::functions::{BuiltinFunction, FunctionRegistry};
use super::inputs::InputManager;
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

    /// Animation delay value in frames (higher = slower animations)
    animation_delay: u8,
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
            animation_delay: 8, // Default: update animation every 8 frames
        }
    }

    /// Set the animation delay value in frames (higher = slower animations)
    /// Default is 8 (animation updates every 8 frames, ~7.5 fps at 60fps)
    pub fn set_animation_delay(&mut self, delay: u8) -> &mut Self {
        self.animation_delay = delay;
        self
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
    pub fn init(&mut self, mut code: impl Emittable) -> &mut Self {
        let instrs = code.emit(&mut self.if_counter);
        self.init_code.extend(instrs);
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

    /// Register a user-defined function from raw instructions
    ///
    /// The function body should include its own label as the first instruction.
    ///
    /// # Example
    /// ```ignore
    /// gb.define_function("IsWallTile", is_specific_tile("IsWallTile", &["$00", "$01"]));
    /// ```
    pub fn define_function(&mut self, name: &str, body: Vec<Instr>) -> &mut Self {
        self.functions.register_user_function(name, body);
        self
    }

    /// Register a user-defined function from an Emittable
    ///
    /// This method automatically adds the function label and ret instruction.
    /// Use this when building functions from control flow structures like If, IfConst, etc.
    ///
    /// # Example
    /// ```ignore
    /// gb.define_function_from("CheckBrick", vec![
    ///     IfConst::eq(value, "BRICK", handle_brick),
    ///     IfA::eq("OTHER", handle_other),
    /// ]);
    /// ```
    pub fn define_function_from(&mut self, name: &str, mut body: impl Emittable) -> &mut Self {
        let mut asm = Asm::new();
        asm.label(name);
        asm.emit_all(body.emit(&mut self.if_counter));
        asm.ret();
        self.functions
            .register_user_function(name, asm.get_main_instrs());
        self
    }

    /// Generate a call instruction with validation
    ///
    /// This method validates that the function exists (either as a builtin or
    /// user-defined function) and automatically registers builtin functions
    /// when they are called.
    ///
    /// # Panics
    /// Panics if the function doesn't exist.
    ///
    /// # Example
    /// ```ignore
    /// // Automatically registers GetTileByPixel as used
    /// gb.add_to_main_loop(gb.call("GetTileByPixel"));
    ///
    /// // Works with user-defined functions too
    /// gb.define_function("IsWallTile", ...);
    /// gb.add_to_main_loop(gb.call("IsWallTile"));
    /// ```
    pub fn call(&mut self, name: &str) -> Vec<Instr> {
        if !self.functions.call_function(name) {
            let available = self.functions.available_functions();
            panic!(
                "Unknown function '{}'. Available functions: {}",
                name,
                available.join(", ")
            );
        }
        vec![Instr::Call {
            target: JumpTarget::Label(name.to_string()),
        }]
    }

    /// Call a function with setup instructions (parameters) and add to main loop
    ///
    /// This method emits the setup instructions before the call directly to
    /// the main loop, allowing fluent chaining without borrow checker issues.
    ///
    /// # Panics
    /// Panics if the function doesn't exist.
    ///
    /// # Example
    /// ```ignore
    /// // Call GetTileByPixel with setup from get_pivot
    /// gb.call_args("GetTileByPixel", gb.sprites.get_pivot(ball, 0, 1));
    /// gb.add_to_main_loop(IfCall::is_true("IsWallTile", _ball_momentum_y.set(1)));
    /// ```
    pub fn call_args(&mut self, name: &str, setup: Vec<Instr>) -> &mut Self {
        if !self.functions.call_function(name) {
            let available = self.functions.available_functions();
            panic!(
                "Unknown function '{}'. Available functions: {}",
                name,
                available.join(", ")
            );
        }
        self.main_loop_code.extend(setup);
        self.main_loop_code.push(Instr::Call {
            target: JumpTarget::Label(name.to_string()),
        });
        self
    }

    /// Check if a function exists (builtin or user-defined)
    pub fn function_exists(&self, name: &str) -> bool {
        self.functions.function_exists(name)
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

        // Add animation variables if animations are used
        if self.sprites.has_animations() {
            self.vars.create_u8("wFrameCounter", 0);

            // Create enabled flag for each animation
            for (var_name, initial_value) in self.sprites.get_animation_variables() {
                self.vars.create_u8(&var_name, initial_value);
            }
        }

        // Emit variable initialization
        asm.emit_all(self.vars.generate_init_code());

        // Turn on screen
        asm.ld_a_label("LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ16"); //TODO set in the struct
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

        // Generate animation calls at start of main loop
        if self.sprites.has_animations() {
            asm.emit_all(self.sprites.generate_animation_calls(self.animation_delay));
        }

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

        // Generate animation functions
        for (name, body) in self.sprites.generate_animation_functions() {
            // Register function first so it's tracked (though we emit directly)
            self.functions.register_user_function(&name, Vec::new());
            asm.emit_all(body);
        }

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
    ///
    /// Accepts anything that implements `Emittable`:
    /// - `Vec<Instr>` - raw instructions
    /// - `If` - control flow statements
    ///
    /// # Example
    /// ```ignore
    /// // Raw instructions
    /// gb.add_to_main_loop(asm.get_main_instrs());
    ///
    /// // If statement (counter managed automatically)
    /// gb.add_to_main_loop(If::eq(left, right, body));
    /// ```
    pub fn add_to_main_loop(&mut self, mut code: impl Emittable) -> &mut Self {
        let instrs = code.emit(&mut self.if_counter);
        self.main_loop_code.extend(instrs);
        self
    }

    /// Add input handling to the main game loop
    ///
    /// This method takes an InputManager and generates the complete input
    /// handling code, including:
    /// 1. Calling UpdateKeys to poll the controller
    /// 2. Checking each registered button binding
    /// 3. Executing associated actions when buttons are pressed
    ///
    /// # Example
    /// ```ignore
    /// let mut inputs = InputManager::new();
    /// inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 15));
    /// inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(paddle, 1, 105));
    /// gb.add_inputs(inputs);
    /// ```
    pub fn add_inputs(&mut self, inputs: InputManager) -> &mut Self {
        if inputs.is_empty() {
            return self;
        }

        // Auto-register UpdateKeys as used
        self.functions.use_function(BuiltinFunction::UpdateKeys);

        // Auto-create input variables required by UpdateKeys
        self.vars.create_u8("wCurKeys", 0);
        self.vars.create_u8("wNewKeys", 0);

        // Add call to UpdateKeys
        self.main_loop_code.push(Instr::Call {
            target: JumpTarget::Label("UpdateKeys".to_string()),
        });

        // Add the input handling code
        self.main_loop_code.extend(inputs.generate_code());

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
        // Get tile count before moving tile_source
        let tile_count = tile_source.tile_count() as u8;

        // Add the tile to the tile manager
        let tile_id = self.tiles.add_sprite(name, tile_source);

        // Add the sprite to the sprite manager with tile count for proper index allocation
        let sprite_id = self.sprites.add(name, x, y, flags, tile_count);

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
