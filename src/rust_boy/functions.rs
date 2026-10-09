//! Function registry: the builtin routines and the user functions, and which of them
//! `build()` emits

use std::collections::{BTreeMap, BTreeSet};

use crate::gb_asm::labels::{code_lines, split_def, split_label, symbol_words};
use crate::gb_asm::{Asm, Condition, Instr, R8, R16};
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

/// A user function, with the global symbols its body refers to and the global labels it
/// defines, read once when it is registered
struct UserFunction {
    name: String,
    body: Vec<Instr>,
    refs: Vec<String>,
    labels: BTreeSet<String>,
}

impl UserFunction {
    fn new(name: &str, body: Vec<Instr>) -> Self {
        let (mut refs, mut labels) = (Vec::new(), BTreeSet::new());
        for instr in &body {
            symbols(instr, &mut refs, &mut labels);
        }
        Self {
            name: name.to_string(),
            body,
            refs,
            labels,
        }
    }
}

/// The builtin and user functions of a program, and the ones that must be emitted even
/// if no code calls them
#[derive(Default)]
pub struct FunctionRegistry {
    /// Builtins emitted even if no code calls them (`RustBoy::use_function`)
    forced_builtins: BTreeSet<BuiltinFunction>,
    /// User-defined functions, in registration order
    user_functions: Vec<UserFunction>,
    /// The index of each user function, by name
    by_name: BTreeMap<String, usize>,
    /// The indexes of the user functions whose body defines each global label (the
    /// first one registered wins): a second entry point finds its function
    by_label: BTreeMap<String, BTreeSet<usize>>,
    /// User functions emitted even if no code calls them (`RustBoy::keep_function`)
    kept_user_functions: BTreeSet<String>,
    /// Symbols defined outside the generated program (`RustBoy::external_symbol`): never
    /// emitted, nor the variables of a builtin of that name
    external_symbols: BTreeSet<String>,
    /// Functions `build()` generates and emits itself (the animation functions): known
    /// to `call`, never scanned
    generated: BTreeSet<String>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit a builtin function even if no code calls it (a user function of that name
    /// replaces it, and is emitted instead)
    pub fn use_function(&mut self, func: BuiltinFunction) {
        self.forced_builtins.insert(func);
    }

    /// Emit the function `name` (a user function, or else a builtin) even if no code
    /// calls it. Returns false if there is no such function. A function `build()`
    /// generates (an animation) is always emitted: once a first `build()` has registered
    /// it, keeping it does nothing. Before that its name is unknown, as for `call`.
    pub fn keep_function(&mut self, name: &str) -> bool {
        if self.generated.contains(name) {
            return true;
        }
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

    /// The symbol `name` is defined outside the generated program (an `INCLUDE`d file):
    /// a function of that name is never emitted
    pub fn external_symbol(&mut self, name: &str) {
        self.external_symbols.insert(name.to_string());
    }

    /// Register a user-defined function
    ///
    /// Registering the same name again replaces the body and keeps its position.
    pub fn register_user_function(&mut self, name: &str, body: Vec<Instr>) {
        let function = UserFunction::new(name, body);
        let index = match self.by_name.get(name) {
            Some(&index) => {
                // Forget the old body's labels, of this function only
                for label in &self.user_functions[index].labels {
                    if let Some(indexes) = self.by_label.get_mut(label) {
                        indexes.remove(&index);
                        if indexes.is_empty() {
                            self.by_label.remove(label);
                        }
                    }
                }
                self.user_functions[index] = function;
                index
            }
            None => {
                let index = self.user_functions.len();
                self.by_name.insert(name.to_string(), index);
                self.user_functions.push(function);
                index
            }
        };
        for label in &self.user_functions[index].labels {
            self.by_label
                .entry(label.clone())
                .or_default()
                .insert(index);
        }
    }

    /// The user function `name` refers to: by its name, or else by another global label
    /// of its body (a second entry point; the first function registered with it)
    fn user_function(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied().or_else(|| {
            self.by_label
                .get(name)
                .and_then(|indexes| indexes.first().copied())
        })
    }

    /// Register a function that `build()` generates and emits itself, so `call` knows it
    pub fn register_generated(&mut self, name: &str) {
        self.generated.insert(name.to_string());
    }

    /// The function `name` refers to: a user function (by its name, or by another global
    /// label of its body), or else a builtin. A user function that defines a builtin's
    /// name, as its name or as a second entry point, replaces the builtin.
    fn resolve(&self, name: &str) -> Option<Function> {
        match self.user_function(name) {
            Some(index) => Some(Function::User(index)),
            None => BuiltinFunction::from_name(name).map(Function::Builtin),
        }
    }

    /// Check if a function exists (builtin, user-defined, or generated by `build()`)
    pub fn function_exists(&self, name: &str) -> bool {
        self.resolve(name).is_some() || self.generated.contains(name)
    }

    /// Get list of all registered function names (for error messages)
    pub fn available_functions(&self) -> Vec<String> {
        let mut names: Vec<String> = BuiltinFunction::ALL
            .iter()
            .map(|func| func.label().to_string())
            .collect();
        names.extend(self.user_functions.iter().map(|f| f.name.clone()));
        names.extend(self.generated.iter().cloned());
        names.sort();
        names.dedup();
        names
    }

    /// The functions a program needs, given its code outside the functions (`code`) and
    /// its variables (`variables`): every function that code refers to, then every
    /// function those refer to, and so on, plus the forced builtins and kept user
    /// functions (B24, B26)
    ///
    /// A function is found by its name anywhere in an instruction (`call`, `jp`, `jr`,
    /// `ld hl, Name`, `dw Name`, a raw line), so a call made through `Call`, `IfCall`, a
    /// user function body or raw code is seen. A name only counts once, so each function
    /// is emitted once. A name the program defines elsewhere is not a builtin: a label
    /// or `DEF` of `code` (its own copy of a routine, a constant), a variable, or an
    /// external symbol; and a builtin whose label an emitted user function defines is not
    /// emitted either.
    ///
    /// The order is fixed: builtins in [`BuiltinFunction`] order, then user functions in
    /// registration order. The variables the emitted builtins need come with them, but
    /// not the ones the program already defines.
    ///
    /// # Panics
    /// If a user function's name is also a variable, a label or `DEF` of `code`, or an
    /// external symbol: a call to it would reach that other definition.
    pub fn generate_used<'a>(
        &self,
        code: &[&[Instr]],
        variables: impl IntoIterator<Item = &'a str>,
    ) -> UsedFunctions {
        let variables: BTreeSet<String> = variables.into_iter().map(str::to_string).collect();
        let mut code_defs = BTreeSet::new();
        let mut pending = Vec::new();
        for instrs in code {
            for instr in instrs.iter() {
                symbols(instr, &mut pending, &mut code_defs);
            }
        }
        for function in &self.user_functions {
            let other = if variables.contains(&function.name) {
                "a variable"
            } else if self.external_symbols.contains(&function.name) {
                "an external symbol (a function is defined either with define_function or \
                 outside the generated code, with external_symbol)"
            } else if code_defs.contains(&function.name) {
                "a constant or a label of the program (define_const, a DEF, or a label in \
                 raw code)"
            } else {
                continue;
            };
            panic!(
                "function `{0}` is also {1}: `call {0}` would not reach the function; \
                 rename one of them",
                function.name, other
            );
        }
        // Names defined outside the functions: variables, external symbols, the labels
        // and DEFs of the code
        let mut defined = variables;
        defined.extend(self.external_symbols.iter().cloned());
        defined.extend(code_defs);
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
                    let function = &self.user_functions[index];
                    pending.extend(function.refs.iter().cloned());
                    user_labels.extend(function.labels.iter().cloned());
                }
            }
        };
        for &func in &self.forced_builtins {
            if defined.contains(func.label()) {
                continue;
            }
            // A user function that defines the builtin's name replaces it, also when forced
            match self.user_function(func.label()) {
                Some(index) => visit(Function::User(index), &mut pending),
                None => visit(Function::Builtin(func), &mut pending),
            }
        }
        // Each name is handled once
        let mut seen = BTreeSet::new();
        while let Some(name) = pending.pop() {
            if defined.contains(&name) || seen.contains(&name) {
                continue;
            }
            if let Some(function) = self.resolve(&name) {
                visit(function, &mut pending);
            }
            seen.insert(name);
        }

        let mut used = UsedFunctions {
            code: Vec::new(),
            variables: Vec::new(),
        };
        // A builtin is only here when no user function defines its label (user names and
        // labels are looked up first)
        for func in builtins {
            used.code.extend(func.generate());
            used.variables.extend(
                func.variables()
                    .iter()
                    .filter(|name| !defined.contains(**name) && !user_labels.contains(**name)),
            );
        }
        for index in users {
            used.code
                .extend(self.user_functions[index].body.iter().cloned());
        }
        used
    }
}

/// Whether `code` defines the global symbol `name`: a label (`Name:`, also in a raw line)
/// or a `DEF`
pub(crate) fn defines(code: &[Instr], name: &str) -> bool {
    let mut defs = BTreeSet::new();
    for instr in code {
        symbols(instr, &mut Vec::new(), &mut defs);
    }
    defs.contains(name)
}

/// Add to `refs` the global symbols `instr` refers to, and to `defs` the global labels it
/// defines
///
/// The text of the instruction is read line by line (a raw instruction can hold several
/// lines), as RGBDS reads it ([`code_lines`]): comments (`;`, `/* … */`) and the contents
/// of strings are skipped, and a line that starts with `Name:` defines `Name`, as does
/// `DEF Name`. A local symbol (`.end_if_0`) is skipped, and `Scope.local` refers to
/// `Scope`. Sections and file names refer to nothing.
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
        Instr::Def { label, value } => {
            defs.insert(label.clone());
            value.clone()
        }
        other => other.to_string(),
    };
    for code in code_lines(&text) {
        let (label, rest) = split_label(&code);
        if let Some(label) = label {
            defs.insert(label.to_string());
        }
        // `DEF Name EQU value` (also in raw text): defines `Name`, refers to the value
        let rest = match split_def(rest) {
            Some((name, value)) => {
                defs.insert(name.to_string());
                value
            }
            None => rest,
        };
        refs.extend(symbol_words(rest).map(str::to_string));
    }
}

// Function implementations

fn generate_delay() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.comment("Delay loop using BC as counter");
    asm.comment("@param bc: delay counter (higher = longer delay)");
    asm.label("Delay");
    asm.ld(R8::A, R8::B);
    asm.or(R8::C);
    asm.dec(R16::BC);
    asm.jr_cond(Condition::NZ, "Delay");
    asm.ret();

    asm.get_main_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::Expr;

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
            labels(&registry.generate_used(&[&calls.get_main_instrs()], []).code),
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

        let used = registry.generate_used(&[&main.get_main_instrs()], []);
        let out = text(&used.code);
        // The global labels (Memcopy has a local `.copy:` too)
        let defined: Vec<&str> = out
            .lines()
            .filter(|l| l.ends_with(':') && !l.starts_with('.'))
            .collect();
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
        let used = registry.generate_used(&[&main.get_main_instrs()], []);
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
            .ld(R16::HL, Expr::sym("Table") + 2)
            .ld(R16::BC, Expr::sym("TilesEnd") - "Tiles")
            .ld_a(5)
            .comment("call NotAReference")
            .raw("Raw: dw Target ; NotAReference either")
            .raw("ld [hl], BLANK_TILE")
            // Several lines in one raw instruction: each read on its own (a comment ends
            // at its line), a `;` in a string is not a comment, strings are not code
            .raw("ld a, 1 ; one\n    call Helper\nSecond: jp Third")
            .raw("db \"a;b\", LOW(Fourth), \"NotAReference\"")
            // Block comments, also over several lines
            .raw("Blocked: /* call NotAReference */ ret /* and\n call NotAReference */")
            .def("CONSTANT", "Fifth + 1");
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
            "Fifth",
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
            ["Blocked", "CONSTANT", "Raw", "Second", "Start"]
        );
    }
}
