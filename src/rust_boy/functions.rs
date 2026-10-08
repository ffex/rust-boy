//! Function registry: the builtin routines and the user functions, and which of them
//! `build()` emits

use std::collections::BTreeSet;

use crate::gb_asm::labels::{code_of_line, split_label, symbol_words};
use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};
use crate::gb_std::graphics::utility::{get_tile_by_pixel, memcopy, wait_not_vblank, wait_vblank};
use crate::gb_std::inputs::update_keys;

/// Builtin functions that can be auto-included
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuiltinFunction {
    /// Memory copy routine
    Memcopy,
    /// Wait for VBlank
    WaitVBlank,
    /// Wait for not VBlank
    WaitNotVBlank,
    /// Update keyboard input state
    UpdateKeys,
    /// The background tile under a pixel: its address in `hl`, its index in `a`
    /// (see `gb_std::graphics::utility::get_tile_by_pixel` for the contract)
    GetTileByPixel,
    /// Delay loop using BC as counter
    Delay,
}

impl BuiltinFunction {
    /// Every builtin, in the order `build()` emits them
    const ALL: [BuiltinFunction; 6] = [
        BuiltinFunction::Memcopy,
        BuiltinFunction::WaitVBlank,
        BuiltinFunction::WaitNotVBlank,
        BuiltinFunction::UpdateKeys,
        BuiltinFunction::GetTileByPixel,
        BuiltinFunction::Delay,
    ];

    /// Get the label name for this function
    pub fn label(&self) -> &'static str {
        match self {
            BuiltinFunction::Memcopy => "Memcopy",
            BuiltinFunction::WaitVBlank => "WaitVBlank",
            BuiltinFunction::WaitNotVBlank => "WaitNotVBlank",
            BuiltinFunction::UpdateKeys => "UpdateKeys",
            BuiltinFunction::GetTileByPixel => "GetTileByPixel",
            BuiltinFunction::Delay => "Delay",
        }
    }

    /// Try to get a BuiltinFunction from its label name
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|func| func.label() == name)
    }

    /// Generate the assembly instructions for this function
    ///
    /// Every routine but `Delay` is the `gb_std` one: a routine exists once (B23).
    pub fn generate(&self) -> Vec<Instr> {
        match self {
            BuiltinFunction::Memcopy => memcopy(),
            BuiltinFunction::WaitVBlank => wait_vblank(),
            BuiltinFunction::WaitNotVBlank => wait_not_vblank(),
            BuiltinFunction::UpdateKeys => update_keys(),
            BuiltinFunction::GetTileByPixel => get_tile_by_pixel(),
            BuiltinFunction::Delay => generate_delay(),
        }
    }

    /// The `u8` variables (WRAM) the routine reads or writes, which `build()` creates
    /// when it emits the routine
    pub fn variables(&self) -> &'static [&'static str] {
        match self {
            BuiltinFunction::UpdateKeys => &["wCurKeys", "wNewKeys"],
            _ => &[],
        }
    }
}

/// What [`FunctionRegistry::generate_used`] found
pub struct UsedFunctions {
    /// The functions to emit, in their fixed order
    pub code: Vec<Instr>,
    /// The variables the emitted builtins need, in builtin order
    pub variables: Vec<&'static str>,
}

/// A function a name refers to
#[derive(Clone, Copy)]
enum Function {
    Builtin(BuiltinFunction),
    /// Index in [`FunctionRegistry::user_functions`]
    User(usize),
}

/// The builtin and user functions of a program, and the ones that must be emitted even
/// if no code calls them
#[derive(Default)]
pub struct FunctionRegistry {
    /// Builtins emitted even if no code calls them (`RustBoy::use_function`)
    forced_builtins: BTreeSet<BuiltinFunction>,
    /// User-defined functions as (name, instructions), in registration order
    user_functions: Vec<(String, Vec<Instr>)>,
    /// User functions emitted even if no code calls them (`RustBoy::keep_function`)
    kept_user_functions: BTreeSet<String>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit a builtin function even if no code calls it
    pub fn use_function(&mut self, func: BuiltinFunction) {
        self.forced_builtins.insert(func);
    }

    /// Emit the function `name` (a user function, or else a builtin) even if no code
    /// calls it. Returns false if there is no such function.
    pub fn keep_function(&mut self, name: &str) -> bool {
        match self.resolve(name) {
            Some(Function::User(_)) => {
                self.kept_user_functions.insert(name.to_string());
                true
            }
            Some(Function::Builtin(func)) => {
                self.forced_builtins.insert(func);
                true
            }
            None => false,
        }
    }

    /// Register a user-defined function
    ///
    /// Registering the same name again replaces the body and keeps its position.
    pub fn register_user_function(&mut self, name: &str, body: Vec<Instr>) {
        match self.user_functions.iter_mut().find(|(n, _)| n == name) {
            Some((_, existing)) => *existing = body,
            None => self.user_functions.push((name.to_string(), body)),
        }
    }

    /// The function `name` refers to: a user function, or else a builtin (a user
    /// function with the name of a builtin replaces it)
    fn resolve(&self, name: &str) -> Option<Function> {
        match self.user_functions.iter().position(|(n, _)| n == name) {
            Some(index) => Some(Function::User(index)),
            None => BuiltinFunction::from_name(name).map(Function::Builtin),
        }
    }

    /// Check if a function exists (builtin or user-defined)
    pub fn function_exists(&self, name: &str) -> bool {
        self.resolve(name).is_some()
    }

    /// Get list of all registered function names (for error messages)
    pub fn available_functions(&self) -> Vec<String> {
        let mut names: Vec<String> = BuiltinFunction::ALL
            .iter()
            .map(|func| func.label().to_string())
            .collect();
        names.extend(self.user_functions.iter().map(|(n, _)| n.clone()));
        names.sort();
        names.dedup();
        names
    }

    /// The functions a program needs, given its code outside the functions (`code`):
    /// every function that code refers to, then every function those refer to, and so
    /// on, plus the forced builtins and kept user functions (B24, B26)
    ///
    /// A function is found by its name anywhere in an instruction (`call`, `jp`, `jr`,
    /// `ld hl, Name`, `dw Name`, a raw line), so a call made through `Call`, `IfCall`, a
    /// user function body or raw code is seen. A name only counts once, so each function
    /// is emitted once; and none is emitted whose label `code` already defines (its own
    /// copy of a routine), nor a builtin whose label an emitted user function defines.
    ///
    /// The order is fixed: builtins in [`BuiltinFunction`] order, then user functions in
    /// registration order. The variables the emitted builtins need come with them.
    pub fn generate_used(&self, code: &[&[Instr]]) -> UsedFunctions {
        let mut defined = BTreeSet::new();
        let mut pending = Vec::new();
        for instrs in code {
            for instr in instrs.iter() {
                symbols(instr, &mut pending, &mut defined);
            }
        }
        pending.extend(self.kept_user_functions.iter().cloned());

        let mut builtins = BTreeSet::new();
        let mut users = BTreeSet::new();
        // Labels defined by the emitted user functions
        let mut user_labels = BTreeSet::new();
        let mut visit = |function: Function, pending: &mut Vec<String>| match function {
            Function::Builtin(func) => {
                if builtins.insert(func) {
                    for instr in func.generate() {
                        symbols(&instr, pending, &mut BTreeSet::new());
                    }
                }
            }
            Function::User(index) => {
                if users.insert(index) {
                    for instr in &self.user_functions[index].1 {
                        symbols(instr, pending, &mut user_labels);
                    }
                }
            }
        };
        for &func in &self.forced_builtins {
            if !defined.contains(func.label()) {
                visit(Function::Builtin(func), &mut pending);
            }
        }
        while let Some(name) = pending.pop() {
            if defined.contains(&name) {
                continue;
            }
            if let Some(function) = self.resolve(&name) {
                visit(function, &mut pending);
            }
        }

        let mut used = UsedFunctions {
            code: Vec::new(),
            variables: Vec::new(),
        };
        for func in builtins {
            if !user_labels.contains(func.label()) {
                used.code.extend(func.generate());
                used.variables.extend(func.variables());
            }
        }
        for index in users {
            used.code
                .extend(self.user_functions[index].1.iter().cloned());
        }
        used
    }
}

/// Add to `refs` the global symbols `instr` refers to, and to `defs` the global labels it
/// defines
///
/// The text of the instruction is read line by line (a raw instruction can hold several
/// lines), as RGBDS reads it: comments and the contents of strings are skipped, and a
/// line that starts with `Name:` defines `Name`. A local symbol (`.end_if_0`) is skipped,
/// and `Scope.local` refers to `Scope`. Comments, sections and file names refer to
/// nothing.
fn symbols(instr: &Instr, refs: &mut Vec<String>, defs: &mut BTreeSet<String>) {
    let text = match instr {
        Instr::Label { name } => {
            if !name.starts_with('.') {
                defs.insert(name.clone());
            }
            return;
        }
        Instr::Comment { .. }
        | Instr::Section { .. }
        | Instr::Include { .. }
        | Instr::Incbin { .. } => return,
        Instr::Def { value, .. } => value.clone(),
        other => other.to_string(),
    };
    for line in text.lines() {
        let code = code_of_line(line);
        let (label, rest) = split_label(&code);
        if let Some(label) = label {
            defs.insert(label.to_string());
        }
        refs.extend(symbol_words(rest).map(str::to_string));
    }
}

// Function implementations

fn generate_delay() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.comment("Delay loop using BC as counter");
    asm.comment("@param bc: delay counter (higher = longer delay)");
    asm.label("Delay");
    asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.or(Operand::Reg(Register::A), Operand::Reg(Register::C));
    asm.dec(Operand::Reg(Register::BC));
    asm.jr_cond(Condition::NZ, "Delay");
    asm.ret();

    asm.get_main_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_body(label: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.label(label).ret();
        asm.get_main_instrs()
    }

    /// The labels of `instrs`, without the `ret`s
    fn labels(instrs: &[Instr]) -> Vec<String> {
        instrs
            .iter()
            .map(|instr| instr.to_string())
            .filter(|line| line != "ret")
            .collect()
    }

    #[test]
    fn test_user_functions_keep_registration_order() {
        // Eight names in neither alphabetical nor any hash order (1 chance in 40320)
        let names = [
            "Golf", "Alpha", "Echo", "Hotel", "Bravo", "Foxtrot", "Charlie", "Delta",
        ];
        let mut registry = FunctionRegistry::new();
        for name in names {
            registry.register_user_function(name, function_body(name));
        }
        // Registering a name again replaces the body but keeps its position
        registry.register_user_function("Echo", function_body("EchoV2"));

        // All of them called, in another order
        let mut calls = Asm::new();
        for name in names.iter().rev() {
            calls.call(name);
        }
        assert_eq!(
            labels(&registry.generate_used(&[&calls.get_main_instrs()]).code),
            [
                "Golf:", "Alpha:", "EchoV2:", "Hotel:", "Bravo:", "Foxtrot:", "Charlie:", "Delta:",
            ]
        );
        assert!(registry.function_exists("Alpha"));
        assert!(!registry.function_exists("Missing"));
    }

    /// The text of `instrs`, one instruction per line
    fn text(instrs: &[Instr]) -> String {
        instrs.iter().map(|instr| format!("{}\n", instr)).collect()
    }

    #[test]
    fn test_builtins_are_the_gb_std_routines() {
        // B23: each routine exists once; rust_boy's GetTileByPixel had another contract
        // than gb_std's (it also loaded the tile into a)
        let gb_std = [
            (BuiltinFunction::Memcopy, memcopy()),
            (BuiltinFunction::WaitVBlank, wait_vblank()),
            (BuiltinFunction::WaitNotVBlank, wait_not_vblank()),
            (BuiltinFunction::UpdateKeys, update_keys()),
            (BuiltinFunction::GetTileByPixel, get_tile_by_pixel()),
        ];
        for (builtin, routine) in gb_std {
            assert_eq!(text(&builtin.generate()), text(&routine), "{:?}", builtin);
        }
        for builtin in BuiltinFunction::ALL {
            assert_eq!(BuiltinFunction::from_name(builtin.label()), Some(builtin));
            let label = format!("{}:", builtin.label());
            let body = text(&builtin.generate());
            assert_eq!(body.lines().filter(|line| *line == label).count(), 1);
        }
    }

    #[test]
    fn test_used_functions_are_found_through_other_functions() {
        // Main -> First -> (Second, Memcopy); Second -> Second; Unused -> Delay
        let mut registry = FunctionRegistry::new();
        for (name, calls) in [
            ("Unused", vec!["Delay"]),
            ("Second", vec!["Second"]),
            ("First", vec!["Second", "Memcopy"]),
        ] {
            let mut body = Asm::new();
            body.label(name);
            for callee in calls {
                body.call(callee);
            }
            body.ret();
            registry.register_user_function(name, body.get_main_instrs());
        }
        let mut main = Asm::new();
        main.label("Main").call("First").jp("Main");

        let used = registry.generate_used(&[&main.get_main_instrs()]);
        let out = text(&used.code);
        let defined: Vec<&str> = out.lines().filter(|l| l.ends_with(':')).collect();
        assert_eq!(defined, ["Memcopy:", "Second:", "First:"]);
        assert!(used.variables.is_empty());

        // UpdateKeys comes with its variables, however it is reached
        registry.register_user_function("Poll", {
            let mut body = Asm::new();
            body.label("Poll").call("UpdateKeys").ret();
            body.get_main_instrs()
        });
        let mut main = Asm::new();
        main.call("Poll");
        let used = registry.generate_used(&[&main.get_main_instrs()]);
        assert_eq!(used.variables, ["wCurKeys", "wNewKeys"]);
    }

    #[test]
    fn test_symbols_of_an_instruction() {
        let mut asm = Asm::new();
        asm.label("Start")
            .label(".loop")
            .call("Func")
            .jp_cond(Condition::NZ, ".loop")
            .jr("Other.local")
            .ld_hl_label("Table + 2")
            .ld_bc_label("TilesEnd - Tiles")
            .ld_a(5)
            .comment("call NotAReference")
            .raw("Raw: dw Target ; NotAReference either")
            .raw("ld [hl], BLANK_TILE")
            // Several lines in one raw instruction: each read on its own (a comment ends
            // at its line), a `;` in a string is not a comment, strings are not code
            .raw("ld a, 1 ; one\n    call Helper\nSecond: jp Third")
            .raw("db \"a;b\", LOW(Fourth), \"NotAReference\"");
        let mut refs = Vec::new();
        let mut defs = BTreeSet::new();
        for instr in asm.get_main_instrs() {
            symbols(&instr, &mut refs, &mut defs);
        }
        // Mnemonics and registers are words too (`call`, `hl`): they never name a function
        for name in [
            "Func",
            "Other",
            "Table",
            "TilesEnd",
            "Tiles",
            "Target",
            "BLANK_TILE",
            "Helper",
            "Third",
            "Fourth",
        ] {
            assert!(refs.iter().any(|r| r == name), "{} not in {:?}", name, refs);
        }
        for name in [
            "Start",
            "Raw",
            "Second",
            "loop",
            "local",
            "NotAReference",
            "5",
        ] {
            assert!(!refs.iter().any(|r| r == name), "{} in {:?}", name, refs);
        }
        assert_eq!(
            defs.into_iter().collect::<Vec<_>>(),
            ["Raw", "Second", "Start"]
        );
    }
}
