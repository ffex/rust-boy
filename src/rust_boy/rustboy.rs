//! Main RustBoy struct - the high-level Game Boy development API

use crate::gb_asm::labels::code_lines;
use crate::gb_asm::{Block, Expr, Instr, JumpTarget, LabelAllocator, R8, Section, is_identifier};
use crate::gb_std::flow::Emittable;
use crate::gb_std::graphics::sprites::{clear_objects_screen, initialize_objects_screen};
use crate::hw;

use super::functions::{BuiltinFunction, FunctionRegistry, defines};
use super::inputs::InputManager;
use super::layout::{Chunk, Layout};
use super::sprites::{SpriteManager, SpriteSize, SpriteTiles, check_name};
use super::tiles::{TileManager, TilemapArea};
use super::variables::VariableManager;

/// The palette `build()` writes to `rBGP`, `rOBP0` and `rOBP1` at start-up: colour i
/// shows shade i (0 lightest, 3 darkest), so both object palettes look like the
/// background one until the program changes them
const DEFAULT_PALETTE: u8 = 0b11100100;

/// The `WRAM0` section `build()` opens for the `raw()` code of `Chunk::Data` when no
/// variable section comes before it and it opens none itself
const RAW_DATA_SECTION: &str = "Raw Data";

/// Whether `code` starts with a `SECTION` (before any other code; comments are skipped)
fn opens_section(code: &[Instr]) -> bool {
    for instr in code {
        match instr {
            Instr::Comment { .. } => continue,
            Instr::Section { .. } => return true,
            Instr::Raw { line } => {
                let lines = code_lines(line);
                match lines.iter().map(|l| l.trim()).find(|l| !l.is_empty()) {
                    Some(first) => {
                        // The keyword, not a label that starts with it (`SectionTable:`)
                        let word = first.split_whitespace().next().unwrap_or("");
                        return word.eq_ignore_ascii_case("SECTION");
                    }
                    None => continue, // a comment only
                }
            }
            _ => return false,
        }
    }
    false
}

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
    /// The code written with [`RustBoy::raw`], in chunks; it owns the program's label
    /// allocator ([`RustBoy::labels`]), shared with the sprite manager, so every
    /// generated label comes from one sequence
    asm: Layout,

    /// Tile manager with automatic VRAM allocation
    pub tiles: TileManager,

    /// Variable manager with automatic WRAM allocation
    pub vars: VariableManager,

    /// Sprite manager with automatic OAM and tile handling
    pub sprites: SpriteManager,

    /// Function registry for auto-including builtin functions
    functions: FunctionRegistry,

    /// Custom constants defined by the user
    constants: Vec<(String, String)>,

    /// Init code to run before the main loop
    init_code: Vec<Instr>,

    /// Main loop code
    main_loop_code: Vec<Instr>,

    /// Animation delay value in frames (higher = slower animations)
    animation_delay: u8,

    /// The tilemap the background shows (LCDC bit 3)
    background_tilemap: TilemapArea,
}

impl RustBoy {
    /// Create a new RustBoy instance
    pub fn new() -> Self {
        let asm = Layout::new();
        Self {
            sprites: SpriteManager::new(asm.labels().clone()),
            asm,
            tiles: TileManager::new(),
            vars: VariableManager::new(),
            functions: FunctionRegistry::new(),
            constants: Vec::new(),
            init_code: Vec::new(),
            main_loop_code: Vec::new(),
            animation_delay: 8, // Default: update animation every 8 frames
            background_tilemap: TilemapArea::default(),
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

    /// Set the tilemap the background shows: [`TilemapArea::Map9800`] (the default, as
    /// on the hardware) or [`TilemapArea::Map9C00`]; `build()` writes it to LCDC
    /// (`LCDCF_BG9C00`) when it turns the LCD on (B19)
    ///
    /// Put a tilemap there with `tiles.add_tilemap_at`. `GetTileByPixel` (and so
    /// `get_pivot` + `GetTileByPixel` collisions) reads the `$9800` map only.
    pub fn set_background_tilemap(&mut self, area: TilemapArea) -> &mut Self {
        self.background_tilemap = area;
        self
    }

    /// The tilemap the background shows (see [`RustBoy::set_background_tilemap`])
    pub fn background_tilemap(&self) -> TilemapArea {
        self.background_tilemap
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

    /// Get the next number of the program's label sequence ([`RustBoy::labels`]), which
    /// every generated label uses: `If`s, key checks, sprite moves
    pub fn next_label_counter(&mut self) -> usize {
        self.labels().next_id()
    }

    /// Generate a unique label with prefix
    pub fn unique_label(&mut self, prefix: &str) -> String {
        format!("{}_{}", prefix, self.next_label_counter())
    }

    /// The allocator of this program's generated labels: the `If`s, the key checks, the
    /// sprite moves, the start-up code and the animation dispatcher all take their labels
    /// from it, so every label is unique
    ///
    /// Pass it to the `gb_std` snippets you mix into a `RustBoy` program
    /// (`check_key`, `Sprite::move_*_limit`), and to [`Emittable::emit`] if you emit code
    /// yourself: a separate allocator starts again at 0 and would repeat labels that
    /// `RustBoy` already emitted. In a [`RustBoy::raw`] closure it is `asm.labels()`.
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
        self.asm.labels()
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
        let instrs = code.emit(self.asm.labels());
        self.init_code.extend(instrs);
        self
    }

    /// Escape hatch: execute raw assembly operations
    ///
    /// This allows advanced users to mix high-level and low-level code. The closure gets
    /// the program's [`Layout`] (the builder methods of an `Asm`, by [`Chunk`]) and writes
    /// to [`Chunk::Main`] unless it switches with `asm.chunk(..)`; every call starts in
    /// `Chunk::Main` again. `build()` keeps every chunk (B15), each one after the code
    /// it generates for that chunk:
    /// - `Main` (the default): after the main loop's `jp Main`, so it runs only if it is
    ///   called or jumped to: start it with a label.
    /// - `Init`: at start-up, after the [`RustBoy::init`] code, before the LCD is turned on.
    /// - `MainLoop`: in the main loop, every frame, after the [`RustBoy::add_to_main_loop`]
    ///   code, before `jp Main`.
    /// - `Functions`: after the generated functions (label your routines; a routine that
    ///   calls a builtin gets it emitted, like any code).
    /// - `Header`, `Constants`, `Tiles`, `Tilemap`: after the generated ones (`Header` is
    ///   inside the header section, after its padding).
    /// - `Data`: after the variable sections, so inside the last of them (a `WRAM0`
    ///   section) unless the raw code opens its own `SECTION`. In a program without
    ///   variables, raw `Data` code that does not start with a `SECTION` gets a `WRAM0`
    ///   section of its own, `SECTION "Raw Data", WRAM0` (it would land in ROM). A RAM
    ///   section only reserves space (labels, `ds n`): code or data there makes `build()`
    ///   panic.
    ///
    /// # Example
    /// ```
    /// use rust_boy::rust_boy::Chunk;
    /// use rust_boy::gb_std::flow::Call;
    /// use rust_boy::rust_boy::RustBoy;
    /// use rust_boy::hw;
    ///
    /// let mut gb = RustBoy::new();
    /// gb.add_to_main_loop(Call::new("LoadAnswer"));
    /// gb.raw(|asm| {
    ///     // A routine: labelled, and called from the main loop
    ///     asm.label("LoadAnswer").ld_a(0x42).ret();
    ///     // Code that runs every frame, after the main loop code
    ///     asm.chunk(Chunk::MainLoop).ld_addr_def_a(hw::SCX);
    /// });
    /// let out = gb.build();
    /// assert!(out.contains("LoadAnswer:") && out.contains("ld [rSCX], a"));
    /// ```
    pub fn raw<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut Layout),
    {
        self.asm.chunk(Chunk::Main);
        f(&mut self.asm);
        self.asm.chunk(Chunk::Main);
        self
    }

    /// What the `raw()` code wrote to `chunk`
    fn raw_chunk(&self, chunk: Chunk) -> Vec<Instr> {
        self.asm.get_chunk(chunk).cloned().unwrap_or_default()
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
    /// `dw Name`, raw lines (`raw`, one or several lines); not in comments or strings.
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
    /// use rust_boy::gb_asm::Block;
    /// use rust_boy::rust_boy::RustBoy;
    ///
    /// let mut gb = RustBoy::new();
    /// let mut body = Block::new();
    /// body.label("OnInterrupt").ret();
    /// gb.define_function("OnInterrupt", body.into_instrs());
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
        let mut asm = Block::new();
        asm.label(name);
        asm.emit_all(body.emit(self.asm.labels()));
        asm.ret();
        self.functions
            .register_user_function(name, asm.into_instrs());
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
        self.build_asm().program().to_asm()
    }

    /// The program [`build`](Self::build) prints, as instructions in chunks (its
    /// [`Layout::program`] is the whole program)
    pub(crate) fn build_asm(&mut self) -> Layout {
        // Start fresh assembly. The code generated here takes its labels after every
        // label handed out so far, from a fork of the program's allocator, so a second
        // build gives the same labels (B14)
        let mut asm = Layout::with_labels(self.labels().fork());
        let labels = asm.labels().clone();

        // === HEADER CHUNK ===
        asm.chunk(Chunk::Header);
        asm.include_hardware();
        asm.emit_all(crate::gb_std::utility::header_section());
        // Each chunk the `raw()` code wrote goes after the code generated for it (B15)
        asm.emit_all(self.raw_chunk(Chunk::Header));

        // === CONSTANTS CHUNK ===
        asm.chunk(Chunk::Constants);
        for (name, value) in &self.constants {
            asm.def(name, value);
        }
        asm.emit_all(self.raw_chunk(Chunk::Constants));

        // === INIT CHUNK ===
        // In two parts, before and after the variable initialisation, which is emitted
        // once the functions are known: a builtin may need variables (B26)
        let mut startup = Block::new();

        // Entry point
        startup.label("EntryPoint");
        startup.call("WaitVBlank");

        // Turn off screen for safe VRAM access
        startup.ld_a(hw::LCDCF_OFF.value);
        startup.ld_addr_def_a(hw::LCDC);

        // Copy the tile data to VRAM (empty blobs are skipped, B27)
        startup.emit_all(self.tiles.generate_memcopy_calls());

        // Clear the whole OAM, with or without sprites: objects are always turned on
        // below, and OAM holds garbage at power-on (B28)
        startup.emit_all(initialize_objects_screen());
        startup.emit_all(clear_objects_screen(&labels));
        if !self.sprites.is_empty() {
            startup.emit_all(self.sprites.generate_init_code());
        }

        // Default palettes, every one of them (OBP1 too, B28)
        startup.ld_a(DEFAULT_PALETTE);
        for palette in [hw::BGP, hw::OBP0, hw::OBP1] {
            startup.ld_addr_def_a(palette);
        }

        // Then the variables (below), then the user init code, after every default it
        // may want to change: variables, animations, palettes, OAM (B11). The LCD is
        // still off, so it can write VRAM.
        let mut finish = Block::new();
        finish.emit_all(self.init_code.clone());
        finish.emit_all(self.raw_chunk(Chunk::Init));

        // Turn on screen, with the sprite size chosen by set_sprite_size, and the
        // background map chosen by set_background_tilemap (`LCDCF_BG9800` is 0, so it is
        // left out, as before B19)
        let mut lcdc = Expr::from(hw::LCDCF_ON)
            | hw::LCDCF_BGON
            | hw::LCDCF_OBJON
            | self.sprites.size().lcdc_flag();
        if self.background_tilemap != TilemapArea::Map9800 {
            lcdc = lcdc | self.background_tilemap.lcdc_bg_flag();
        }
        finish.ld(R8::A, lcdc);
        finish.ld_addr_def_a(hw::LCDC);
        let startup = startup.into_instrs();
        let finish = finish.into_instrs();

        // === MAIN LOOP CHUNK ===
        asm.chunk(Chunk::MainLoop);

        asm.label("Main");
        asm.call("WaitNotVBlank");
        asm.call("WaitVBlank");

        // Generate animation calls at start of main loop
        if self.sprites.has_animations() {
            asm.emit_all(
                self.sprites
                    .generate_animation_calls(&labels, self.animation_delay),
            );
        }

        // Emit main loop code, then the raw main loop code
        asm.emit_all(self.main_loop_code.clone());
        asm.emit_all(self.raw_chunk(Chunk::MainLoop));

        // Jump back to main loop
        asm.jp("Main");

        // === TILES CHUNK ===
        asm.chunk(Chunk::Tiles);
        asm.emit_all(self.tiles.generate_tile_data());
        asm.emit_all(self.raw_chunk(Chunk::Tiles));

        // === TILEMAP CHUNK ===
        asm.chunk(Chunk::Tilemap);
        asm.emit_all(self.tiles.generate_tilemap_data());
        asm.emit_all(self.raw_chunk(Chunk::Tilemap));

        // Include any raw assembly that was added (legacy Main chunk)
        let existing = self.raw_chunk(Chunk::Main);
        if !existing.is_empty() {
            asm.chunk(Chunk::Main);
            asm.emit_all(existing);
        }
        // The raw functions and data are emitted at the end of their chunks, but their
        // code is part of the program now
        let raw_functions = self.raw_chunk(Chunk::Functions);
        let raw_data = self.raw_chunk(Chunk::Data);

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
        code.extend([
            startup.as_slice(),
            finish.as_slice(),
            raw_functions.as_slice(),
            raw_data.as_slice(),
        ]);
        code.extend(animations.iter().map(|(_, body)| body.as_slice()));
        let functions = self.functions.generate_used(&code, self.vars.names());
        asm.chunk(Chunk::Functions);
        asm.emit_all(functions.code);

        for (name, body) in animations {
            // Known to `call` from now on; emitted here, not scanned as a user function
            self.functions.register_generated(&name);
            asm.emit_all(body);
        }
        asm.emit_all(raw_functions);

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
        if !raw_data.is_empty() && self.vars.is_empty() && !opens_section(&raw_data) {
            // No variable section before it: the raw data would land in the ROM0 section
            // of the code, so it gets a WRAM0 section of its own
            asm.section(Section::wram0(RAW_DATA_SECTION));
        }
        asm.emit_all(raw_data);

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
    /// gb.add_to_main_loop(asm.into_instrs());
    ///
    /// // If statement (its labels come from the program's allocator, `labels()`)
    /// gb.add_to_main_loop(If::eq(left, right, body));
    /// ```
    pub fn add_to_main_loop(&mut self, mut code: impl Emittable) -> &mut Self {
        let instrs = code.emit(self.asm.labels());
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
            .extend(inputs.generate_code(self.asm.labels()));

        self
    }

    /// Add a sprite with its tile in one call
    /// Returns the sprite ID for later reference
    ///
    /// The tiles go to VRAM after the sprite tiles already added (also those added with
    /// `tiles.add_sprite` alone), and the sprite's tile index is where they went: the
    /// tile manager is the one source of both (B18). This is the only way to add a
    /// sprite: `SpriteManager::add` is no longer public.
    ///
    /// # Panics
    /// - If `name` is not a valid RGBDS identifier, or another sprite has it: the name
    ///   becomes part of labels.
    /// - In 8x16 mode, if `tile_source` has an odd number of tiles, or the sprite would
    ///   start on an odd tile (after an odd number of tiles added with `tiles.add_sprite`).
    /// - If the tiles do not fit in the 256 sprite tiles ($8000-$8FFF), OAM already holds
    ///   40 sprites, or `x` is above 247 or `y` above 239 (OAM X = x + 8 and OAM Y =
    ///   y + 16 are bytes).
    pub fn add_sprite(
        &mut self,
        name: &str,
        tile_source: super::tiles::TileSource,
        x: u8,
        y: u8,
        flags: u8,
    ) -> super::sprites::SpriteId {
        let count = tile_source.tile_count();
        let id = self.tiles.add_sprite(name, tile_source);
        let tiles = SpriteTiles {
            id,
            first: self.tiles.sprite_tile_index(id),
            count: u16::try_from(count).unwrap_or(u16::MAX),
        };
        self.sprites.add(name, tiles, x, y, flags)
    }

    /// Give sprite `sprite` more tiles, from another source: frames spread over several
    /// `.2bpp` files, for example
    ///
    /// The tiles go to VRAM right after the sprite's own tiles (their label is `name`,
    /// copied at start-up like any tiles), and they count as the sprite's: its
    /// animations can use them as the next frames (an animation steps through
    /// contiguous tiles). Call it before adding the animations that use them.
    ///
    /// Composite (16x16) sprites are not supported: the right half's tiles always follow
    /// the left half's, so neither half can be extended.
    ///
    /// # Example
    /// ```
    /// use rust_boy::rust_boy::{AnimationType, RustBoy, TileSource};
    ///
    /// let mut gb = RustBoy::new();
    /// let player = gb.add_sprite("Player", TileSource::from_file("idle.2bpp", 1), 80, 72, 0);
    /// gb.add_sprite_tiles(player, "PlayerWalk", TileSource::from_file("walk.2bpp", 3));
    /// // Frames 0 to 3: the idle tile, then the three walk tiles
    /// gb.sprites.add_animation(player, "Walk", 0, 3, AnimationType::Loop);
    /// ```
    ///
    /// # Panics
    /// - If there is no sprite `sprite`.
    /// - If other sprite tiles were added after the sprite's (another sprite, or
    ///   `tiles.add_sprite`): the new tiles would not follow the sprite's, so its frames
    ///   would not be contiguous. Add the tiles right after the sprite.
    /// - If the tiles do not fit in the 256 sprite tiles, or in 8x16 mode if their count
    ///   is odd.
    pub fn add_sprite_tiles(
        &mut self,
        sprite: super::sprites::SpriteId,
        name: &str,
        source: super::tiles::TileSource,
    ) -> super::tiles::TileId {
        let (after, sprite_name) = self.sprites.tile_after(sprite);
        let next = self.tiles.next_sprite_tile();
        if next != after {
            panic!(
                "add_sprite_tiles(\"{}\"): the tiles of sprite \"{}\" end before tile {}, but \
                 other sprite tiles were added after them (the next free tile is {}), so the new \
                 tiles would not follow the sprite's; add them right after the sprite",
                name, sprite_name, after, next
            );
        }
        let count = source.tile_count();
        let id = self.tiles.add_sprite(name, source);
        self.sprites
            .extend_tiles(sprite, u16::try_from(count).unwrap_or(u16::MAX));
        id
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
    /// - If `x` is above 239 (the right half's OAM X, x + 16, is a byte), and in the cases
    ///   of [`RustBoy::add_sprite`] for each half.
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

        // The right half is 8 pixels to the right, and its OAM X, x + 16, is a byte (B17)
        let right_x = x
            .checked_add(hw::TILE_WIDTH)
            .filter(|right_x| right_x.checked_add(hw::OAM_X_OFFSET).is_some())
            .unwrap_or_else(|| {
                panic!(
                    "add_sprite_16x16(\"{}\"): x = {} is too large: the right half is at x + 8, \
                     and its OAM X, x + 16, must fit in a byte, so x is at most {}",
                    name,
                    x,
                    u8::MAX - hw::TILE_WIDTH - hw::OAM_X_OFFSET
                )
            });

        // Create the left sprite
        let left_name = format!("{}_left", name);
        let left_sprite = self.add_sprite(&left_name, left_tiles, x, y, flags);

        // Create the right sprite
        let right_name = format!("{}_right", name);
        let right_sprite = self.add_sprite(&right_name, right_tiles, right_x, y, flags);
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
            let mut body = Block::new();
            body.label(name).ret();
            gb.define_function(name, body.into_instrs());
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
    fn test_sprite_tiles_have_one_source() {
        // B18: the sprite manager counted tile indices on its own and the tile manager
        // VRAM addresses on its own: tiles added with `gb.tiles.add_sprite` moved the
        // VRAM copy of the next sprite but not its OAM tile index, so it showed other tiles
        let mut gb = RustBoy::new();
        let paddle = gb.add_sprite("Paddle", tiles(1), 16, 128, 0);
        gb.tiles.add_sprite("Extra", tiles(3)); // tiles 1 to 3, for no sprite
        let ball = gb.add_sprite("Ball", tiles(1), 32, 100, 0);
        assert_eq!(tile_of(&gb, paddle), (0, 0x8000));
        assert_eq!(tile_of(&gb, ball), (4, 0x8040));

        // The start-up code writes that tile index to Ball's OAM entry (entry 1)
        let cpu = run_startup(&mut gb);
        assert_mem(&cpu, &oam(4 + 2), 4);
        assert_links(&gb.build());
    }

    // ==================== Memory limits (B17) ====================

    use crate::rust_boy::panic_message;

    #[test]
    fn test_sprite_tiles_must_fit_in_their_vram_block() {
        // B17: sprite tiles past $8FFF ran into the background tiles; the tile count was
        // cut to a u8 (257 tiles counted as 1) and the u8 tile index wrapped at 256
        let mut gb = RustBoy::new();
        let a = gb.add_sprite("A", tiles(128), 0, 0, 0);
        let b = gb.add_sprite("B", tiles(128), 0, 0, 0);
        assert_eq!(tile_of(&gb, a), (0, 0x8000));
        assert_eq!(tile_of(&gb, b), (128, 0x8800));
        let message = panic_message(|| gb.add_sprite("C", tiles(1), 0, 0, 0));
        assert!(
            message.contains("no room for sprite tiles \"C\" (1 tiles)")
                && message.contains("0 bytes left"),
            "{}",
            message
        );

        let mut gb = RustBoy::new();
        let message = panic_message(|| gb.add_sprite("Big", tiles(257), 0, 0, 0));
        assert!(
            message.contains("sprite tiles \"Big\" (257 tiles)"),
            "{}",
            message
        );
    }

    #[test]
    fn test_a_sprite_with_tiles_from_two_sources() {
        // Frames spread over two blobs (e.g. two .2bpp files): without add_sprite_tiles
        // the frames past the first blob are not the sprite's, and the animation panics
        let mut gb = RustBoy::new();
        gb.add_sprite("Other", tiles(2), 0, 0, 0);
        let player = gb.add_sprite("Player", tiles(1), 80, 72, 0);
        let message = panic_message(|| {
            let mut gb = RustBoy::new();
            let player = gb.add_sprite("Player", tiles(1), 80, 72, 0);
            gb.tiles.add_sprite("PlayerMore", tiles(3));
            gb.sprites
                .add_animation(player, "Walk", 0, 3, AnimationType::Loop);
        });
        assert!(
            message.contains("use RustBoy::add_sprite_tiles"),
            "{}",
            message
        );

        let more = gb.add_sprite_tiles(player, "PlayerMore", tiles(3));
        assert_eq!(tile_of(&gb, player), (2, 0x8020));
        assert_eq!(
            gb.tiles.get_address(more),
            Some(0x8030),
            "right after the sprite's"
        );
        let walk = gb
            .sprites
            .add_animation(player, "Walk", 0, 3, AnimationType::Loop);
        gb.sprites.set_initial_animation(player, walk);
        let out = gb.build();
        assert_links(&out);
        assert!(out.contains("PlayerMore:"), "the tiles are copied: {}", out);

        // The animation plays the four frames: tiles 2 (its own), then 3, 4, 5 (the others)
        let mut code = gb
            .sprites
            .generate_animation_calls(&LabelAllocator::new(), 1);
        code.push(Instr::Ret);
        for (_, body) in gb.sprites.generate_animation_functions() {
            code.extend(body);
        }
        let mut cpu = TestCpu::default();
        cpu.mem.insert(oam(4 + 2), 2);
        cpu.mem.insert("wFrameCounter".to_string(), 0);
        for (name, value) in gb.sprites.get_animation_variables() {
            cpu.mem.insert(name, value);
        }
        let frames: Vec<u8> = (0..6)
            .map(|_| {
                cpu.run(&code);
                cpu.mem[&oam(4 + 2)]
            })
            .collect();
        assert_eq!(frames, [3, 4, 5, 2, 3, 4]);
    }

    #[test]
    fn test_add_sprite_tiles_must_follow_the_sprite() {
        let mut gb = RustBoy::new();
        let player = gb.add_sprite("Player", tiles(1), 80, 72, 0);
        gb.add_sprite("Ball", tiles(1), 0, 0, 0);
        let message = panic_message(|| gb.add_sprite_tiles(player, "PlayerMore", tiles(3)));
        assert!(
            message.contains("tiles of sprite \"Player\" end before tile 1")
                && message.contains("would not follow"),
            "{}",
            message
        );
        // In 8x16 mode a frame is two tiles
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let player = gb.add_sprite("Player", tiles(2), 80, 72, 0);
        let message = panic_message(|| gb.add_sprite_tiles(player, "PlayerMore", tiles(3)));
        assert!(
            message.contains("the tile count must be even"),
            "{}",
            message
        );
    }

    #[test]
    fn test_background_tiles_must_fit_before_the_tilemap() {
        // B17: background tiles past $97FF overwrote the tilemap at $9800
        let mut gb = RustBoy::new();
        gb.tiles.add_background("Tiles", tiles(100));
        let more = gb.tiles.add_background("More", tiles(28));
        assert_eq!(gb.tiles.get_address(more), Some(0x9640));
        let message = panic_message(|| gb.tiles.add_background("Extra", tiles(1)));
        assert!(
            message.contains("no room for background tiles \"Extra\" (1 tiles)"),
            "{}",
            message
        );
    }

    #[test]
    fn test_at_most_40_sprites() {
        // B17: a 41st sprite was written past the OAM ($FEA0 on)
        let mut gb = RustBoy::new();
        for i in 0..40 {
            gb.add_sprite(&format!("S{}", i), tiles(1), 0, 0, 0);
        }
        let message = panic_message(|| gb.add_sprite("S40", tiles(1), 0, 0, 0));
        assert!(
            message.contains("sprite \"S40\" does not fit in OAM, which holds 40 sprites"),
            "{}",
            message
        );
    }

    #[test]
    fn test_sprite_position_must_fit_in_oam() {
        // B17: y + 16 and x + 8 overflowed a u8 when the start-up code was generated
        let mut gb = RustBoy::new();
        gb.add_sprite("Low", tiles(1), 247, 239, 0);
        let message = panic_message(|| gb.add_sprite("Lower", tiles(1), 0, 240, 0));
        assert!(
            message.contains("y = 240") && message.contains("at most 239"),
            "{}",
            message
        );
        let message = panic_message(|| gb.add_sprite("Right", tiles(1), 248, 0, 0));
        assert!(
            message.contains("x = 248") && message.contains("at most 247"),
            "{}",
            message
        );
        // The right half of a 16x16 sprite is 8 pixels further
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let message =
            panic_message(|| gb.add_sprite_16x16("Player", tiles(2), tiles(2), 245, 0, 0));
        assert!(
            message.contains("add_sprite_16x16(\"Player\")") && message.contains("at most 239"),
            "{}",
            message
        );
    }

    // ==================== Tilemaps (B19) ====================

    #[test]
    fn test_a_second_tilemap_on_one_map_panics() {
        // B19: every tilemap went to $9800, so the second one silently replaced the first
        let mut gb = RustBoy::new();
        gb.tiles.add_tilemap("Level", &[[1u8; 32]; 18]);
        let message = panic_message(|| gb.tiles.add_tilemap("Window", &[[2u8; 32]; 18]));
        assert!(
            message.contains("tilemap \"Window\"")
                && message.contains("$9800")
                && message.contains("\"Level\""),
            "{}",
            message
        );
    }

    #[test]
    #[should_panic(expected = "tilemap \"Tall\" has 33 rows, but a map has 32")]
    fn test_a_tilemap_has_at_most_32_rows() {
        // B19: a 33rd row ran into the next map ($9C00), or out of VRAM from $9C00
        RustBoy::new().tiles.add_tilemap("Tall", &[[0u8; 32]; 33]);
    }

    #[test]
    fn test_a_tilemap_at_9c00() {
        let mut gb = RustBoy::new();
        gb.tiles
            .add_background("BgTiles", TileSource::from_raw(&[["$00"; 8]]));
        let level = gb.tiles.add_tilemap("Level", &[[1u8; 32]; 2]);
        let hud = gb
            .tiles
            .add_tilemap_at("Hud", TilemapArea::Map9C00, &[[2u8; 32]; 2]);
        assert_eq!(gb.tiles.get_address(level), Some(0x9800));
        assert_eq!(gb.tiles.get_address(hud), Some(0x9C00));
        // By default the background shows $9800: LCDC as before
        let out = gb.build();
        assert_links(&out);
        assert_eq!(
            lcdc_on(&out),
            "ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ8"
        );

        // Memcopy runs for real: each map lands at its own address
        let (code, mut cpu) = startup(&mut gb);
        for (blob, size) in [("BgTiles", 16), ("Level", 64), ("Hud", 64)] {
            cpu.consts16.insert(format!("{0}End - {0}", blob), size);
        }
        for i in 0..64 {
            cpu.mem.insert(format!("BgTiles+{}", i % 16), 0);
            cpu.mem.insert(format!("Level+{}", i), 1);
            cpu.mem.insert(format!("Hud+{}", i), 2);
        }
        cpu.run(&code);
        for i in 0..64 {
            assert_eq!(cpu.mem[&format!("${:04X}", 0x9800 + i)], 1);
            assert_eq!(cpu.mem[&format!("${:04X}", 0x9C00 + i)], 2);
        }

        // The background can show the $9C00 map instead
        gb.set_background_tilemap(TilemapArea::Map9C00);
        assert_eq!(gb.background_tilemap(), TilemapArea::Map9C00);
        let out = gb.build();
        assert_links(&out);
        assert_eq!(
            lcdc_on(&out),
            "ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ8 | LCDCF_BG9C00"
        );
    }

    #[test]
    #[should_panic(expected = "\"Player\" would start on tile 1, an odd one")]
    fn test_8x16_sprite_after_an_odd_number_of_tiles_panics() {
        // In 8x16 mode the hardware ignores bit 0 of the tile index: tiles added with
        // `gb.tiles.add_sprite` must keep the next sprite on an even tile
        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        gb.tiles.add_sprite("Extra", tiles(1));
        gb.add_sprite("Player", tiles(2), 80, 72, 0);
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
        assert!(walk.contains("cp a, 4"), "{}", walk);
        assert!(walk.contains("cp a, 8"), "{}", walk);
        // The reset loads the tile before the first frame, then steps onto it
        assert!(walk.contains("ld a, 2"), "{}", walk);
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
        assert!(spin.contains("cp a, 6"), "{}", spin);
        // Frame 0 is tile 0: the reset loads 255, and `inc a` wraps it to 0
        assert!(spin.contains("ld a, 255"), "{}", spin);
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
            gb.sprite_size().lcdc_flag().name
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
        let mut own = Block::new();
        own.ld_a(1)
            .ld_addr_def_a("wAnim_Coin_Dir")
            .ld_a(0b00011011)
            .ld_addr_def_a("rBGP");
        gb.init(own.into_instrs());

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
        let mut user = Block::new();
        user.ld_a(5).ld_addr_def_a("rSCX");
        gb.init(user.into_instrs());

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

    // ==================== raw() (B15) ====================

    /// The text of chunk `chunk` of `gb`'s program
    fn chunk_text(gb: &mut RustBoy, chunk: Chunk) -> String {
        gb.build_asm()
            .get_chunk(chunk)
            .map(|code| code.iter().map(|instr| format!("{}\n", instr)).collect())
            .unwrap_or_default()
    }

    #[test]
    fn test_raw_keeps_every_chunk() {
        // B15: build() copied only the Main chunk of raw(): code written after
        // `asm.chunk(..)` in the closure, and in every later raw() call, was dropped
        let mut gb = RustBoy::new();
        gb.add_to_main_loop(Call::new("RawFunc"));
        gb.raw(|asm| {
            asm.label("RawMain").ret();
            asm.chunk(Chunk::Constants).def("RAW_CONST", 5);
            asm.chunk(Chunk::Functions)
                .label("RawFunc")
                .ld(R8::A, "RAW_CONST")
                .call("UpdateKeys")
                .ret();
            asm.chunk(Chunk::Data).section(Section::wram0("RawData"));
            asm.raw("wRawData: db");
            asm.chunk(Chunk::Tiles).label("RawTiles");
            asm.chunk(Chunk::Tilemap).label("RawMap");
        });
        // A later call starts in the Main chunk again
        gb.raw(|asm| {
            asm.label("Second").ret();
        });

        for (chunk, label) in [
            (Chunk::Main, "RawMain:"),
            (Chunk::Main, "Second:"),
            (Chunk::Constants, "DEF RAW_CONST EQU 5"),
            (Chunk::Functions, "RawFunc:"),
            (Chunk::Data, "wRawData: db"),
            (Chunk::Tiles, "RawTiles:"),
            (Chunk::Tilemap, "RawMap:"),
        ] {
            let text = chunk_text(&mut gb, chunk);
            assert!(
                text.contains(label),
                "{} not in {:?}:\n{}",
                label,
                chunk,
                text
            );
        }
        // UpdateKeys, which only the raw function calls, is emitted, with its variables
        let out = gb.build();
        assert!(
            out.contains("UpdateKeys:") && out.contains("wCurKeys: db"),
            "{}",
            out
        );
        assert_links(&out);
    }

    #[test]
    fn test_raw_data_always_lands_in_wram() {
        // Raw Data code without a SECTION, in a program without variables, used to land
        // in the ROM0 section of the code (`wLonely: db` at $018F)
        let data_text = |gb: &mut RustBoy| chunk_text(gb, Chunk::Data);
        let mut gb = RustBoy::new();
        gb.raw(|asm| {
            asm.chunk(Chunk::Data).comment("my data");
            asm.raw("wLonely: db");
        });
        let data = data_text(&mut gb);
        assert!(
            data.starts_with("SECTION \"Raw Data\", WRAM0\n"),
            "a WRAM0 section first:\n{}",
            data
        );
        assert_links(&gb.build());

        // Raw data that opens its own section (typed or in a raw line) gets none
        for open in [
            |asm: &mut Layout| {
                asm.section(Section::wram0("Mine"));
            },
            |asm: &mut Layout| {
                asm.raw("  ; mine\n  section \"Mine\", HRAM");
            },
        ] {
            let mut gb = RustBoy::new();
            gb.raw(|asm| {
                asm.chunk(Chunk::Data);
                open(asm);
                asm.raw("hByte: db");
            });
            let data = data_text(&mut gb);
            assert!(!data.contains("Raw Data"), "{}", data);
            assert_links(&gb.build());
        }

        // A label that starts with "Section" is not the keyword: the default section is
        // still added
        let mut gb = RustBoy::new();
        gb.raw(|asm| {
            asm.chunk(Chunk::Data).raw("SectionTable: db");
        });
        let data = data_text(&mut gb);
        assert!(
            data.starts_with("SECTION \"Raw Data\", WRAM0\n"),
            "a WRAM0 section first:\n{}",
            data
        );
        assert_links(&gb.build());

        // After the variables, the raw data goes in their last section, as documented
        let mut gb = RustBoy::new();
        gb.vars.create_u8("wScore", 0);
        gb.raw(|asm| {
            asm.chunk(Chunk::Data).raw("wMore: db");
        });
        let data = data_text(&mut gb);
        assert!(!data.contains("Raw Data"), "{}", data);
        assert!(data.contains("wScore: db\nwMore: db"), "{}", data);
        assert_links(&gb.build());
    }

    #[test]
    fn test_raw_data_reserves_space_only() {
        // The Data chunk is in WRAM0: labels and `ds n` are fine there
        let mut gb = RustBoy::new();
        gb.vars.create_u8("wScore", 0);
        gb.raw(|asm| {
            asm.chunk(Chunk::Data).label("wBuffer").ds("16");
        });
        assert_links(&gb.build());

        // Code or initialised data there is rejected when the program is built (rgbasm
        // rejected it: "cannot contain code or data")
        for write in [
            |asm: &mut Layout| {
                asm.ld_a(1);
            },
            |asm: &mut Layout| {
                asm.db("1, 2");
            },
            |asm: &mut Layout| {
                asm.ds_fill("4", "0");
            },
        ] {
            let mut gb = RustBoy::new();
            gb.raw(|asm| write(asm.chunk(Chunk::Data)));
            let message = crate::rust_boy::panic_message(|| gb.build());
            assert!(
                message.contains("in the WRAM0 section \"Raw Data\": a RAM section holds no code"),
                "{}",
                message
            );
        }
    }

    #[test]
    fn test_raw_init_and_main_loop_code_runs() {
        // B15: raw code was unreachable unless labelled and called; code written to the
        // Init and MainLoop chunks now runs, like `init()` and `add_to_main_loop` code
        let mut gb = RustBoy::new();
        gb.vars.create_u8("wRawInit", 0);
        gb.vars.create_u8("wRawLoop", 0);
        let user_init = gb.vars.create_u8("wUserInit", 0);
        let user_loop = gb.vars.create_u8("wUserLoop", 0);
        gb.init(user_init.set(1));
        gb.add_to_main_loop(user_loop.set(2));
        gb.raw(|asm| {
            asm.chunk(Chunk::Init).ld_a(7).ld_addr_def_a("wRawInit");
            asm.chunk(Chunk::MainLoop).ld_a(9).ld_addr_def_a("wRawLoop");
        });

        // At start-up: after the variables and the init() code, before the LCD is on
        let cpu = run_startup(&mut gb);
        let at = |name: &str, value: u8| {
            let event = Event::Write(name.to_string(), value);
            cpu.trace
                .iter()
                .rposition(|e| *e == event)
                .unwrap_or_else(|| panic!("{:?} not in {:?}", event, cpu.trace))
        };
        // The variables are set to 0 first, then the init() code, then the raw code
        assert!(at("wRawInit", 0) < at("wUserInit", 1), "{:?}", cpu.trace);
        assert!(at("wUserInit", 1) < at("wRawInit", 7), "{:?}", cpu.trace);
        assert!(at("wRawInit", 7) < at("rLCDC", 0x83), "{:?}", cpu.trace);
        assert_mem(&cpu, "wRawInit", 7);

        // In the main loop: after the add_to_main_loop code, before `jp Main`
        let main_loop = chunk_text(&mut gb, Chunk::MainLoop);
        let user = main_loop.find("ld [wUserLoop], a").expect("user code");
        let raw = main_loop.find("ld [wRawLoop], a").expect("raw code");
        let jp = main_loop.find("jp Main").expect("jp Main");
        assert!(user < raw && raw < jp, "{}", main_loop);
        assert_links(&gb.build());
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
        let mut body = Block::new();
        body.label(name);
        for callee in callees {
            body.call(callee);
        }
        body.ret();
        body.into_instrs()
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
                let mut code = Block::new();
                code.raw("ld a, 1 ; one\n    call Delay");
                gb.add_to_main_loop(code.into_instrs());
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
        let mut table = Block::new();
        table
            .ld(crate::gb_asm::R16::HL, "ByAddress")
            .jp_cond(crate::gb_asm::Condition::Z, "Jumped")
            // Raw text of several lines, the call after a comment
            .raw("ld a, 1 ; one\n    call FromRawText");
        gb.add_to_main_loop(table.into_instrs());
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
        assert_links(&asm.program().to_asm());
        let mut code = Block::new();
        code.call("CheckAndHandleBrick").ret();
        let mut code = code.into_instrs();
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
            let mut code = Block::new();
            code.ld_a_addr_def("Delay");
            code.into_instrs()
        };
        let mut gb = RustBoy::new();
        gb.define_const("Delay", 5);
        gb.add_to_main_loop(Block::new().ld(R8::A, "Delay").to_vec());
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
        let mut body = Block::new();
        body.label("Blank")
            .ld_a(0)
            .label("BlankWithA")
            .ld_addr_def_a("wTile")
            .ret();
        let mut gb = RustBoy::new();
        gb.vars.create_u8("wTile", 0);
        gb.define_function("Blank", body.into_instrs());
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
            let mut own = Block::new();
            own.label(name).ld_a(42).ret();
            let mut gb = RustBoy::new();
            gb.use_function(builtin);
            gb.define_function(name, own.into_instrs());
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
            let mut body = Block::new();
            body.label("MyLib").ret().label("UpdateKeys").ld_a(42).ret();
            body.into_instrs()
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
            let mut body = Block::new();
            body.label(name).ret().label("Entry").ld_a(value).ret();
            body.into_instrs()
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
            let mut body = Block::new();
            body.label(&name);
            for i in 0..48 {
                body.ld_a_addr_def(format!("wVar{}", i % 10)).inc(R8::A);
            }
            // Each function calls the next one, and has a second entry point
            if f < 99 {
                body.call(&format!("Func{}", f + 1));
            }
            body.label(&format!("Func{}Entry", f)).ret();
            gb.define_function(&name, body.into_instrs());
        }
        let mut main = Block::new();
        for i in 0..5000 {
            main.ld_a_addr_def(format!("wVar{}", i % 10));
        }
        main.call("Func0");
        gb.add_to_main_loop(main.into_instrs());
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
        // B27: Memcopy used to copy at least one byte, so an empty blob made it copy
        // 64 KiB over WRAM, the stack and the I/O registers; an empty blob gets no copy
        // code at all (and Memcopy now copies nothing for a length of 0)
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

    // ==================== One label allocator, relaxed jumps (Phase 2) ====================

    use crate::gb_asm::Condition;
    use crate::gb_asm::label_check::{assert_links_with, jr_range_errors};

    /// Every label the allocator handed out in `out` (a local `.{stem}_{n}`): each one is
    /// defined once in the whole program, whatever its scope. Returns them.
    fn generated_labels(out: &str) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for line in out.lines() {
            let Some(name) = line.trim().strip_suffix(':') else {
                continue;
            };
            let Some((_, number)) = name.strip_prefix('.').and_then(|n| n.rsplit_once('_')) else {
                continue;
            };
            if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            assert!(
                seen.insert(name.to_string()),
                "{} is defined twice in:\n{}",
                name,
                out
            );
        }
        seen.into_iter().collect()
    }

    /// Panics if a `jr` of the program `gb` builds does not reach its target, as RGBDS
    /// would (the relaxed program, as `build()` prints it)
    fn assert_jumps_in_range(gb: &mut RustBoy) {
        let program = gb.build_asm().program().program();
        assert_eq!(jr_range_errors(&program), Vec::<String>::new());
    }

    /// A program that makes up labels in every way: `If`, `IfConst`, `IfA` and `IfCall`
    /// (nested, with else, in `init`, the main loop, a function and `raw()` code), key
    /// checks, single and composite moves, `gb_std` snippets, animations on several
    /// sprites, and the start-up code
    fn labelled_program() -> RustBoy {
        use crate::gb_std::graphics::sprites::Sprite;
        use crate::gb_std::inputs::check_key;

        let mut gb = RustBoy::new();
        gb.set_sprite_size(SpriteSize::Size8x16);
        let player = gb.add_sprite_16x16("Player", tiles(4), tiles(4), 80, 72, 0);
        gb.sprites
            .add_composite_animation(player, "Walk", 0, 1, AnimationType::Loop);
        let coin = gb.add_sprite("Coin", tiles(4), 16, 16, 0);
        gb.sprites
            .add_animation(coin, "Spin", 0, 1, AnimationType::PingPong);
        gb.sprites
            .add_animation(coin, "Flash", 0, 1, AnimationType::Once);
        let score = gb.vars.create_u8("wScore", 0);

        gb.init(If::le(score.get(), score.get(), score.set(1)).or_else(score.set(2)));
        let inner = If::lt(
            gb.sprites.get_x(coin),
            gb.sprites.get_y(coin),
            gb.sprites.move_left_limit(coin, 1, 16),
        )
        .or_else(gb.sprites.move_composite_right_limit(player, 1, 140));
        gb.add_to_main_loop(If::gt(score.get(), score.get(), inner));
        gb.add_to_main_loop(IfConst::eq(
            score.get(),
            3,
            IfA::ne(4, gb.sprites.move_down_limit(coin, 1, 140)),
        ));

        let mut inputs = InputManager::new();
        inputs.on_press(
            PadButton::Left,
            gb.sprites.move_composite_left_limit(player, 1, 8),
        );
        inputs.on_press(PadButton::Left, gb.sprites.move_up_limit(coin, 1, 16));
        gb.add_inputs(inputs);

        // gb_std snippets, with the program's allocator
        let mut oam_4 = Sprite::new(4, 0, 0, 0, 0);
        let body = oam_4.move_right_limit(gb.labels(), 1, 140);
        gb.add_to_main_loop(check_key(gb.labels(), PadButton::Right, body));

        gb.define_function_from("Bump", IfA::eq(5, score.set(2)).or_else(score.set(3)));
        gb.add_to_main_loop(IfCall::is_true("Bump", score.set(4)).or_else(score.set(5)));

        // raw() code takes the same allocator, through its Layout
        let (get, set) = (score.get(), score.set(6));
        gb.raw(move |asm| {
            asm.chunk(Chunk::MainLoop)
                .emit_code(If::ne(get.clone(), get, set));
        });
        gb
    }

    #[test]
    fn test_every_generated_label_is_unique() {
        let mut gb = labelled_program();
        let out = gb.build();
        assert_labels_ok(&out);
        assert_links(&out);
        // Unique in the whole program, not only in their scope: one sequence for all
        let labels = generated_labels(&out);
        for kind in [
            ".end_if_",
            ".else_",
            ".then_",
            ".check_left_",
            ".check_left_end_",
            ".check_right_",
            "_limit_store_",
            "_limit_end_",
            ".clear_oam_",
            ".anim_end_",
            ".anim_Coin_end_",
            ".skip_Coin_Flash_",
            ".skip_Player_left_Walk_",
        ] {
            assert!(
                labels.iter().any(|label| label.contains(kind)),
                "no {} label in:\n{}",
                kind,
                out
            );
        }
        // The fixed global labels of the snippets are gone (B7, Phase 2)
        assert!(
            !out.contains("ClearOam") && !out.contains("AnimEnd"),
            "{}",
            out
        );
        assert_jumps_in_range(&mut gb);
    }

    #[test]
    fn test_a_generated_label_never_repeats_a_number() {
        // Labels taken by hand from the program's allocator (RustBoy::labels,
        // unique_label) and by the generators never share a number
        let mut gb = RustBoy::new();
        let mine = gb.labels().local("mine");
        let score = gb.vars.create_u8("wScore", 0);
        gb.add_to_main_loop(If::eq(score.get(), score.get(), score.set(1)));
        let unique = gb.unique_label("Loop");
        assert_eq!((mine.as_str(), unique.as_str()), (".mine_0", "Loop_2"));
        let mut body = Block::new();
        body.label(&mine).jr(&mine);
        gb.add_to_main_loop(body);
        let out = gb.build();
        assert!(out.contains(".end_if_1:"), "{}", out);
        assert!(
            out.contains(".clear_oam_3:"),
            "after every label so far: {}",
            out
        );
        assert_labels_ok(&out);
        generated_labels(&out);
    }

    /// The file `far_jumps_program` includes: `External`, a routine `build()` does not see
    const EXTERNAL_INC: (&str, &str) = ("external.inc", "External:\n    ret\n");

    /// A program whose own code has jumps out of reach of a `jr`, and jumps in reach
    fn far_jumps_program() -> RustBoy {
        let mut gb = RustBoy::new();
        let (paddle, ball) = paddle_and_ball(&mut gb);
        let score = gb.vars.create_u8("wScore", 0);

        // A jr over an If whose body is long (four moves, 22 bytes each, and more)
        let skip = gb.labels().local("skip");
        let mut test = Block::new();
        test.ld_a_addr_def("wScore")
            .and(R8::A)
            .jr_cond(Condition::Z, &skip);
        gb.add_to_main_loop(test);
        let mut moves = Vec::new();
        for _ in 0..2 {
            moves.extend(gb.sprites.move_left_limit(paddle, 1, 16));
            moves.extend(gb.sprites.move_right_limit(paddle, 1, 104));
            moves.extend(gb.sprites.move_up_limit(ball, 1, 16));
            moves.extend(gb.sprites.move_down_limit(ball, 1, 144));
        }
        gb.add_to_main_loop(If::lt(score.get(), score.get(), moves));
        let mut end = Block::new();
        end.label(&skip);
        gb.add_to_main_loop(end);

        // A jr in reach, in the start-up code
        let near = gb.labels().local("near");
        let mut init = Block::new();
        init.jr(&near).ld_a(1).label(&near);
        gb.init(init);

        // A routine whose loop jumps back over more than 128 bytes
        let mut wait = Block::new();
        wait.label("LongWait").label(".loop");
        for _ in 0..130 {
            wait.nop();
        }
        wait.dec(R8::B).jr_cond(Condition::NZ, ".loop").ret();
        gb.define_function("LongWait", wait.into_instrs());
        gb.add_to_main_loop(Call::new("LongWait"));

        // A jr to a routine the program does not define (an INCLUDEd file)
        gb.raw(|asm| {
            asm.chunk(Chunk::Functions)
                .label("Helper")
                .jr("External")
                .chunk(Chunk::Header)
                .include("external.inc");
        });
        gb.external_symbol("External");
        gb
    }

    #[test]
    fn test_out_of_range_jumps_become_jp() {
        let mut gb = far_jumps_program();
        // As written, two jumps are out of range (rgbasm would reject the program)
        let asm = gb.build_asm();
        let written: Vec<Instr> = [Chunk::MainLoop, Chunk::Functions]
            .iter()
            .flat_map(|chunk| asm.get_chunk(*chunk).cloned().unwrap_or_default())
            .collect();
        let errors = jr_range_errors(&written);
        assert!(
            errors
                .iter()
                .any(|e| e.starts_with("jr z, .skip_0: offset"))
                && errors
                    .iter()
                    .any(|e| e.starts_with("jr nz, .loop: offset -")),
            "{:?}",
            errors
        );
        let out = gb.build();
        // The jr over the If and the loop are out of range: jp; the near one stays jr
        assert!(out.contains("jp z, .skip_0\n"), "{}", out);
        assert!(out.contains("jp nz, .loop\n"), "{}", out);
        assert!(out.contains("jr .near_"), "{}", out);
        // A target the program does not define: jp, whatever the distance
        assert!(out.contains("jp External\n"), "{}", out);
        // The generated code's own jr stay jr (Loop/Once/PingPong functions, Memcopy...)
        assert_jumps_in_range(&mut gb);
        assert_links_with(&out, &[EXTERNAL_INC]);
    }

    #[test]
    fn test_generated_programs_have_no_jr_out_of_range() {
        // No program build() makes reports a jr out of range: these use every generator
        // (and the examples are assembled by CI)
        for mut gb in [
            sample_rustboy(),
            labelled_program(),
            far_jumps_program(),
            RustBoy::new(),
        ] {
            assert_jumps_in_range(&mut gb);
        }
    }

    #[test]
    fn test_the_same_program_gives_the_same_output() {
        // Labels and relaxation depend only on the program: built twice from scratch, or
        // twice in a row, it prints the same text
        for make in [labelled_program, far_jumps_program] {
            let mut first = make();
            let out = first.build();
            assert_eq!(out, make().build(), "two programs built the same way");
            assert_eq!(out, first.build(), "a second build()");
            // Code added after a build takes new numbers: the next build is still unique
            let score = first.vars.create_u8("wScore", 0);
            first.add_to_main_loop(If::eq(score.get(), score.get(), score.set(9)));
            let again = first.build();
            assert_ne!(again, out);
            generated_labels(&again);
            assert_links_with(&again, &[EXTERNAL_INC]);
        }
    }
}
