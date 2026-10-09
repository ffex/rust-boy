//! Main RustBoy struct - the high-level Game Boy development API

use crate::gb_asm::{Asm, Chunk, Instr, JumpTarget, LabelAllocator, is_identifier};
use crate::gb_std::flow::Emittable;
use crate::gb_std::graphics::sprites::{clear_objects_screen, initialize_objects_screen};

use super::functions::{BuiltinFunction, FunctionRegistry, defines};
use super::inputs::InputManager;
use super::sprites::{SpriteManager, SpriteSize, check_name};
use super::tiles::TileManager;
use super::variables::VariableManager;

/// The palette `build()` writes to `rBGP`, `rOBP0` and `rOBP1` at start-up: colour i
/// shows shade i (0 lightest, 3 darkest), so both object palettes look like the
/// background one until the program changes them
const DEFAULT_PALETTE: u8 = 0b11100100;

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

    /// Numbers every other generated label (key checks, sprite moves, `unique_label`);
    /// shared with the sprite manager, so they never clash (B7)
    labels: LabelAllocator,

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
        let labels = LabelAllocator::new();
        Self {
            asm: Asm::new(),
            tiles: TileManager::new(),
            vars: VariableManager::new(),
            sprites: SpriteManager::new(labels.clone()),
            functions: FunctionRegistry::new(),
            if_counter: 0,
            labels,
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

    /// Set the size of every sprite: [`SpriteSize::Size8x8`] (the default, as on the
    /// hardware) or [`SpriteSize::Size8x16`]. `build()` writes it to LCDC.
    ///
    /// In 8x16 mode each sprite shows two stacked tiles, so every sprite needs an even
    /// number of tiles and an animation frame is two tiles. [`RustBoy::add_sprite_16x16`]
    /// needs 8x16 mode.
    ///
    /// # Panics
    /// If sprites were already added with another size: call it before adding sprites.
    pub fn set_sprite_size(&mut self, size: SpriteSize) -> &mut Self {
        self.sprites.set_size(size);
        self
    }

    /// The size of every sprite (see [`RustBoy::set_sprite_size`])
    pub fn sprite_size(&self) -> SpriteSize {
        self.sprites.size()
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

    /// Get the next general-purpose label counter (auto-increments); the sequence is
    /// shared with the labels of the generated key checks and sprite moves
    pub fn next_label_counter(&mut self) -> usize {
        self.labels.next_id()
    }

    /// Generate a unique label with prefix
    pub fn unique_label(&mut self, prefix: &str) -> String {
        format!("{}_{}", prefix, self.next_label_counter())
    }

    /// The allocator that numbers this program's generated local labels (key checks,
    /// sprite moves)
    ///
    /// Pass it to the `gb_std` snippets you mix into a `RustBoy` program
    /// (`check_key`, `Sprite::move_*_limit`): a separate allocator starts again at 0
    /// and would repeat labels that `RustBoy` already emitted.
    ///
    /// # Example
    /// ```
    /// use rust_boy::gb_std::inputs::{PadButton, check_key};
    /// use rust_boy::rust_boy::RustBoy;
    ///
    /// let mut gb = RustBoy::new();
    /// gb.add_to_main_loop(check_key(gb.labels(), PadButton::A, Vec::new()));
    /// ```
    pub fn labels(&self) -> &LabelAllocator {
        &self.labels
    }

    /// Add initialization code (runs once at startup)
    ///
    /// The start-up code runs, in order: LCD off, tile data copied to VRAM, OAM cleared
    /// and the sprites written, default palettes (`rBGP`, `rOBP0`, `rOBP1`), every
    /// variable set to its initial value (animation variables included), **this code**,
    /// then LCD on. So the code here can change any variable, animation or palette, and
    /// runs with the LCD off (VRAM and OAM can be written freely). `rLCDC` is the
    /// exception: `build()` sets it after this code to turn the LCD on.
    ///
    /// Do not wait for VBlank here (`call WaitVBlank`, a loop on `rLY`): with the LCD
    /// off, `rLY` stays 0 and the wait never ends.
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

    /// Emit a builtin function even if no code calls it
    ///
    /// Not needed for a builtin the program calls: `build()` emits every builtin and
    /// user function that the generated code refers to (see [`RustBoy::keep_function`]).
    pub fn use_function(&mut self, func: BuiltinFunction) -> &mut Self {
        self.functions.use_function(func);
        self
    }

    /// Emit the function `name` (a user function or a builtin) even if no code calls it
    ///
    /// `build()` emits only the functions the program uses: the ones the start-up code,
    /// the main loop, the code passed to [`RustBoy::raw`] or the animations refer to, and
    /// the ones those functions refer to, and so on. A function is found by its name
    /// anywhere in an instruction's code: `call`, `jp`, `IfCall`, `Call`, `ld hl, Name`,
    /// `dw Name`, raw lines (`Asm::raw`, one or several lines); not in comments or strings.
    /// The variables a builtin needs (`wCurKeys`, `wNewKeys` for `UpdateKeys`) are created
    /// with it. So keep only a function that is called from code `build()` does not see:
    /// asm added to its output afterwards, or an `INCLUDE`d file. For the opposite, a
    /// routine *defined* where `build()` does not see, use [`RustBoy::external_symbol`].
    ///
    /// # Panics
    /// If there is no function `name`: define it first.
    ///
    /// # Example
    /// ```
    /// use rust_boy::gb_asm::Asm;
    /// use rust_boy::rust_boy::RustBoy;
    ///
    /// let mut gb = RustBoy::new();
    /// let mut body = Asm::new();
    /// body.label("OnInterrupt").ret();
    /// gb.define_function("OnInterrupt", body.get_main_instrs());
    /// assert!(!gb.build().contains("OnInterrupt:"), "never called");
    ///
    /// gb.keep_function("OnInterrupt");
    /// assert!(gb.build().contains("OnInterrupt:"));
    /// ```
    pub fn keep_function(&mut self, name: &str) -> &mut Self {
        if !self.functions.keep_function(name) {
            self.unknown_function(name);
        }
        self
    }

    /// Declare that the symbol `name` is defined outside the code `build()` generates,
    /// for example by an `INCLUDE`d file
    ///
    /// `build()` takes every name the generated code defines (labels, `DEF`s, variables)
    /// as the program's own, and never emits a function of that name; but it does not read
    /// `INCLUDE`d files. A program that includes its own `UpdateKeys` (or another routine
    /// with a builtin's name) declares it here, so the builtin is not emitted as well (that
    /// would be a duplicate label), and neither are the variables that builtin needs
    /// (`wCurKeys`, `wNewKeys`): the included code defines what it uses. The opposite of
    /// [`RustBoy::keep_function`], which emits a function that only outside code calls.
    ///
    /// # Panics
    /// If `name` is not a valid RGBDS identifier; and `build()` panics if a user function
    /// has the same name (a function is defined either here or outside, not both).
    ///
    /// # Example
    /// ```
    /// use rust_boy::gb_std::flow::Call;
    /// use rust_boy::rust_boy::RustBoy;
    ///
    /// let mut gb = RustBoy::new();
    /// gb.raw(|asm| {
    ///     asm.include("my_input.inc"); // defines UpdateKeys
    /// });
    /// gb.add_to_main_loop(Call::new("UpdateKeys"));
    /// gb.external_symbol("UpdateKeys");
    /// assert!(!gb.build().contains("UpdateKeys:"));
    /// ```
    pub fn external_symbol(&mut self, name: &str) -> &mut Self {
        if !is_identifier(name) {
            panic!(
                "invalid external symbol \"{}\": it must be a valid RGBDS identifier",
                name
            );
        }
        self.functions.external_symbol(name);
        self
    }

    /// Panics: there is no function `name`
    fn unknown_function(&self, name: &str) -> ! {
        let available = self.functions.available_functions();
        panic!(
            "Unknown function '{}'. Available functions: {}",
            name,
            available.join(", ")
        );
    }

    /// Register a user-defined function from raw instructions
    ///
    /// The function body should include its own label as the first instruction.
    /// `build()` emits it only if the program uses it (see [`RustBoy::keep_function`]).
    /// A call to another global label of the body (a second entry point) uses it too.
    /// A function with the name of a builtin (`Memcopy`, ...) replaces that builtin.
    ///
    /// # Panics
    /// If `name` is not a valid RGBDS identifier, or `body` does not define the global
    /// label `name` (`build()` finds a function by its label). `build()` panics if `name`
    /// is also a variable, a constant, a label of the program or an external symbol.
    ///
    /// # Example
    /// ```ignore
    /// gb.define_function("IsWallTile", is_specific_tile("IsWallTile", &["$00", "$01"]));
    /// ```
    pub fn define_function(&mut self, name: &str, body: Vec<Instr>) -> &mut Self {
        check_function_name(name);
        if !defines(&body, name) {
            panic!(
                "define_function(\"{0}\"): the body does not define the label `{0}:`, so \
                 calls to `{0}` could not reach it (start the body with `{0}:`)",
                name
            );
        }
        self.functions.register_user_function(name, body);
        self
    }

    /// Register a user-defined function from an Emittable
    ///
    /// This method automatically adds the function label and ret instruction.
    /// Use this when building functions from control flow structures like If, IfConst, etc.
    /// `build()` emits it only if the program uses it (see [`RustBoy::keep_function`]).
    ///
    /// # Panics
    /// If `name` is not a valid RGBDS identifier: it becomes the function's label.
    ///
    /// # Example
    /// ```ignore
    /// gb.define_function_from("CheckBrick", vec![
    ///     IfConst::eq(value, "BRICK", handle_brick),
    ///     IfA::eq("OTHER", handle_other),
    /// ]);
    /// ```
    pub fn define_function_from(&mut self, name: &str, mut body: impl Emittable) -> &mut Self {
        check_function_name(name);
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
    /// user-defined function). Wherever the call ends up in the program, `build()` emits
    /// the function, like any function the program refers to.
    ///
    /// # Panics
    /// Panics if the function doesn't exist.
    ///
    /// # Example
    /// ```ignore
    /// // GetTileByPixel is emitted, because the main loop calls it
    /// let call = gb.call("GetTileByPixel");
    /// gb.add_to_main_loop(call);
    ///
    /// // Works with user-defined functions too
    /// gb.define_function("IsWallTile", ...);
    /// let call = gb.call("IsWallTile");
    /// gb.add_to_main_loop(call);
    /// ```
    pub fn call(&mut self, name: &str) -> Vec<Instr> {
        if !self.functions.function_exists(name) {
            self.unknown_function(name);
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
        if !self.functions.function_exists(name) {
            self.unknown_function(name);
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
        self.build_asm().to_asm()
    }

    /// The program [`build`](Self::build) prints, as instructions in chunks
    pub(crate) fn build_asm(&mut self) -> Asm {
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
        // In two parts, before and after the variable initialisation, which is emitted
        // once the functions are known: a builtin may need variables (B26)
        let mut startup = Asm::new();

        // Entry point
        startup.label("EntryPoint");
        startup.call("WaitVBlank");

        // Turn off screen for safe VRAM access
        startup.ld_a(0);
        startup.ld_addr_def_a("rLCDC");

        // Copy the tile data to VRAM (empty blobs are skipped, B27)
        startup.emit_all(self.tiles.generate_memcopy_calls());

        // Clear the whole OAM, with or without sprites: objects are always turned on
        // below, and OAM holds garbage at power-on (B28)
        startup.emit_all(initialize_objects_screen());
        startup.emit_all(clear_objects_screen());
        if !self.sprites.is_empty() {
            startup.emit_all(self.sprites.generate_init_code());
        }

        // Default palettes, every one of them (OBP1 too, B28)
        startup.ld_a(DEFAULT_PALETTE);
        for palette in ["rBGP", "rOBP0", "rOBP1"] {
            startup.ld_addr_def_a(palette);
        }

        // Then the variables (below), then the user init code, after every default it
        // may want to change: variables, animations, palettes, OAM (B11). The LCD is
        // still off, so it can write VRAM.
        let mut finish = Asm::new();
        finish.emit_all(self.init_code.clone());

        // Turn on screen, with the sprite size chosen by set_sprite_size
        finish.ld_a_label(&format!(
            "LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | {}",
            self.sprites.size().lcdc_flag()
        ));
        finish.ld_addr_def_a("rLCDC");
        let startup = startup.get_main_instrs();
        let finish = finish.get_main_instrs();

        // === MAIN LOOP CHUNK ===
        asm.chunk(Chunk::MainLoop);

        asm.label("Main");
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

        // Add animation variables if animations are used
        if self.sprites.has_animations() {
            self.vars.create_u8("wFrameCounter", 0);

            // Create enabled flag for each animation
            for (var_name, initial_value) in self.sprites.get_animation_variables() {
                self.vars.create_u8(&var_name, initial_value);
            }
        }

        // === FUNCTIONS CHUNK ===
        // Once all the code is known: the builtins and user functions that the program
        // refers to, directly or through other functions (B24, B26), then the animation
        // functions. (Variables only hold data, so their code is not needed for this, but
        // their names are: a name the program defines is not a function.)
        let animations = self.sprites.generate_animation_functions();
        let mut code: Vec<&[Instr]> = [
            Chunk::Header,
            Chunk::Constants,
            Chunk::MainLoop,
            Chunk::Main,
            Chunk::Tiles,
            Chunk::Tilemap,
        ]
        .iter()
        .filter_map(|chunk| asm.get_chunk(*chunk))
        .map(Vec::as_slice)
        .collect();
        code.extend([startup.as_slice(), finish.as_slice()]);
        code.extend(animations.iter().map(|(_, body)| body.as_slice()));
        let functions = self.functions.generate_used(&code, self.vars.names());
        asm.chunk(Chunk::Functions);
        asm.emit_all(functions.code);

        for (name, body) in animations {
            // Known to `call` from now on; emitted here, not scanned as a user function
            self.functions.register_generated(&name);
            asm.emit_all(body);
        }

        // === VARIABLES: the INIT CHUNK, and the DATA CHUNK ===
        // The variables of the emitted builtins (`wCurKeys`, `wNewKeys` for UpdateKeys),
        // however the program calls them, unless it defines them already (as variables or
        // in raw code)
        for name in functions.variables {
            self.vars.create_u8(name, 0);
        }

        asm.chunk(Chunk::Init);
        asm.emit_all(startup);
        asm.emit_all(self.vars.generate_init_code());
        asm.emit_all(finish);

        asm.chunk(Chunk::Data);
        asm.emit_all(self.vars.generate_sections());

        asm
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
    /// inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 16));
    /// inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(paddle, 1, 104));
    /// gb.add_inputs(inputs);
    /// ```
    pub fn add_inputs(&mut self, inputs: InputManager) -> &mut Self {
        if inputs.is_empty() {
            return self;
        }

        // Auto-create input variables required by UpdateKeys
        self.vars.create_u8("wCurKeys", 0);
        self.vars.create_u8("wNewKeys", 0);

        // Add call to UpdateKeys
        self.main_loop_code.push(Instr::Call {
            target: JumpTarget::Label("UpdateKeys".to_string()),
        });

        // Add the input handling code
        self.main_loop_code
            .extend(inputs.generate_code(&self.labels));

        self
    }

    /// Add a sprite with its tile in one call
    /// Returns the sprite ID for later reference
    ///
    /// # Panics
    /// - If `name` is not a valid RGBDS identifier, or another sprite has it: the name
    ///   becomes part of labels.
    /// - In 8x16 mode, if `tile_source` has an odd number of tiles.
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

    /// Add a 16x16 composite sprite made of two 8x16 sprites side by side
    ///
    /// This creates two hardware sprites (left and right halves) and groups them
    /// as a composite sprite that can be moved and animated together.
    ///
    /// # Arguments
    /// * `name` - Base name for the composite sprite
    /// * `left_tiles` - Tile source for the left 8x16 half
    /// * `right_tiles` - Tile source for the right 8x16 half
    /// * `x` - X position of the left edge
    /// * `y` - Y position
    /// * `flags` - OAM flags for both sprites
    ///
    /// # Returns
    /// A `CompositeSpriteId` that can be used with composite sprite methods
    ///
    /// # Panics
    /// - If `name` is not a valid RGBDS identifier, or a sprite already has the name of a
    ///   half (`{name}_left`, `{name}_right`): the names become labels.
    /// - In 8x8 mode (call `set_sprite_size(SpriteSize::Size8x16)` first), or if a half has
    ///   an odd number of tiles.
    pub fn add_sprite_16x16(
        &mut self,
        name: &str,
        left_tiles: super::tiles::TileSource,
        right_tiles: super::tiles::TileSource,
        x: u8,
        y: u8,
        flags: u8,
    ) -> super::sprites::CompositeSpriteId {
        check_name("composite sprite", name);
        if self.sprites.size() != SpriteSize::Size8x16 {
            panic!(
                "add_sprite_16x16(\"{}\") needs 8x16 sprites: call \
                 set_sprite_size(SpriteSize::Size8x16) before adding sprites",
                name
            );
        }

        // Create the left sprite
        let left_name = format!("{}_left", name);
        let left_sprite = self.add_sprite(&left_name, left_tiles, x, y, flags);

        // Create the right sprite (8 pixels to the right)
        let right_name = format!("{}_right", name);
        let right_sprite = self.add_sprite(&right_name, right_tiles, x + 8, y, flags);

        // Group them as a composite sprite
        self.sprites
            .create_composite(name, vec![left_sprite, right_sprite])
    }
}

/// Panics unless `name` can be a function's label
fn check_function_name(name: &str) {
    if !is_identifier(name) {
        panic!(
            "invalid function name \"{}\": it must be a valid RGBDS identifier (a letter or \
             `_`, then letters, digits, `_`, `#`, `$` or `@`)",
            name
        );
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

    /// One program that uses every manager whose output order matters
    fn sample_rustboy() -> RustBoy {
        use crate::gb_std::inputs::PadButton;
        use crate::rust_boy::{AnimationType, TileSource, VarType};

        let mut gb = RustBoy::new();

        gb.tiles
            .add_background("BgTiles", TileSource::from_raw(&[["$00"; 8]]));
        gb.tiles.add_tilemap("Map", &[[0u8; 32]]);

        let frames: [[&str; 8]; 2] = [["$FF"; 8], ["$00"; 8]];
        // The same animation name on every sprite (B25)
        for name in ["Alpha", "Bravo", "Charlie"] {
            let sprite = gb.add_sprite(name, TileSource::from_raw(&frames), 16, 16, 0);
            gb.sprites
                .add_animation(sprite, "Spin", 0, 1, AnimationType::Loop);
        }

        gb.vars.create_u8("wZulu", 1);
        gb.vars.create_u8("wYankee", 2);
        gb.vars.create_in_section("wHigh", VarType::U8, 3, "Other");
        gb.vars.create_u8("wXray", 4);

        for name in ["FuncB", "FuncA"] {
            let mut body = Asm::new();
            body.label(name).ret();
            gb.define_function(name, body.get_main_instrs());
        }
        // Called in the other order: functions are emitted in registration order
        gb.add_to_main_loop(crate::gb_std::flow::Call::new("FuncA"));
        gb.add_to_main_loop(crate::gb_std::flow::Call::new("FuncB"));
        gb.use_function(BuiltinFunction::Delay);
        gb.use_function(BuiltinFunction::GetTileByPixel);

        let mut inputs = InputManager::new();
        inputs.on_press(PadButton::A, Vec::new());
        gb.add_inputs(inputs);

        gb
    }

    fn sample_game() -> String {
        sample_rustboy().build()
    }

    #[test]
    fn test_build_is_deterministic() {
        let first = sample_game();
        for _ in 0..10 {
            assert!(
                sample_game() == first,
                "two builds of the same program differ"
            );
        }
    }

    #[test]
    fn test_build_follows_creation_order() {
        let out = sample_game();
        let pos = |needle: &str| {
            out.find(needle)
                .unwrap_or_else(|| panic!("`{}` not found in the output", needle))
        };
        let in_order = |needles: &[&str]| {
            for pair in needles.windows(2) {
                assert!(
                    pos(pair[0]) < pos(pair[1]),
                    "`{}` should come before `{}`",
                    pair[0],
                    pair[1]
                );
            }
        };

        // Variable sections in first-use order, variables in creation order
        in_order(&[
            "SECTION \"Variables\"",
            "wZulu: db",
            "wYankee: db",
            "wXray: db",
            "SECTION \"Other\"",
            "wHigh: db",
        ]);
        // Variable initialisation in creation order
        in_order(&[
            "ld [wZulu], a",
            "ld [wYankee], a",
            "ld [wHigh], a",
            "ld [wXray], a",
        ]);
        // Tile data copied to VRAM in creation order
        in_order(&[
            "ld de, BgTiles",
            "ld de, Map",
            "ld de, Alpha",
            "ld de, Bravo",
            "ld de, Charlie",
        ]);
        // Animation dispatch and functions in sprite order
        in_order(&[
            "call Anim_Alpha_Spin",
            "call Anim_Bravo_Spin",
            "call Anim_Charlie_Spin",
        ]);
        in_order(&["Anim_Alpha_Spin:", "Anim_Bravo_Spin:", "Anim_Charlie_Spin:"]);
        // Builtins in a fixed order, then user functions in registration order
        in_order(&[
            "Memcopy:",
            "WaitVBlank:",
            "WaitNotVBlank:",
            "UpdateKeys:",
            "GetTileByPixel:",
            "Delay:",
            "FuncB:",
            "FuncA:",
        ]);
    }

    #[test]
    fn test_variables_created_twice_are_emitted_once() {
        use crate::gb_std::inputs::PadButton;

        let mut gb = RustBoy::new();
        // add_inputs creates wCurKeys/wNewKeys too
        gb.vars.create_u8("wCurKeys", 0);
        gb.vars.create_u8("wNewKeys", 0);
        let mut inputs = InputManager::new();
        inputs.on_press(PadButton::A, Vec::new());
        gb.add_inputs(inputs);

        let out = gb.build();
        assert_eq!(out.matches("wCurKeys: db").count(), 1);
        assert_eq!(out.matches("wNewKeys: db").count(), 1);
    }

    #[test]
    fn test_build_twice_gives_the_same_output() {
        let mut gb = sample_rustboy();
        let first = gb.build();
        assert!(gb.build() == first, "a second build() changed the output");
    }

    // ==================== Sprite size (B4) ====================

    use crate::rust_boy::{AnimationType, SpriteId, TileSource};

    /// `count` sprite tiles
    fn tiles(count: usize) -> TileSource {
        TileSource::from_raw(&vec![["$FF"; 8]; count])
    }

    /// The instruction that turns the LCD on
    fn lcdc_on(out: &str) -> &str {
        out.lines()
            .find(|line| line.contains("LCDCF_ON"))
            .expect("the LCD is never turned on")
            .trim()
    }

    /// The body of a generated function, from its label to its `ret`
    fn function<'a>(out: &'a str, label: &str) -> &'a str {
        let start = out
            .find(&format!("{}:", label))
            .unwrap_or_else(|| panic!("`{}` not found", label));
        let len = out[start..].find("ret").expect("no ret");
        &out[start..start + len]
    }

    /// OAM tile index of a sprite, and the VRAM address its tiles are copied to
    fn tile_of(gb: &RustBoy, id: SpriteId) -> (u8, u16) {
        let sprite = gb.sprites.get(id).expect("unknown sprite");
        let addr = gb.tiles.get_address(sprite.tile_id).expect("no tiles");
        (sprite.tile_index, addr)
    }

    #[test]
    fn test_sprites_are_8x8_by_default() {
        let mut gb = RustBoy::new();
        assert_eq!(gb.sprite_size(), SpriteSize::Size8x8);
        let paddle = gb.add_sprite("Paddle", tiles(1), 16, 128, 0);
        let ball = gb.add_sprite("Ball", tiles(1), 32, 100, 0);

        let out = gb.build();
        assert_eq!(
            lcdc_on(&out),
            "ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ8"
        );
        // One tile per sprite: Ball draws its own tile, not Paddle's bottom half
        assert_eq!(tile_of(&gb, paddle), (0, 0x8000));
        assert_eq!(tile_of(&gb, ball), (1, 0x8010));
    }

    #[test]
    fn test_8x16_mode_sets_lcdc_obj16() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        gb.add_sprite("Player", tiles(2), 80, 72, 0);
        // Setting the same size again is fine
        gb.set_sprite_size(SpriteSize::Size8x16);
        assert_eq!(gb.sprite_size(), SpriteSize::Size8x16);

        let out = gb.build();
        assert_eq!(
            lcdc_on(&out),
            "ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ16"
        );
    }

    #[test]
    fn test_8x16_tile_indices_are_even() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let a = gb.add_sprite("A", tiles(2), 0, 0, 0);
        let b = gb.add_sprite("B", tiles(6), 0, 0, 0);
        let c = gb.add_sprite_16x16("C", tiles(4), tiles(4), 0, 0, 0);
        let d = gb.add_sprite("D", tiles(2), 0, 0, 0);
        let halves = gb.sprites.get_composite_sprites(c).unwrap().clone();

        assert_eq!(tile_of(&gb, a), (0, 0x8000));
        assert_eq!(tile_of(&gb, b), (2, 0x8020));
        assert_eq!(tile_of(&gb, halves[0]), (8, 0x8080));
        assert_eq!(tile_of(&gb, halves[1]), (12, 0x80C0));
        assert_eq!(tile_of(&gb, d), (16, 0x8100));
    }

    #[test]
    fn test_8x16_animation_frame_is_two_tiles() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        gb.add_sprite("Other", tiles(2), 0, 0, 0);
        let walker = gb.add_sprite("Walker", tiles(8), 80, 72, 0);
        gb.sprites
            .add_animation(walker, "Walk", 1, 3, AnimationType::Loop);

        let out = gb.build();
        // Walker starts at tile 2; frames 1..=3 are tiles 4, 6 and 8, two apart
        let walk = function(&out, "Anim_Walker_Walk");
        assert!(walk.contains("add a, 2"), "{}", walk);
        assert!(walk.contains("cp 4"), "{}", walk);
        assert!(walk.contains("cp 10"), "{}", walk);
        assert!(walk.contains("ld a, 4"), "{}", walk);
    }

    #[test]
    fn test_8x8_animation_frame_is_one_tile() {
        let mut gb = RustBoy::new();
        let coin = gb.add_sprite("Coin", tiles(7), 80, 72, 0);
        gb.sprites
            .add_animation(coin, "Spin", 0, 6, AnimationType::Loop);

        let out = gb.build();
        let spin = function(&out, "Anim_Coin_Spin");
        assert!(spin.contains("inc a"), "{}", spin);
        assert!(spin.contains("cp 7"), "{}", spin);
    }

    #[test]
    #[should_panic(expected = "add_sprite_16x16(\"Player\") needs 8x16 sprites")]
    fn test_16x16_sprite_needs_8x16_mode() {
        let mut gb = RustBoy::new();
        gb.add_sprite_16x16("Player", tiles(2), tiles(2), 80, 72, 0);
    }

    #[test]
    #[should_panic(expected = "sprite \"Odd\" has 3 tiles")]
    fn test_odd_tile_count_in_8x16_mode_panics() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        gb.add_sprite("Odd", tiles(3), 0, 0, 0);
    }

    #[test]
    #[should_panic(expected = "animation \"Walk\": frame_step 1 is odd")]
    fn test_odd_frame_step_in_8x16_mode_panics() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let walker = gb.add_sprite("Walker", tiles(8), 0, 0, 0);
        gb.sprites
            .add_animation_with_step(walker, "Walk", 0, 3, AnimationType::Loop, 1);
    }

    #[test]
    #[should_panic(expected = "set_sprite_size(Size8x16) must be called before the first sprite")]
    fn test_sprite_size_cannot_change_after_adding_sprites() {
        let mut gb = RustBoy::new();
        gb.add_sprite("Ball", tiles(1), 0, 0, 0);
        gb.set_sprite_size(SpriteSize::Size8x16);
    }

    // ==================== Labels (B7, B25) ====================

    use crate::gb_asm::label_check::assert_labels_ok;
    use crate::gb_std::flow::{Call, If};
    use crate::gb_std::inputs::PadButton;
    use crate::rust_boy::CompositeSpriteId;

    /// A 8x8 paddle and ball
    fn paddle_and_ball(gb: &mut RustBoy) -> (SpriteId, SpriteId) {
        let paddle = gb.add_sprite("Paddle", tiles(1), 16, 128, 0);
        let ball = gb.add_sprite("Ball", tiles(1), 32, 100, 0);
        (paddle, ball)
    }

    /// A 16x16 player, in 8x16 mode
    fn player(gb: &mut RustBoy) -> CompositeSpriteId {
        gb.set_sprite_size(SpriteSize::Size8x16);
        gb.add_sprite_16x16("Player", tiles(2), tiles(2), 80, 72, 0)
    }

    #[test]
    fn test_sample_game_labels_are_ok() {
        assert_labels_ok(&sample_game());
    }

    #[test]
    fn test_two_bindings_on_one_button() {
        // B7: each binding emitted the global labels CheckLeft and CheckLeftEnd
        let mut gb = RustBoy::new();
        let (paddle, ball) = paddle_and_ball(&mut gb);
        let mut inputs = InputManager::new();
        inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 16));
        inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(ball, 1, 16));
        gb.add_inputs(inputs);
        // Bindings added later, through another InputManager
        let mut more = InputManager::new();
        more.on_press(PadButton::Left, gb.sprites.move_up_limit(ball, 1, 16));
        gb.add_inputs(more);

        assert_labels_ok(&gb.build());
    }

    #[test]
    fn test_one_move_on_two_buttons() {
        // B7: each copy of a move emitted the same Sprite0LeftLimitStore / ...End labels
        let mut gb = RustBoy::new();
        let (paddle, _) = paddle_and_ball(&mut gb);
        let mut inputs = InputManager::new();
        for button in [PadButton::Left, PadButton::B] {
            inputs.on_press(button, gb.sprites.move_left_limit(paddle, 1, 16));
            inputs.on_press(button, gb.sprites.move_down_limit(paddle, 2, 144));
        }
        gb.add_inputs(inputs);

        assert_labels_ok(&gb.build());
    }

    #[test]
    fn test_one_composite_move_on_two_buttons() {
        // B7: a composite move used its leading sprite's labels, so it clashed with
        // itself and with that sprite's own move
        let mut gb = RustBoy::new();
        let player = player(&mut gb);
        let left_half = gb.sprites.get_composite_sprites(player).unwrap()[0];
        let mut inputs = InputManager::new();
        for button in [PadButton::Left, PadButton::B] {
            inputs.on_press(button, gb.sprites.move_composite_left_limit(player, 1, 8));
        }
        inputs.on_press(PadButton::A, gb.sprites.move_left_limit(left_half, 1, 8));
        gb.add_inputs(inputs);

        assert_labels_ok(&gb.build());
    }

    #[test]
    fn test_moves_inside_an_if() {
        // B7: the labels of a move were global, so inside an If body they started a new
        // label scope and the If's jump to .end_if_N could not be resolved
        let mut gb = RustBoy::new();
        let (paddle, ball) = paddle_and_ball(&mut gb);
        let moves = [
            gb.sprites.move_left_limit(paddle, 1, 16),
            gb.sprites.move_right_limit(paddle, 1, 104),
            gb.sprites.move_up_limit(ball, 1, 16),
            gb.sprites.move_down_limit(ball, 1, 144),
        ];
        for code in moves {
            let if_ball_above = If::lt(gb.sprites.get_y(ball), gb.sprites.get_y(paddle), code);
            gb.add_to_main_loop(if_ball_above);
        }
        // In the else branch, and in a function
        let else_move = If::eq(
            gb.sprites.get_x(ball),
            gb.sprites.get_x(paddle),
            Vec::<Instr>::new(),
        )
        .or_else(gb.sprites.move_left_limit(ball, 2, 16));
        gb.define_function_from("FollowPaddle", else_move);
        gb.add_to_main_loop(Call::new("FollowPaddle"));

        let out = gb.build();
        assert!(out.contains("FollowPaddle:"));
        assert_labels_ok(&out);
    }

    #[test]
    fn test_gb_std_snippets_share_the_program_labels() {
        // gb_std snippets mixed into a RustBoy program take its allocator: one of their
        // own would start again at 0 and repeat RustBoy's labels (rgbasm:
        // `Main.check_left_0` already defined)
        use crate::gb_std::graphics::sprites::Sprite;
        use crate::gb_std::inputs::check_key;

        let mut gb = RustBoy::new();
        let (paddle, _) = paddle_and_ball(&mut gb);
        let mut inputs = InputManager::new();
        inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 16));
        gb.add_inputs(inputs);
        // The same button and the same OAM entry, through gb_std
        let mut oam_0 = Sprite::new(0, 16, 128, 0, 0);
        let body = oam_0.move_left_limit(gb.labels(), 1, 16);
        gb.add_to_main_loop(check_key(gb.labels(), PadButton::Left, body));

        assert_labels_ok(&gb.build());
    }

    #[test]
    fn test_composite_move_inside_an_if() {
        let mut gb = RustBoy::new();
        let player = player(&mut gb);
        let halves = gb.sprites.get_composite_sprites(player).unwrap().clone();
        let move_left = If::ge(
            gb.sprites.get_x(halves[0]),
            gb.sprites.get_y(halves[0]),
            gb.sprites.move_composite_left_limit(player, 1, 8),
        );
        gb.add_to_main_loop(move_left);

        assert_labels_ok(&gb.build());
    }

    #[test]
    fn test_two_sprites_with_the_same_animation_name() {
        // B25: both sprites emitted Anim_Spin, and the dispatcher .skip_Spin twice
        let mut gb = RustBoy::new();
        for name in ["Coin", "Gem"] {
            let sprite = gb.add_sprite(name, tiles(4), 16, 16, 0);
            gb.sprites
                .add_animation(sprite, "Spin", 0, 3, AnimationType::Loop);
        }

        let out = gb.build();
        assert_labels_ok(&out);
        // Each sprite runs its own animation, on its own tiles
        assert!(out.contains("call Anim_Coin_Spin"), "{}", out);
        assert!(out.contains("call Anim_Gem_Spin"), "{}", out);
        assert!(function(&out, "Anim_Coin_Spin").contains("_OAMRAM+2"));
        assert!(function(&out, "Anim_Gem_Spin").contains("_OAMRAM+6"));
    }

    #[test]
    fn test_two_composites_with_the_same_animation_name() {
        // B25: both composites emitted Anim_Walk_0 and Anim_Walk_1
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        for name in ["Hero", "Rival"] {
            let composite = gb.add_sprite_16x16(name, tiles(4), tiles(4), 0, 0, 0);
            gb.sprites
                .add_composite_animation(composite, "Walk", 0, 1, AnimationType::Loop);
            gb.sprites
                .add_composite_animation(composite, "Run", 0, 1, AnimationType::Loop);
        }

        assert_labels_ok(&gb.build());
    }

    #[test]
    #[should_panic(expected = "invalid sprite name \"my sprite\"")]
    fn test_sprite_name_must_be_an_identifier() {
        let mut gb = RustBoy::new();
        gb.add_sprite("my sprite", tiles(1), 0, 0, 0);
    }

    #[test]
    #[should_panic(expected = "invalid composite sprite name \"2player\"")]
    fn test_composite_name_must_be_an_identifier() {
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        gb.add_sprite_16x16("2player", tiles(2), tiles(2), 0, 0, 0);
    }

    #[test]
    #[should_panic(expected = "invalid animation name \"Spin-Left\"")]
    fn test_animation_name_must_be_an_identifier() {
        let mut gb = RustBoy::new();
        let coin = gb.add_sprite("Coin", tiles(4), 0, 0, 0);
        gb.sprites
            .add_animation(coin, "Spin-Left", 0, 3, AnimationType::Loop);
    }

    #[test]
    #[should_panic(expected = "invalid animation name \"Walk Left\"")]
    fn test_composite_animation_name_must_be_an_identifier() {
        let mut gb = RustBoy::new();
        let player = player(&mut gb);
        gb.sprites
            .add_composite_animation(player, "Walk Left", 0, 0, AnimationType::Loop);
    }

    #[test]
    #[should_panic(expected = "sprite name \"Coin\" is already used")]
    fn test_sprite_names_are_unique() {
        let mut gb = RustBoy::new();
        gb.add_sprite("Coin", tiles(1), 0, 0, 0);
        gb.add_sprite("Coin", tiles(1), 8, 0, 0);
    }

    #[test]
    #[should_panic(expected = "sprite \"Coin\" already has an animation \"Spin\"")]
    fn test_animation_names_are_unique_per_sprite() {
        let mut gb = RustBoy::new();
        let coin = gb.add_sprite("Coin", tiles(4), 0, 0, 0);
        gb.sprites
            .add_animation(coin, "Spin", 0, 1, AnimationType::Loop);
        gb.sprites
            .add_animation(coin, "Spin", 2, 3, AnimationType::Loop);
    }

    #[test]
    #[should_panic(expected = "composite sprite \"Player\" already has an animation \"Walk\"")]
    fn test_composite_animation_names_are_unique() {
        let mut gb = RustBoy::new();
        let player = player(&mut gb);
        gb.sprites
            .add_composite_animation(player, "Walk", 0, 0, AnimationType::Loop);
        gb.sprites
            .add_composite_animation(player, "Walk", 0, 0, AnimationType::Loop);
    }

    #[test]
    #[should_panic(expected = "label Anim_Big_Coin_Spin")]
    fn test_animation_labels_cannot_collide() {
        // Sprite "Big_Coin" + animation "Spin" and sprite "Big" + animation "Coin_Spin"
        // would both be Anim_Big_Coin_Spin
        let mut gb = RustBoy::new();
        let big_coin = gb.add_sprite("Big_Coin", tiles(2), 0, 0, 0);
        let big = gb.add_sprite("Big", tiles(2), 8, 0, 0);
        gb.sprites
            .add_animation(big_coin, "Spin", 0, 1, AnimationType::Loop);
        gb.sprites
            .add_animation(big, "Coin_Spin", 0, 1, AnimationType::Loop);
    }

    // ==================== Start-up code (B11, B28) ====================

    use crate::gb_asm::test_cpu::{Event, TestCpu};

    /// The palette every palette register starts with: colour i shows shade i
    const IDENTITY_PALETTE: u8 = 0b11100100;

    /// Run the start-up code of `gb` on the test CPU, from `EntryPoint` to where the main
    /// loop starts. `Memcopy` is a stub (the CPU model does not copy blocks): its calls
    /// are in the trace.
    fn run_startup(gb: &mut RustBoy) -> TestCpu {
        let (code, mut cpu) = startup(gb);
        cpu.stubs.insert("Memcopy".to_string());
        cpu.run(&code);
        cpu
    }

    /// The start-up code of `gb`, from `EntryPoint` to where the main loop starts, then
    /// its functions; and a CPU ready to run it
    fn startup(gb: &mut RustBoy) -> (Vec<Instr>, TestCpu) {
        let lcdc_on = format!(
            "LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | {}",
            gb.sprite_size().lcdc_flag()
        );
        let asm = gb.build_asm();
        let mut code = asm
            .get_chunk(Chunk::Init)
            .cloned()
            .expect("no start-up code");
        code.push(Instr::Ret); // stop where the main loop starts
        code.extend(asm.get_chunk(Chunk::Functions).cloned().unwrap_or_default());

        let mut cpu = TestCpu::default();
        cpu.mem.insert("rLY".to_string(), 144); // in VBlank: WaitVBlank returns at once
        cpu.consts.insert(lcdc_on, 0x83);
        (code, cpu)
    }

    /// The memory symbol of OAM byte `i`, as the test CPU names it
    fn oam(i: u16) -> String {
        if i == 0 {
            "_OAMRAM".to_string()
        } else {
            format!("_OAMRAM+{}", i)
        }
    }

    /// Asserts that `cpu` wrote `value` to `symbol`
    fn assert_mem(cpu: &TestCpu, symbol: &str, value: u8) {
        assert_eq!(cpu.mem.get(symbol), Some(&value), "[{}]", symbol);
    }

    #[test]
    fn test_init_code_runs_after_variable_initialisation() {
        let mut gb = RustBoy::new();
        let lives = gb.vars.create_u8("wLives", 0);
        let coin = gb.add_sprite("Coin", tiles(4), 16, 16, 0);
        gb.sprites
            .add_animation(coin, "Spin", 0, 1, AnimationType::Loop);
        gb.sprites
            .add_animation(coin, "Bounce", 2, 3, AnimationType::PingPong);

        // What init() sets must survive the start-up code: a variable, the animation
        // (disabled by default), the PingPong direction and a palette
        gb.init(lives.set(3));
        let enable = gb.sprites.enable_animation(coin, 1);
        gb.init(enable);
        let mut own = Asm::new();
        own.ld_a(1)
            .ld_addr_def_a("wAnim_Coin_Dir")
            .ld_a(0b00011011)
            .ld_addr_def_a("rBGP");
        gb.init(own.get_main_instrs());

        let cpu = run_startup(&mut gb);
        assert_mem(&cpu, "wLives", 3);
        assert_mem(&cpu, "wAnim_Coin_Current", 1);
        assert_mem(&cpu, "wAnim_Coin_Dir", 1);
        assert_mem(&cpu, "wFrameCounter", 0);
        assert_mem(&cpu, "rBGP", 0b00011011);
    }

    #[test]
    fn test_startup_order() {
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("BgTiles", TileSource::from_raw(&[["$00"; 8]]));
        gb.vars.create_u8("wScore", 7);
        gb.add_sprite("Ball", tiles(1), 16, 16, 0);
        let mut user = Asm::new();
        user.ld_a(5).ld_addr_def_a("rSCX");
        gb.init(user.get_main_instrs());

        let cpu = run_startup(&mut gb);
        // Each side effect, by the start-up step it belongs to
        let step = |event: &Event| match event {
            Event::Call(name) if name == "WaitVBlank" => "wait for VBlank",
            Event::Write(reg, 0) if reg == "rLCDC" => "LCD off",
            Event::Call(name) if name == "Memcopy" => "copy to VRAM",
            Event::Write(addr, _) if addr.starts_with("_OAMRAM") => "OAM",
            Event::Write(reg, _) if ["rBGP", "rOBP0", "rOBP1"].contains(&reg.as_str()) => {
                "palettes"
            }
            Event::Write(var, _) if var.starts_with('w') => "variables",
            Event::Write(reg, _) if reg == "rSCX" => "user init",
            Event::Write(reg, _) if reg == "rLCDC" => "LCD on",
            other => panic!("unexpected start-up event {:?}", other),
        };
        let mut steps: Vec<&str> = cpu.trace.iter().map(step).collect();
        steps.dedup();
        assert_eq!(
            steps,
            [
                "wait for VBlank",
                "LCD off",
                "copy to VRAM",
                "OAM",
                "palettes",
                "variables",
                "user init",
                "LCD on",
            ]
        );
    }

    #[test]
    fn test_oam_is_cleared_without_sprites() {
        // A background-only program still turns objects on: the OAM must be empty
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("BgTiles", TileSource::from_raw(&[["$00"; 8]]));
        gb.vars.create_u8("wScore", 0);

        let cpu = run_startup(&mut gb);
        for i in 0..160 {
            assert_mem(&cpu, &oam(i), 0);
        }
        assert_labels_ok(&gb.build());
    }

    #[test]
    fn test_oam_is_cleared_before_the_sprites_are_written() {
        let mut gb = RustBoy::new();
        gb.add_sprite("Ball", tiles(1), 16, 24, 0);

        let cpu = run_startup(&mut gb);
        let written: Vec<u8> = (0..4).map(|i| cpu.mem[&oam(i)]).collect();
        assert_eq!(written, [24 + 16, 16 + 8, 0, 0], "Y, X, tile, flags");
        for i in 4..160 {
            assert_mem(&cpu, &oam(i), 0);
        }
    }

    #[test]
    fn test_every_palette_is_set() {
        for with_sprites in [false, true] {
            let mut gb = RustBoy::new();
            if with_sprites {
                gb.add_sprite("Ball", tiles(1), 16, 16, 0);
            }
            let cpu = run_startup(&mut gb);
            for palette in ["rBGP", "rOBP0", "rOBP1"] {
                assert_mem(&cpu, palette, IDENTITY_PALETTE);
            }
        }
    }

    // ==================== Functions (B23, B24, B26, B27) ====================

    // assert_links: no undefined symbol, and with RGBDS_LINK_CHECK set, rgbasm + rgblink
    use crate::gb_asm::label_check::assert_links;
    use crate::gb_std::flow::{IfA, IfCall, IfConst, boxed};
    use crate::gb_std::graphics::tile_ref::TileRef;

    /// How many times `out` defines the global label `name`
    fn definitions(out: &str, name: &str) -> usize {
        let label = format!("{}:", name);
        out.lines().filter(|line| line.trim() == label).count()
    }

    /// A function `name` that calls each of `callees`
    fn calling(name: &str, callees: &[&str]) -> Vec<Instr> {
        let mut body = Asm::new();
        body.label(name);
        for callee in callees {
            body.call(callee);
        }
        body.ret();
        body.get_main_instrs()
    }

    #[test]
    fn test_builtins_are_emitted_whatever_calls_them() {
        // B26: only RustBoy::call / call_args / use_function included a builtin, so one
        // reached through Call, IfCall, a function body or raw code was missing (rgblink:
        // undefined symbol)
        type Program = fn(&mut RustBoy);
        let paths: [(&str, &str, Program); 9] = [
            ("Call in the main loop", "GetTileByPixel", |gb| {
                gb.add_to_main_loop(Call::with_args("GetTileByPixel", Vec::new()));
            }),
            ("IfCall in init", "Delay", |gb| {
                gb.init(IfCall::is_false("Delay", Vec::<Instr>::new()));
            }),
            ("Call in a define_function_from body", "Memcopy", |gb| {
                gb.define_function_from("CopyAll", Call::new("Memcopy"));
                gb.add_to_main_loop(Call::new("CopyAll"));
            }),
            ("a define_function body", "UpdateKeys", |gb| {
                gb.define_function("Poll", calling("Poll", &["UpdateKeys"]));
                let call = gb.call("Poll");
                gb.add_to_main_loop(call);
            }),
            ("a function called by a function", "GetTileByPixel", |gb| {
                gb.define_function("Inner", calling("Inner", &["GetTileByPixel"]));
                gb.define_function("Outer", calling("Outer", &["Inner"]));
                gb.init(IfCall::is_true("Outer", Vec::<Instr>::new()));
            }),
            ("raw code", "Delay", |gb| {
                gb.raw(|asm| {
                    asm.label("RawCode").call("Delay").ret();
                });
            }),
            // UpdateKeys needs wCurKeys / wNewKeys, which add_inputs used to create alone
            ("a function that only init calls", "UpdateKeys", |gb| {
                gb.define_function_from("PollOnce", Call::new("UpdateKeys"));
                gb.init(Call::new("PollOnce"));
            }),
            // A raw instruction of several lines, a comment before the call
            ("raw text after a comment", "Delay", |gb| {
                let mut code = Asm::new();
                code.raw("ld a, 1 ; one\n    call Delay");
                gb.add_to_main_loop(code.get_main_instrs());
            }),
            // A `;` in a string is not a comment
            ("a db with a string", "Delay", |gb| {
                gb.raw(|asm| {
                    asm.raw("Table: db \"a;b\", LOW(Delay), HIGH(Delay)");
                });
            }),
        ];
        for (path, builtin, program) in paths {
            let mut gb = RustBoy::new();
            program(&mut gb);
            let out = gb.build();
            assert_eq!(
                definitions(&out, builtin),
                1,
                "{} through {}",
                builtin,
                path
            );
            assert_links(&out);
        }
    }

    #[test]
    fn test_a_builtin_called_from_everywhere_is_emitted_once() {
        let mut gb = RustBoy::new();
        gb.define_function_from("Probe", Call::new("GetTileByPixel"));
        gb.call_args("GetTileByPixel", Vec::new());
        gb.add_to_main_loop(Call::new("GetTileByPixel"));
        gb.add_to_main_loop(Call::new("Probe"));
        gb.init(IfCall::is_true("GetTileByPixel", Vec::<Instr>::new()));
        gb.use_function(BuiltinFunction::GetTileByPixel);

        let out = gb.build();
        assert_eq!(definitions(&out, "GetTileByPixel"), 1);
        assert_links(&out);
    }

    #[test]
    fn test_only_used_user_functions_are_emitted() {
        // B24: every user function was emitted, used or not. Now: the ones the program
        // refers to (call, jp, an address, raw code), and the ones those refer to, in
        // registration order
        let mut gb = RustBoy::new();
        gb.define_function("Unused", calling("Unused", &["Delay"]));
        gb.define_function("Leaf", calling("Leaf", &["Leaf"])); // recursive
        gb.define_function("PingA", calling("PingA", &["PingB"])); // unused cycle
        gb.define_function("PingB", calling("PingB", &["PingA"]));
        gb.define_function("Helper", calling("Helper", &["Leaf"]));
        gb.define_function("Called", calling("Called", &["Helper"]));
        gb.define_function("ByAddress", calling("ByAddress", &[]));
        gb.define_function("FromRaw", calling("FromRaw", &[]));
        gb.define_function("Jumped", calling("Jumped", &[]));
        gb.define_function("FromRawText", calling("FromRawText", &[]));
        gb.define_function("InTable", calling("InTable", &[]));
        gb.add_to_main_loop(Call::new("Called"));
        let mut table = Asm::new();
        table
            .ld_hl_label("ByAddress")
            .jp_cond(crate::gb_asm::Condition::Z, "Jumped")
            // Raw text of several lines, the call after a comment
            .raw("ld a, 1 ; one\n    call FromRawText");
        gb.add_to_main_loop(table.get_main_instrs());
        gb.raw(|asm| {
            asm.label("RawCode")
                .call("FromRaw")
                .ret()
                // A `;` in a string, then a reference
                .raw("Pointers: db \"a;b\"\n    dw InTable");
        });

        let out = gb.build();
        assert_links(&out);
        let user_functions: Vec<&str> = [
            "Unused",
            "Leaf",
            "PingA",
            "PingB",
            "Helper",
            "Called",
            "ByAddress",
            "FromRaw",
            "Jumped",
            "FromRawText",
            "InTable",
        ]
        .into_iter()
        .filter(|name| definitions(&out, name) > 0)
        .collect();
        assert_eq!(
            user_functions,
            [
                "Leaf",
                "Helper",
                "Called",
                "ByAddress",
                "FromRaw",
                "Jumped",
                "FromRawText",
                "InTable"
            ]
        );
        for name in &user_functions {
            assert_eq!(definitions(&out, name), 1, "{}", name);
        }
        // Registration order
        let position = |name: &str| out.find(&format!("{}:", name)).unwrap();
        assert!(position("Leaf") < position("Helper") && position("Helper") < position("Called"));
        // The builtin only the unused function calls is not emitted either
        assert_eq!(definitions(&out, "Delay"), 0);
    }

    #[test]
    fn test_keep_function_emits_a_function_nothing_calls() {
        let mut gb = RustBoy::new();
        gb.define_function("FromOutside", calling("FromOutside", &["Delay"]));
        gb.define_function("Unused", calling("Unused", &[]));
        let out = gb.build();
        assert_eq!(definitions(&out, "FromOutside"), 0);

        // Kept: emitted with what it calls; a builtin can be kept by name too
        gb.keep_function("FromOutside")
            .keep_function("GetTileByPixel");
        let out = gb.build();
        for name in ["FromOutside", "Delay", "GetTileByPixel"] {
            assert_eq!(definitions(&out, name), 1, "{}", name);
        }
        assert_eq!(definitions(&out, "Unused"), 0);
        assert_links(&out);
    }

    #[test]
    #[should_panic(expected = "Unknown function 'Missing'")]
    fn test_keep_function_needs_a_function() {
        RustBoy::new().keep_function("Missing");
    }

    #[test]
    fn test_a_routine_is_never_emitted_twice() {
        use crate::gb_std::graphics::utility::memcopy;

        // A user function with the name of a builtin replaces it
        let mut gb = RustBoy::new();
        gb.define_function("Delay", calling("Delay", &[]));
        let call = gb.call("Delay");
        gb.add_to_main_loop(call);
        let out = gb.build();
        assert_eq!(definitions(&out, "Delay"), 1);
        assert!(!out.contains("Delay loop using BC"), "the builtin Delay");

        // Raw code with its own copy of a routine the program calls
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("BgTiles", TileSource::from_raw(&[["$00"; 8]]));
        gb.raw(|asm| {
            asm.emit_all(memcopy());
        });
        let out = gb.build();
        assert_eq!(definitions(&out, "Memcopy"), 1);
        assert_links(&out);
    }

    #[test]
    fn test_get_tile_by_pixel_callers_follow_its_contract() {
        // B23: GetTileByPixel returns the tile address in hl and the tile in a. The
        // unbricked_rustboy brick handler tests a (IfConst, IfA), then blanks the brick
        // through hl (TileRef). Its call is inside a function body (B26).
        let mut gb = RustBoy::new();
        gb.define_const("BRICK_LEFT", 5)
            .define_const("BRICK_RIGHT", 6)
            .define_const("BLANK_TILE", 8);
        gb.add_sprite("Paddle", tiles(1), 16, 128, 0);
        let ball = gb.add_sprite("Ball", tiles(1), 32, 100, 0);
        gb.define_function_from(
            "CheckAndHandleBrick",
            vec![
                boxed(IfConst::eq(
                    Call::with_args("GetTileByPixel", gb.sprites.get_pivot(ball, 0, 1)),
                    "BRICK_LEFT",
                    vec![
                        TileRef::set_tile_label("BLANK_TILE"),
                        TileRef::next_tile(),
                        TileRef::set_tile_label("BLANK_TILE"),
                    ],
                )),
                boxed(IfA::eq(
                    "BRICK_RIGHT",
                    vec![
                        TileRef::set_tile_label("BLANK_TILE"),
                        TileRef::prev_tile(),
                        TileRef::set_tile_label("BLANK_TILE"),
                    ],
                )),
            ],
        );
        gb.add_to_main_loop(Call::new("CheckAndHandleBrick"));
        let asm = gb.build_asm();
        assert_links(&asm.to_asm());
        let mut code = Asm::new();
        code.call("CheckAndHandleBrick").ret();
        let mut code = code.get_main_instrs();
        code.extend(asm.get_chunk(Chunk::Functions).cloned().unwrap());

        // The ball at OAM X 48, Y 57: the pixel above it is (40, 40), map tile (5, 5)
        const BRICK_LEFT: u8 = 5;
        const BRICK_RIGHT: u8 = 6;
        const BLANK_TILE: u8 = 8;
        let hit = 0x9800 + 5 * 32 + 5;
        let map = |addr: u16| format!("${:04X}", addr);
        for (tile, blanked) in [
            (BRICK_LEFT, [hit, hit + 1]),
            (BRICK_RIGHT, [hit, hit - 1]),
            (BLANK_TILE, [0, 0]),
        ] {
            let mut cpu = TestCpu::default();
            cpu.mem.insert("_OAMRAM+4".to_string(), 57);
            cpu.mem.insert("_OAMRAM+5".to_string(), 48);
            cpu.mem.insert(map(hit), tile);
            for (name, value) in [
                ("BRICK_LEFT", BRICK_LEFT),
                ("BRICK_RIGHT", BRICK_RIGHT),
                ("BLANK_TILE", BLANK_TILE),
            ] {
                cpu.consts.insert(name.to_string(), value);
            }
            cpu.run(&code);
            let writes: Vec<Event> = cpu
                .trace
                .iter()
                .filter(|event| matches!(event, Event::Write(..)))
                .cloned()
                .collect();
            let expected: Vec<Event> = blanked
                .iter()
                .filter(|addr| **addr != 0)
                .map(|addr| Event::Write(map(*addr), BLANK_TILE))
                .collect();
            assert_eq!(writes, expected, "tile {}", tile);
        }
    }

    #[test]
    fn test_names_the_program_defines_are_not_functions() {
        // A constant or a variable named like a builtin is not a call to it: the builtin
        // was emitted too, and rgbasm reported `Delay` already defined
        let read_delay = || {
            let mut code = Asm::new();
            code.ld_a_addr_def("Delay");
            code.get_main_instrs()
        };
        let mut gb = RustBoy::new();
        gb.define_const("Delay", 5);
        gb.add_to_main_loop(Asm::new().ld_a_label("Delay").get_main_instrs());
        let out = gb.build();
        assert_eq!(definitions(&out, "Delay"), 0, "{}", out);
        assert_links(&out);

        let mut gb = RustBoy::new();
        gb.vars.create_u8("Delay", 0);
        gb.add_to_main_loop(read_delay());
        let out = gb.build();
        assert_eq!(definitions(&out, "Delay"), 0, "{}", out);
        assert_eq!(out.matches("Delay: db").count(), 1);
        assert_links(&out);

        // A builtin's variables that the program defines already are not created again:
        // in raw code (rgbasm: `wCurKeys` already defined), or as a variable of another
        // type (create_u8 panicked)
        let mut gb = RustBoy::new();
        gb.use_function(BuiltinFunction::UpdateKeys);
        gb.raw(|asm| {
            asm.raw("wCurKeys: db\n    wNewKeys: db");
        });
        let out = gb.build();
        assert_eq!(definitions(&out, "UpdateKeys"), 1);
        assert_eq!(out.matches("wCurKeys:").count(), 1, "{}", out);
        assert_eq!(out.matches("wNewKeys:").count(), 1, "{}", out);
        assert_links(&out);

        // A call in a block comment is no call
        let mut gb = RustBoy::new();
        gb.raw(|asm| {
            asm.raw("Commented: /* call Delay */ ret");
        });
        let out = gb.build();
        assert_eq!(definitions(&out, "Delay"), 0, "{}", out);
        assert_links(&out);

        let mut gb = RustBoy::new();
        gb.vars.create_u16("wCurKeys", 0);
        gb.vars.create_u8("wNewKeys", 0);
        gb.use_function(BuiltinFunction::UpdateKeys);
        let out = gb.build();
        assert_eq!(out.matches("wCurKeys: dw").count(), 1, "{}", out);
        assert_eq!(out.matches("wNewKeys: db").count(), 1, "{}", out);
        assert_links(&out);
    }

    #[test]
    #[should_panic(
        expected = "define_function(\"Inner\"): the body does not define the label `Inner:`"
    )]
    fn test_define_function_needs_its_label() {
        // The body is labelled `Other`: a call to `Inner` could never reach it
        let mut gb = RustBoy::new();
        gb.define_function("Inner", calling("Other", &[]));
    }

    #[test]
    #[should_panic(expected = "invalid function name \"my func\"")]
    fn test_function_name_must_be_an_identifier() {
        RustBoy::new().define_function_from("my func", Vec::<Instr>::new());
    }

    #[test]
    fn test_a_second_entry_point_of_a_function_is_found() {
        // A body with two global labels: a call to the second one emits the function
        let mut body = Asm::new();
        body.label("Blank")
            .ld_a(0)
            .label("BlankWithA")
            .ld_addr_def_a("wTile")
            .ret();
        let mut gb = RustBoy::new();
        gb.vars.create_u8("wTile", 0);
        gb.define_function("Blank", body.get_main_instrs());
        gb.add_to_main_loop(Call::new("BlankWithA"));
        let out = gb.build();
        assert_eq!(definitions(&out, "BlankWithA"), 1, "{}", out);
        assert_links(&out);
    }

    #[test]
    #[should_panic(expected = "TileSource::from_file(\"empty.2bpp\", 0)")]
    fn test_a_tile_file_needs_tiles() {
        // B27: a file blob is copied whole, so a count of 0 cannot mean "skip it"
        TileSource::from_file("empty.2bpp", 0);
    }

    #[test]
    fn test_a_def_in_raw_code_is_not_a_function() {
        // A DEF in raw text defines its name, in every form: no builtin of that name is
        // emitted (rgbasm: `Delay` already defined), nor its variables
        for def in [
            "DEF Delay EQU 5",
            "DEF Delay = 5",
            "def Delay equ 5",
            "REDEF Delay EQU 5",
        ] {
            let mut gb = RustBoy::new();
            gb.raw(move |asm| {
                asm.raw(&format!("{}\n    db Delay", def));
            });
            let out = gb.build();
            assert_eq!(definitions(&out, "Delay"), 0, "{}:\n{}", def, out);
            assert_links(&out);
        }

        // Memcopy, without tiles: nothing else calls it
        let mut gb = RustBoy::new();
        gb.raw(|asm| {
            asm.raw("DEF Memcopy EQU 3\n    db Memcopy");
        });
        let out = gb.build();
        assert_eq!(definitions(&out, "Memcopy"), 0, "{}", out);
        assert_links(&out);

        // The UpdateKeys variables, defined as constants
        let mut gb = RustBoy::new();
        gb.use_function(BuiltinFunction::UpdateKeys);
        gb.raw(|asm| {
            asm.raw("DEF wCurKeys EQU $C100\n    DEF wNewKeys EQU $C101");
        });
        let out = gb.build();
        assert_eq!(definitions(&out, "UpdateKeys"), 1);
        assert!(!out.contains("wCurKeys: db"), "{}", out);
        assert_links(&out);
    }

    #[test]
    fn test_external_symbols_are_not_emitted() {
        // The program's own routines with builtin names, in an INCLUDEd file that build()
        // does not read: declared external, they are not emitted again (rgbasm: already
        // defined), nor the UpdateKeys variables
        use crate::gb_asm::label_check::assert_links_with;

        let included = "UpdateKeys:\n    ret\nDelay:\n    ret\nMemcopy:\n    ret\n";
        let mut gb = RustBoy::new();
        gb.raw(|asm| {
            asm.include("my_routines.inc");
        });
        for name in ["UpdateKeys", "Delay", "Memcopy"] {
            gb.add_to_main_loop(Call::new(name));
            gb.external_symbol(name);
        }
        let out = gb.build();
        for name in ["UpdateKeys", "Delay", "Memcopy"] {
            assert_eq!(definitions(&out, name), 0, "{}:\n{}", name, out);
        }
        assert!(!out.contains("wCurKeys"), "{}", out);
        assert_links_with(&out, &[("my_routines.inc", included)]);
    }

    /// A program with a user function `Jump`, called, and `other` defining `Jump` too
    fn jump_also_defined(other: fn(&mut RustBoy)) -> String {
        let mut gb = RustBoy::new();
        other(&mut gb);
        gb.define_function("Jump", calling("Jump", &[]));
        let call = gb.call("Jump");
        gb.add_to_main_loop(call);
        gb.build()
    }

    #[test]
    #[should_panic(expected = "function `Jump` is also a constant or a label of the program")]
    fn test_a_function_named_like_a_constant_panics() {
        // The function was dropped and `call Jump` went to address 1 (on refactor,
        // rgbasm: `Jump` already defined)
        jump_also_defined(|gb| {
            gb.define_const("Jump", 1);
        });
    }

    #[test]
    #[should_panic(expected = "function `Jump` is also a variable")]
    fn test_a_function_named_like_a_variable_panics() {
        // `call Jump` went into WRAM
        jump_also_defined(|gb| {
            gb.vars.create_u8("Jump", 0);
        });
    }

    #[test]
    #[should_panic(expected = "function `Jump` is also a constant or a label of the program")]
    fn test_a_function_named_like_a_raw_label_or_def_panics() {
        jump_also_defined(|gb| {
            gb.raw(|asm| {
                asm.raw("DEF Jump EQU 2");
            });
        });
    }

    #[test]
    #[should_panic(expected = "function `Jump` is also an external symbol")]
    fn test_a_function_cannot_be_external() {
        // A function is defined either here (define_function) or outside the generated
        // code (external_symbol), not both
        jump_also_defined(|gb| {
            gb.external_symbol("Jump");
        });
    }

    #[test]
    fn test_a_user_function_replaces_a_forced_builtin() {
        // use_function(B) with an uncalled user function B: the user's one was emitted
        // for Delay (whose body names itself) but the builtin for GetTileByPixel
        for builtin in [BuiltinFunction::Delay, BuiltinFunction::GetTileByPixel] {
            let name = builtin.label();
            let mut own = Asm::new();
            own.label(name).ld_a(42).ret();
            let mut gb = RustBoy::new();
            gb.use_function(builtin);
            gb.define_function(name, own.get_main_instrs());
            let out = gb.build();
            assert_eq!(definitions(&out, name), 1, "{}:\n{}", name, out);
            let body = function(&out, name);
            assert!(
                body.contains("ld a, 42"),
                "{}: not the user's:\n{}",
                name,
                body
            );
            assert_links(&out);
        }
    }

    #[test]
    fn test_a_second_entry_point_replaces_a_builtin() {
        // A routine bundle whose body defines UpdateKeys as a second entry point: the
        // builtin was found first, so it was emitted (with wCurKeys/wNewKeys) and the
        // user's routine dropped. It linked, but ran other code.
        let bundle = || {
            let mut body = Asm::new();
            body.label("MyLib").ret().label("UpdateKeys").ld_a(42).ret();
            body.get_main_instrs()
        };
        let called = |gb: &mut RustBoy| {
            gb.add_to_main_loop(Call::new("UpdateKeys"));
        };
        let forced = |gb: &mut RustBoy| {
            gb.use_function(BuiltinFunction::UpdateKeys);
        };
        for (how, reach) in [
            ("called", &called as &dyn Fn(&mut RustBoy)),
            ("forced", &forced),
        ] {
            let mut gb = RustBoy::new();
            gb.define_function("MyLib", bundle());
            reach(&mut gb);
            let out = gb.build();
            assert_eq!(definitions(&out, "MyLib"), 1, "{}:\n{}", how, out);
            assert_eq!(definitions(&out, "UpdateKeys"), 1, "{}:\n{}", how, out);
            assert!(
                function(&out, "UpdateKeys").contains("ld a, 42"),
                "{}: not the user's UpdateKeys:\n{}",
                how,
                out
            );
            assert!(
                !out.contains("wCurKeys"),
                "{}: builtin variables:\n{}",
                how,
                out
            );
            assert_links(&out);
        }
    }

    #[test]
    fn test_keep_function_accepts_a_generated_function() {
        // An animation function is always emitted: once a build has registered it, keeping it
        // does nothing, it is not an "unknown function"
        let mut gb = RustBoy::new();
        let coin = gb.add_sprite("Coin", tiles(2), 80, 72, 0);
        gb.sprites
            .add_animation(coin, "Spin", 0, 1, AnimationType::Loop);
        let first = gb.build();
        gb.keep_function("Anim_Coin_Spin");
        let second = gb.build();
        assert_eq!(definitions(&second, "Anim_Coin_Spin"), 1);
        assert_eq!(first, second);
    }

    #[test]
    fn test_redefining_a_function_moves_its_second_entry_point() {
        // Two functions define the label Entry: the first registered owns it. When that
        // one is redefined without Entry, the label belongs to the other one.
        let with_entry = |name: &str, value: u8| {
            let mut body = Asm::new();
            body.label(name).ret().label("Entry").ld_a(value).ret();
            body.get_main_instrs()
        };
        let mut gb = RustBoy::new();
        gb.define_function("First", with_entry("First", 1));
        gb.define_function("Second", with_entry("Second", 2));
        gb.add_to_main_loop(Call::new("Entry"));
        let out = gb.build();
        assert_eq!(definitions(&out, "First"), 1, "{}", out);
        assert_eq!(definitions(&out, "Second"), 0, "{}", out);

        gb.define_function("First", calling("First", &[]));
        let out = gb.build();
        assert_eq!(definitions(&out, "First"), 0, "{}", out);
        assert_eq!(definitions(&out, "Second"), 1, "{}", out);
        assert_links(&out);
    }

    #[test]
    #[should_panic(expected = "invalid external symbol \"my routine\"")]
    fn test_an_external_symbol_is_an_identifier() {
        RustBoy::new().external_symbol("my routine");
    }

    #[test]
    #[should_panic(expected = "tiles \"Bg\": the file \"bg.2bpp\" has a tile count of 0")]
    fn test_a_tile_file_built_directly_needs_tiles() {
        // TileSource::File is public: the count is checked where the tiles are added
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("Bg", TileSource::File("bg.2bpp".to_string(), 0));
    }

    #[test]
    fn test_a_large_program_builds_quickly() {
        // The function scan once re-read every user function for each word of the
        // program: 100 functions and a 5000-line main loop took minutes. Each body is now
        // read once, when it is registered, each name handled once, and a label map finds
        // second entry points (`call` scanned every body). The bound is generous on purpose (a
        // debug build on a slow CI runner): it only catches that kind of regression.
        let start = std::time::Instant::now();
        let mut gb = RustBoy::new();
        for v in 0..10 {
            gb.vars.create_u8(&format!("wVar{}", v), 0);
        }
        for f in 0..100 {
            let name = format!("Func{}", f);
            let mut body = Asm::new();
            body.label(&name);
            for i in 0..48 {
                body.ld_a_addr_def(&format!("wVar{}", i % 10))
                    .inc(crate::gb_asm::Operand::Reg(crate::gb_asm::Register::A));
            }
            // Each function calls the next one, and has a second entry point
            if f < 99 {
                body.call(&format!("Func{}", f + 1));
            }
            body.label(&format!("Func{}Entry", f)).ret();
            gb.define_function(&name, body.get_main_instrs());
        }
        let mut main = Asm::new();
        for i in 0..5000 {
            main.ld_a_addr_def(&format!("wVar{}", i % 10));
        }
        main.call("Func0");
        gb.add_to_main_loop(main.get_main_instrs());
        // `call` through second entry points: a lookup each, not a scan of every body
        for i in 0..1000 {
            let call = gb.call(&format!("Func{}Entry", i % 100));
            gb.add_to_main_loop(call);
        }

        let out = gb.build();
        let elapsed = start.elapsed();
        assert_eq!(definitions(&out, "Func99"), 1);
        assert!(
            elapsed < std::time::Duration::from_secs(20),
            "build() took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_empty_blobs_are_not_copied() {
        // B27: Memcopy copies at least one byte, so an empty blob made it copy 64 KiB
        // over WRAM, the stack and the I/O registers
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("NoTiles", TileSource::from_raw(&[]));
        gb.tiles
            .add_background("BgTiles", TileSource::from_raw(&[["$00"; 8]]));
        gb.tiles.add_tilemap("NoMap", &[]);
        assert_links(&gb.build());

        // Memcopy runs for real: only BgTiles is copied, to $9000
        let (code, mut cpu) = startup(&mut gb);
        for (blob, size) in [("NoTiles", 0), ("BgTiles", 16), ("NoMap", 0)] {
            cpu.consts16.insert(format!("{0}End - {0}", blob), size);
        }
        for i in 0..16 {
            cpu.mem.insert(format!("BgTiles+{}", i), 100 + i);
        }
        cpu.run(&code);
        let vram: Vec<(String, u8)> = cpu
            .trace
            .iter()
            .filter_map(|event| match event {
                Event::Write(addr, value) if addr.starts_with('$') => Some((addr.clone(), *value)),
                _ => None,
            })
            .collect();
        let expected: Vec<(String, u8)> = (0..16)
            .map(|i| (format!("${:04X}", 0x9000 + i), 100 + i as u8))
            .collect();
        assert_eq!(vram, expected);
        let copies = cpu
            .trace
            .iter()
            .filter(|event| **event == Event::Call("Memcopy".to_string()));
        assert_eq!(copies.count(), 1);

        // With only empty blobs, nothing is copied and Memcopy is not emitted
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("NoTiles", TileSource::from_raw(&[]));
        gb.tiles.add_tilemap("NoMap", &[]);
        let out = gb.build();
        assert_links(&out);
        assert!(!out.contains("Memcopy"), "{}", out);
        assert!(
            out.contains("NoTiles:") && out.contains("NoMapEnd:"),
            "labels kept"
        );
    }
}
