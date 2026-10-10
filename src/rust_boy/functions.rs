//! Function registry: the builtin routines and the user functions, and which of them
//! `build()` emits
//!
//! Every function is a [`Routine`]: the builtins are the `gb_std` routines, and a user
//! function is a routine registered with `RustBoy::define_routine`, or built from its body
//! by `define_function` / `define_function_from`. What a function needs is the routines it
//! depends on ([`Routine::deps`], given with the routine), and what its body refers to
//! (read as [`symbols`] reads code: typed operands by their type, raw text as RGBDS reads
//! it). A builtin's dependencies are given in full (a test checks them against its body),
//! so its body is not read.

use std::collections::{BTreeMap, BTreeSet};

use crate::gb_asm::Instr;
use crate::gb_asm::labels::symbols;
use crate::gb_std::graphics::utility::{get_tile_by_pixel, memcopy, wait_not_vblank, wait_vblank};
use crate::gb_std::inputs::update_keys;
use crate::gb_std::routine::Routine;
use crate::gb_std::utility::delay;

use super::error::{Definition, Error};
use super::variables::VarType;

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
    pub const ALL: [BuiltinFunction; 6] = [
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

    /// The routine: its code, the routines it needs, its variables and its calling
    /// convention
    ///
    /// Every builtin is the `gb_std` routine: a routine exists once (B23).
    pub fn routine(&self) -> Routine {
        match self {
            BuiltinFunction::Memcopy => memcopy(),
            BuiltinFunction::WaitVBlank => wait_vblank(),
            BuiltinFunction::WaitNotVBlank => wait_not_vblank(),
            BuiltinFunction::UpdateKeys => update_keys(),
            BuiltinFunction::GetTileByPixel => get_tile_by_pixel(),
            BuiltinFunction::Delay => delay(),
        }
    }

    /// The code of the routine ([`BuiltinFunction::routine`])
    pub fn generate(&self) -> Vec<Instr> {
        self.routine().into()
    }

    /// The `u8` variables (WRAM) the routine reads or writes, which `build()` creates
    /// when it emits the routine ([`Routine::variables`])
    pub fn variables(&self) -> Vec<String> {
        self.routine().variables().to_vec()
    }
}

/// What [`FunctionRegistry::generate_used`] found
pub struct UsedFunctions {
    /// The functions to emit, in their fixed order
    pub code: Vec<Instr>,
    /// The variables the emitted routines need, in the order of the routines, each once
    pub variables: Vec<String>,
}

/// A function a name refers to
#[derive(Clone, Copy)]
enum Function {
    Builtin(BuiltinFunction),
    /// Index in [`FunctionRegistry::user_functions`]
    User(usize),
}

/// A user function: its routine, the global symbols it refers to (its dependencies, then
/// what its body refers to) and the global labels its body defines, read once when it is
/// registered
struct UserFunction {
    routine: Routine,
    refs: Vec<String>,
    labels: BTreeSet<String>,
}

impl UserFunction {
    fn new(routine: Routine) -> Self {
        let mut refs: Vec<String> = routine
            .deps()
            .iter()
            .map(|dep| dep.name().to_string())
            .collect();
        let mut labels = BTreeSet::new();
        for instr in routine.body() {
            symbols(instr, &mut refs, &mut labels);
        }
        Self {
            routine,
            refs,
            labels,
        }
    }

    fn name(&self) -> &str {
        self.routine.name()
    }
}

/// The builtin and user functions of a program, and the ones that must be emitted even
/// if no code calls them (the builtins forced with `RustBoy::use_function` are in the
/// program's `RustBoyConfig`, given to [`generate_used`](Self::generate_used))
#[derive(Default)]
pub struct FunctionRegistry {
    /// User-defined functions, in registration order
    user_functions: Vec<UserFunction>,
    /// The index of each user function, by name
    by_name: BTreeMap<String, usize>,
    /// The indexes of the user functions whose body defines each global label (the
    /// first one registered wins): a second entry point finds its function
    by_label: BTreeMap<String, BTreeSet<usize>>,
    /// Functions emitted even if no code calls them (`RustBoy::keep_function`), by name,
    /// in the order they were kept: found when the program is built
    kept: Vec<String>,
    /// The names given to `RustBoy::call` / `call_args`, in order: each one must be a
    /// function when the program is built (it may be defined after the call)
    called: Vec<String>,
    /// Symbols defined outside the generated program (`RustBoy::external_symbol`): never
    /// emitted, nor the variables of a builtin of that name
    external_symbols: BTreeSet<String>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit the function `name` (a user function, or else a builtin) even if no code
    /// calls it. The name is looked up when the program is built: a function `build()`
    /// generates (an animation) is always emitted, and a name that is no function makes
    /// [`generate_used`](Self::generate_used) return [`Error::UnknownFunction`].
    pub fn keep_function(&mut self, name: &str) {
        if !self.kept.iter().any(|kept| kept == name) {
            self.kept.push(name.to_string());
        }
    }

    /// `name` is called (`RustBoy::call`, `call_args`): when the program is built it must
    /// be a function, or [`generate_used`](Self::generate_used) returns
    /// [`Error::UnknownFunction`]
    pub fn called(&mut self, name: &str) {
        if !self.called.iter().any(|called| called == name) {
            self.called.push(name.to_string());
        }
    }

    /// The symbol `name` is defined outside the generated program (an `INCLUDE`d file):
    /// a function of that name is never emitted
    pub fn external_symbol(&mut self, name: &str) {
        self.external_symbols.insert(name.to_string());
    }

    /// Register a user-defined function, from its body: what it needs is read from the body
    ///
    /// Registering the same name again replaces the function and keeps its position.
    pub fn register_user_function(&mut self, name: &str, body: Vec<Instr>) {
        self.register(Routine::new(name, body));
    }

    /// Register a routine and the routines it depends on (each one before the routines that
    /// need it), as user functions
    ///
    /// The routine replaces a user function of the same name (keeping its position), as
    /// [`register_user_function`](Self::register_user_function) does: that is how a program
    /// replaces a builtin. A dependency is shared, never replaced: a builtin's `gb_std`
    /// routine is the builtin (or the user function the program replaced it with), and a
    /// dependency equal to a user function (the whole [`Routine`]: code, dependencies,
    /// variables, convention) is that function.
    ///
    /// # Panics
    /// If a dependency has the name of a builtin, of a user function, or of another global
    /// label of one (a second entry point), but is another routine: a call to that name
    /// could reach only one of them. To replace a builtin, register the replacement itself
    /// (`define_function`, `define_routine`).
    pub fn register_routine(&mut self, routine: Routine) {
        let parent = format!("routine `{}`", routine.name());
        for dep in routine.deps() {
            self.register_dep(dep, &parent);
        }
        self.register(routine);
    }

    /// Make sure the program has the routine `routine`, which `needed_by` needs (a
    /// routine, `routine \`Name\``, or a typed call): register it, with its dependencies,
    /// unless it is a builtin or a user function already (see
    /// [`register_routine`](Self::register_routine))
    pub fn register_dep(&mut self, routine: &Routine, needed_by: &str) {
        let name = routine.name();
        let builtin = BuiltinFunction::from_name(name).map(|builtin| builtin.routine());
        // The builtin itself (or the user function the program replaced it with)
        let is_builtin = builtin.as_ref() == Some(routine);
        if let Some(index) = self.user_function(name) {
            let function = &self.user_functions[index];
            if is_builtin || (function.name() == name && function.routine == *routine) {
                return;
            }
            panic!(
                "{} needs a routine `{}`, but the program already has a function with that \
                 name and another routine (`{}`); rename one of them",
                needed_by,
                name,
                function.name()
            );
        }
        if is_builtin {
            return;
        }
        if builtin.is_some() {
            // Registered here, it would replace the builtin for the whole program, also
            // for the code that needs the builtin itself
            panic!(
                "{} needs a routine `{}` that is not the builtin `{}`; rename it, or replace \
                 the builtin for the whole program with define_function / define_routine",
                needed_by, name, name
            );
        }
        let parent = format!("routine `{}`", name);
        for dep in routine.deps() {
            self.register_dep(dep, &parent);
        }
        self.register(routine.clone());
    }

    /// Register `routine` as a user function, replacing one of the same name
    fn register(&mut self, routine: Routine) {
        let name = routine.name().to_string();
        let function = UserFunction::new(routine);
        let index = match self.by_name.get(&name) {
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
                self.by_name.insert(name, index);
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

    /// The function `name` refers to: a user function (by its name, or by another global
    /// label of its body), or else a builtin. A user function that defines a builtin's
    /// name, as its name or as a second entry point, replaces the builtin.
    fn resolve(&self, name: &str) -> Option<Function> {
        match self.user_function(name) {
            Some(index) => Some(Function::User(index)),
            None => BuiltinFunction::from_name(name).map(Function::Builtin),
        }
    }

    /// Check if a function exists (builtin or user-defined; the functions `build()`
    /// generates are the engine's to add)
    pub fn function_exists(&self, name: &str) -> bool {
        self.resolve(name).is_some()
    }

    /// Every function name, with the functions `build()` generates (`generated`), sorted
    /// (for error messages)
    pub fn available_functions(&self, generated: &[String]) -> Vec<String> {
        let mut names: Vec<String> = BuiltinFunction::ALL
            .iter()
            .map(|func| func.label().to_string())
            .collect();
        names.extend(self.user_functions.iter().map(|f| f.name().to_string()));
        names.extend(generated.iter().cloned());
        names.sort();
        names.dedup();
        names
    }

    /// The functions a program needs, given its code outside the functions (`code`), its
    /// variables (`variables`), the functions `build()` generates and emits itself
    /// (`generated`, the animations) and the builtins it emits even if no code calls them
    /// (`forced`; a user function of that name replaces one, and is emitted instead):
    /// every function that code refers to, then every function those need, and so on,
    /// plus the forced builtins and kept functions (B24, B26)
    ///
    /// A function is found by its name anywhere in an instruction (`call`, `jp`, `jr`,
    /// `ld hl, Name`, `dw Name`, a raw line; see [`symbols`]), so a call made through
    /// `Call`, `IfCall`, a user function body or raw code is seen. What a function needs
    /// is its dependencies ([`Routine::deps`]) and, for a user function, what its body
    /// refers to. A name only counts once, so each function is emitted once. A name the
    /// program defines elsewhere is not a builtin: a label or `DEF` of `code` (its own
    /// copy of a routine, a constant), a variable, or an external symbol; and a builtin
    /// whose label an emitted user function defines is not emitted either.
    ///
    /// The order is fixed: builtins in [`BuiltinFunction`] order, then user functions in
    /// registration order (a routine's dependencies are registered before it). The
    /// variables the emitted routines need come with them, but not the ones the program
    /// already defines.
    ///
    /// # Errors
    /// - [`Error::UnknownFunction`] if a name given to `call` / `call_args` /
    ///   `keep_function` is no function (nor a `generated` one);
    /// - [`Error::NameConflict`] if a user function's name is also a variable, a label or
    ///   `DEF` of `code`, or an external symbol: a call to it would reach that other
    ///   definition.
    pub fn generate_used<'a>(
        &self,
        code: &[&[Instr]],
        variables: impl IntoIterator<Item = (&'a str, VarType)>,
        generated: &[String],
        forced: &BTreeSet<BuiltinFunction>,
    ) -> Result<UsedFunctions, Error> {
        for name in self.called.iter().chain(&self.kept) {
            if !self.function_exists(name) && !generated.contains(name) {
                return Err(Error::UnknownFunction {
                    name: name.clone(),
                    available: self.available_functions(generated),
                });
            }
        }
        let variables: BTreeMap<String, VarType> = variables
            .into_iter()
            .map(|(name, var_type)| (name.to_string(), var_type))
            .collect();
        let mut code_defs = BTreeSet::new();
        let mut pending = Vec::new();
        for instrs in code {
            for instr in instrs.iter() {
                symbols(instr, &mut pending, &mut code_defs);
            }
        }
        for function in &self.user_functions {
            let name = function.name();
            let other = if generated.iter().any(|generated| generated == name) {
                Definition::GeneratedFunction
            } else if let Some(var_type) = variables.get(name) {
                Definition::Variable(*var_type)
            } else if self.external_symbols.contains(name) {
                Definition::ExternalSymbol
            } else if code_defs.contains(name) {
                Definition::CodeSymbol
            } else {
                continue;
            };
            return Err(Error::NameConflict {
                name: name.to_string(),
                first: Definition::Function,
                second: other,
            });
        }
        // Names defined outside the functions: variables, external symbols, the labels
        // and DEFs of the code
        let mut defined: BTreeSet<String> = variables.into_keys().collect();
        defined.extend(self.external_symbols.iter().cloned());
        defined.extend(code_defs);
        // A kept name is looked up as a name the code refers to: a user function, or else
        // a builtin (a function `build()` generates is emitted anyway)
        pending.extend(self.kept.iter().cloned());

        let mut builtins = BTreeSet::new();
        let mut users = BTreeSet::new();
        // Labels defined by the emitted user functions
        let mut user_labels = BTreeSet::new();
        let mut visit = |function: Function, pending: &mut Vec<String>| match function {
            Function::Builtin(func) => {
                if builtins.insert(func) {
                    // Its dependencies, given in full: the body is not read
                    pending.extend(
                        func.routine()
                            .deps()
                            .iter()
                            .map(|dep| dep.name().to_string()),
                    );
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
        for &func in forced {
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

        // A builtin is only here when no user function defines its label (user names and
        // labels are looked up first)
        let builtins: Vec<Routine> = builtins.into_iter().map(|func| func.routine()).collect();
        let routines = builtins.iter().chain(
            users
                .into_iter()
                .map(|index| &self.user_functions[index].routine),
        );
        let mut used = UsedFunctions {
            code: Vec::new(),
            variables: Vec::new(),
        };
        for routine in routines {
            for variable in routine.variables() {
                let needed = !defined.contains(variable)
                    && !user_labels.contains(variable)
                    && !used.variables.contains(variable);
                if needed {
                    used.variables.push(variable.clone());
                }
            }
            used.code.extend(routine.body().iter().cloned());
        }
        Ok(used)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::Block;
    use crate::gb_std::routine::Regs;

    fn function_body(label: &str) -> Vec<Instr> {
        let mut asm = Block::new();
        asm.label(label).ret();
        asm.into_instrs()
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
        // Registering a name again replaces the body but keeps its position (the new body
        // has a second label, to tell it from the old one)
        let mut echo = Block::new();
        echo.label("Echo").label("EchoV2").ret();
        registry.register_user_function("Echo", echo.into_instrs());

        // All of them called, in another order
        let mut calls = Block::new();
        for name in names.iter().rev() {
            calls.call(name);
        }
        assert_eq!(
            labels(
                &registry
                    .generate_used(&[&calls.into_instrs()], [], &[], &BTreeSet::new())
                    .unwrap()
                    .code
            ),
            [
                "Golf:", "Alpha:", "Echo:", "EchoV2:", "Hotel:", "Bravo:", "Foxtrot:", "Charlie:",
                "Delta:",
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
        // than gb_std's (it also loaded the tile into a). Each builtin is the gb_std
        // Routine value: code, dependencies, variables and calling convention
        let gb_std = [
            (BuiltinFunction::Memcopy, memcopy()),
            (BuiltinFunction::WaitVBlank, wait_vblank()),
            (BuiltinFunction::WaitNotVBlank, wait_not_vblank()),
            (BuiltinFunction::UpdateKeys, update_keys()),
            (BuiltinFunction::GetTileByPixel, get_tile_by_pixel()),
        ];
        for (builtin, routine) in gb_std {
            assert_eq!(builtin.routine(), routine, "{:?}", builtin);
            assert_eq!(
                text(&builtin.generate()),
                text(routine.body()),
                "{:?}",
                builtin
            );
        }
        // Delay is gb_std's too since routines are values: rust_boy has no routine of its own
        assert_eq!(BuiltinFunction::Delay.routine(), delay());
        for builtin in BuiltinFunction::ALL {
            assert_eq!(BuiltinFunction::from_name(builtin.label()), Some(builtin));
            assert_eq!(builtin.routine().name(), builtin.label());
            let label = format!("{}:", builtin.label());
            let body = text(&builtin.generate());
            assert_eq!(body.lines().filter(|line| *line == label).count(), 1);
            assert_eq!(builtin.variables(), builtin.routine().variables());
        }
    }

    #[test]
    fn test_builtin_dependencies_are_complete() {
        // build() does not read a builtin's body: what it needs is its dependencies and its
        // variables. So every global symbol its body refers to must be one of them, one of its
        // own labels, or a hardware.inc name; and a dependency must be a builtin, which
        // build() knows by name.
        let hardware: BTreeSet<&str> = crate::hw::SYMBOLS.iter().map(|(name, _)| *name).collect();
        for builtin in BuiltinFunction::ALL {
            let routine = builtin.routine();
            let (mut refs, mut defs) = (Vec::new(), BTreeSet::new());
            for instr in routine.body() {
                symbols(instr, &mut refs, &mut defs);
            }
            for name in refs {
                let known = defs.contains(&name)
                    || routine.deps().iter().any(|dep| dep.name() == name)
                    || routine.variables().contains(&name)
                    || hardware.contains(name.as_str());
                assert!(known, "{}: `{}` is not a dependency", builtin.label(), name);
            }
            for dep in routine.deps() {
                let dep_builtin = BuiltinFunction::from_name(dep.name());
                assert!(
                    dep_builtin.is_some_and(|b| b.routine() == *dep),
                    "{}: dependency {} is not a builtin",
                    builtin.label(),
                    dep.name()
                );
            }
        }
    }

    /// A routine `name` that calls each of `deps`, which it depends on
    fn routine_calling(name: &str, deps: Vec<Routine>) -> Routine {
        let mut body = Block::new();
        body.label(name);
        for dep in &deps {
            body.call(dep.name());
        }
        body.ret();
        deps.into_iter()
            .fold(Routine::new(name, body), Routine::with_dep)
            .with_clobbers(Regs::ALL)
    }

    /// The global labels of `code`, in order
    fn global_labels(code: &[Instr]) -> Vec<String> {
        text(code)
            .lines()
            .filter(|l| l.ends_with(':') && !l.starts_with('.'))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn test_a_routine_brings_its_dependencies() {
        // Top -> (Middle, Memcopy); Middle -> (Leaf, Memcopy, WaitVBlank); Leaf -> Delay.
        // Only Top is registered: its dependencies come with it, each once, in a fixed
        // order (builtins first, then each routine after the ones it needs)
        let leaf = routine_calling("Leaf", vec![delay()]);
        let middle = routine_calling("Middle", vec![leaf.clone(), memcopy(), wait_vblank()]);
        let top = routine_calling("Top", vec![middle.clone(), memcopy()]);
        let mut registry = FunctionRegistry::new();
        registry.register_routine(top.clone());
        assert!(registry.function_exists("Leaf"));

        let mut main = Block::new();
        main.call("Top").call("Top");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(
            global_labels(&used.code),
            [
                "Memcopy:",
                "WaitVBlank:",
                "Delay:",
                "Leaf:",
                "Middle:",
                "Top:"
            ]
        );

        // A dependency that is reached only through the dependency list (nothing in the body
        // names it, as for a jump table built elsewhere) is emitted too
        let mut body = Block::new();
        body.label("Dispatch").ret();
        let dispatch = Routine::new("Dispatch", body).with_dep(leaf);
        let mut registry = FunctionRegistry::new();
        registry.register_routine(dispatch);
        let mut main = Block::new();
        main.call("Dispatch");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(global_labels(&used.code), ["Delay:", "Leaf:", "Dispatch:"]);

        // And a dependency's variables come with it
        let poll = routine_calling("Poll", vec![crate::gb_std::inputs::update_keys()]);
        let mut registry = FunctionRegistry::new();
        registry.register_routine(poll);
        let mut main = Block::new();
        main.call("Poll");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(used.variables, ["wCurKeys", "wNewKeys"]);
    }

    #[test]
    fn test_a_routine_variable_is_created_once() {
        // A user routine that needs wCurKeys, as UpdateKeys does: one variable
        let mut body = Block::new();
        body.label("ReadKeys").ld_a_addr_def("wCurKeys").ret();
        let read_keys = Routine::new("ReadKeys", body)
            .with_variable("wCurKeys")
            .with_dep(crate::gb_std::inputs::update_keys());
        let mut registry = FunctionRegistry::new();
        registry.register_routine(read_keys);
        let mut main = Block::new();
        main.call("ReadKeys");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(used.variables, ["wCurKeys", "wNewKeys"]);
        // Not the ones the program defines
        let mut main = Block::new();
        main.call("ReadKeys");
        let used = registry
            .generate_used(
                &[&main.into_instrs()],
                [("wNewKeys", VarType::U8)],
                &[],
                &BTreeSet::new(),
            )
            .unwrap();
        assert_eq!(used.variables, ["wCurKeys"]);
    }

    #[test]
    fn test_a_dependency_is_shared_not_replaced() {
        // Two routines that need the same Helper: registered once, at its first place
        let helper = routine_calling("Helper", vec![]);
        let first = routine_calling("First", vec![helper.clone()]);
        let second = routine_calling("Second", vec![helper.clone()]);
        let mut registry = FunctionRegistry::new();
        registry.register_routine(first);
        registry.register_routine(second);
        let mut main = Block::new();
        main.call("Second").call("First");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(global_labels(&used.code), ["Helper:", "First:", "Second:"]);

        // A user function that replaces a builtin stays: a gb_std dependency on the builtin
        // reaches it
        let mut own = Block::new();
        own.label("Delay").ld_a(42).ret();
        let mut registry = FunctionRegistry::new();
        registry.register_user_function("Delay", own.into_instrs());
        registry.register_routine(routine_calling("Wait", vec![delay()]));
        let mut main = Block::new();
        main.call("Wait");
        let out = text(
            &registry
                .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
                .unwrap()
                .code,
        );
        assert!(out.contains("ld a, 42"), "{}", out);
        assert!(!out.contains("Delay loop"), "{}", out);
    }

    #[test]
    #[should_panic(
        expected = "routine `Second` needs a routine `Helper`, but the program \
                               already has a function with that name and another routine"
    )]
    fn test_two_routines_with_one_name_panic() {
        let mut registry = FunctionRegistry::new();
        registry.register_routine(routine_calling(
            "First",
            vec![routine_calling("Helper", vec![])],
        ));
        let other_helper = routine_calling("Helper", vec![delay()]);
        registry.register_routine(routine_calling("Second", vec![other_helper]));
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
            let mut body = Block::new();
            body.label(name);
            for callee in calls {
                body.call(callee);
            }
            body.ret();
            registry.register_user_function(name, body.into_instrs());
        }
        let mut main = Block::new();
        main.label("Main").call("First").jp("Main");

        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
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
            let mut body = Block::new();
            body.label("Poll").call("UpdateKeys").ret();
            body.into_instrs()
        });
        let mut main = Block::new();
        main.call("Poll");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(used.variables, ["wCurKeys", "wNewKeys"]);
    }

    /// A routine named like a builtin, with other code
    fn custom_memcopy() -> Routine {
        let mut body = Block::new();
        body.label("Memcopy").ld_a(119).ret();
        Routine::new("Memcopy", body).with_clobbers(Regs::A)
    }

    #[test]
    fn test_a_dependency_cannot_replace_a_builtin() {
        // Review of #27: a dependency named like a builtin, with other code, was registered
        // as a user function and replaced the builtin for the whole program, also for a
        // routine that needs the real Memcopy. Now it panics, in either order.
        let custom = routine_calling("UsesCustom", vec![custom_memcopy()]);
        let real = routine_calling("UsesReal", vec![memcopy()]);
        for order in [[custom.clone(), real.clone()], [real, custom]] {
            let message = crate::rust_boy::panic_message(|| {
                let mut registry = FunctionRegistry::new();
                for routine in order.clone() {
                    registry.register_routine(routine);
                }
            });
            assert!(
                message.contains(
                    "routine `UsesCustom` needs a routine `Memcopy` that is not the builtin \
                     `Memcopy`"
                ),
                "{}",
                message
            );
        }
        // So does a typed call to it (RustBoy::call_routine)
        let message = crate::rust_boy::panic_message(|| {
            FunctionRegistry::new().register_dep(&custom_memcopy(), "call_routine(\"Memcopy\")")
        });
        assert!(message.contains("that is not the builtin"), "{}", message);

        // Replaced explicitly, for the whole program: a dependency on the gb_std Memcopy
        // reaches the replacement, and one on the replacement itself is shared
        let mut registry = FunctionRegistry::new();
        registry.register_routine(custom_memcopy());
        registry.register_routine(routine_calling("UsesReal", vec![memcopy()]));
        registry.register_routine(routine_calling("UsesCustom", vec![custom_memcopy()]));
        let mut main = Block::new();
        main.call("UsesReal").call("UsesCustom");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        let out = text(&used.code);
        assert_eq!(out.matches("Memcopy:").count(), 1, "{}", out);
        assert!(out.contains("ld a, 119"), "{}", out);
    }

    #[test]
    fn test_a_dependency_is_shared_only_when_it_is_the_same_routine() {
        // Review of #27: only the bodies were compared, so a dependency with the same code but
        // other dependencies, variables or convention was "shared" and lost them. Now the
        // whole Routine is compared, as Routine::with_dep and with_deps do.
        let helper = routine_calling("Helper", vec![]);
        let others = [
            helper.clone().with_dep(delay()),
            helper.clone().with_variable("wHelper"),
            helper.clone().with_clobbers(Regs::A),
        ];
        for other in others {
            let message = crate::rust_boy::panic_message(|| {
                let mut registry = FunctionRegistry::new();
                registry.register_routine(routine_calling("First", vec![helper.clone()]));
                registry.register_routine(routine_calling("Second", vec![other.clone()]));
            });
            assert!(
                message.contains("routine `Second` needs a routine `Helper`"),
                "{:?}: {}",
                other,
                message
            );
        }
        // The same routine is shared: registered once, with its dependencies and variables
        let helper = routine_calling("Helper", vec![delay()]).with_variable("wHelper");
        let mut registry = FunctionRegistry::new();
        registry.register_routine(routine_calling("First", vec![helper.clone()]));
        registry.register_routine(routine_calling("Second", vec![helper]));
        let mut main = Block::new();
        main.call("Second");
        let used = registry
            .generate_used(&[&main.into_instrs()], [], &[], &BTreeSet::new())
            .unwrap();
        assert_eq!(global_labels(&used.code), ["Delay:", "Helper:", "Second:"]);
        assert_eq!(used.variables, ["wHelper"]);
    }
}
