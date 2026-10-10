//! Variable management with automatic WRAM allocation

use std::collections::BTreeMap;

use crate::gb_asm::expr::check_symbol;
use crate::gb_asm::{Block, Expr, Instr, R8, Section, is_identifier};

use super::error::{Definition, Error};
use super::memory::{MemoryAllocator, MemoryRegion};

/// Unique identifier for a variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarId(pub(crate) usize);

/// A handle to a variable that provides convenient operations.
///
/// This allows writing:
/// ```
/// use rust_boy::rust_boy::RustBoy;
///
/// let mut gb = RustBoy::new();
/// let ball_momentum_y = gb.vars.create_i8("wBallMomentumY", -1);
/// let score = gb.vars.create_u16("wScore", 0);
///
/// // Set a value: any value of the variable's type
/// gb.add_to_main_loop(ball_momentum_y.set(1));
/// gb.add_to_main_loop(score.set(1000));
///
/// // Get the value: into `a` for an 8-bit variable, into `hl` for a 16-bit one
/// gb.add_to_main_loop(ball_momentum_y.get());
/// gb.add_to_main_loop(score.get());
/// ```
#[derive(Debug, Clone)]
pub struct Var {
    id: VarId,
    name: String,
    var_type: VarType,
}

impl Var {
    /// The code that sets the variable to `value`, which must be a value of its type
    ///
    /// - 8-bit (`U8`, `I8`): `ld a, value` then `ld [name], a`.
    /// - 16-bit (`U16`, `I16`): both bytes, little-endian as `dw` stores them: the low
    ///   byte at `name`, the high byte at `name+1`.
    ///
    /// Changes `a`. Takes any integer type (`set(-1)`, `set(200u8)`, `set(1000)`).
    ///
    /// # Panics
    /// If `value` is out of the range of the variable's type (`U8` 0 to 255, `I8` -128 to
    /// 127, `U16` 0 to 65535, `I16` -32768 to 32767): the variable could not hold it.
    pub fn set(&self, value: impl Into<i32>) -> Vec<Instr> {
        let value = value.into();
        let (min, max) = self.var_type.range();
        if !(min..=max).contains(&value) {
            panic!(
                "{}.set({}): the value is out of range for a {:?} variable ({} to {})",
                self.name, value, self.var_type, min, max
            );
        }
        let mut asm = Block::new();
        store(&mut asm, &self.name, self.var_type, value);
        asm.into_instrs()
    }

    /// The code that loads the variable's value
    ///
    /// - 8-bit (`U8`, `I8`): into `a` (`ld a, [name]`).
    /// - 16-bit (`U16`, `I16`): into `hl` (`h` = the high byte, `l` = the low byte);
    ///   `a` is changed too (it holds the high byte).
    ///
    /// The `If` comparisons test `a`: they work on 8-bit variables only.
    pub fn get(&self) -> Vec<Instr> {
        let mut asm = Block::new();
        asm.ld_a_addr_def(&self.name);
        if self.var_type.size() == 2 {
            asm.ld(R8::L, R8::A);
            asm.ld_a_addr_def(Expr::sym(&self.name) + 1);
            asm.ld(R8::H, R8::A);
        }
        asm.into_instrs()
    }

    /// Get the variable name/label
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the variable id (for `VariableManager` lookups)
    pub fn id(&self) -> VarId {
        self.id
    }

    /// The variable's type
    pub fn var_type(&self) -> VarType {
        self.var_type
    }
}

/// Emit the code that writes `value` (in the range of `var_type`) to the variable `name`
///
/// One way for `Var::set` and the start-up initialisation. A negative 8-bit value is
/// written as such (`ld a, -1`, as before); a 16-bit value byte by byte, low byte first.
fn store(asm: &mut Block, name: &str, var_type: VarType, value: i32) {
    match var_type {
        VarType::U8 | VarType::I8 => {
            // A negative value is written as such: `ld a, -1`
            asm.ld(R8::A, value);
            asm.ld_addr_def_a(name);
        }
        VarType::U16 | VarType::I16 => {
            let [low, high] = (value as u16).to_le_bytes();
            asm.ld_a(low);
            asm.ld_addr_def_a(name);
            asm.ld_a(high);
            asm.ld_addr_def_a(Expr::sym(name) + 1);
        }
    }
}

/// Variable type and size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    /// 1 byte unsigned (db)
    U8,
    /// 2 bytes unsigned (dw)
    U16,
    /// 1 byte signed (db, interpreted as signed)
    I8,
    /// 2 bytes signed (dw, interpreted as signed)
    I16,
}

impl VarType {
    /// Get size in bytes
    pub fn size(&self) -> u16 {
        match self {
            VarType::U8 | VarType::I8 => 1,
            VarType::U16 | VarType::I16 => 2,
        }
    }

    /// Get the assembly directive for this type
    pub fn directive(&self) -> &'static str {
        match self {
            VarType::U8 | VarType::I8 => "db",
            VarType::U16 | VarType::I16 => "dw",
        }
    }

    /// The smallest and largest value a variable of this type holds
    pub fn range(&self) -> (i32, i32) {
        match self {
            VarType::U8 => (u8::MIN.into(), u8::MAX.into()),
            VarType::I8 => (i8::MIN.into(), i8::MAX.into()),
            VarType::U16 => (u16::MIN.into(), u16::MAX.into()),
            VarType::I16 => (i16::MIN.into(), i16::MAX.into()),
        }
    }
}

/// Internal variable data
#[derive(Debug, Clone)]
pub(crate) struct Variable {
    pub name: String,
    pub var_type: VarType,
    pub initial_value: i32,
}

/// Manages variables, laid out in WRAM0 by `build()`
///
/// A name is one WRAM label: creating a variable whose name already exists returns the
/// existing variable (its first initial value and section are kept). Creating it again
/// with a different type panics.
///
/// Every section is a `WRAM0` section, so all the variables share its 4 KiB
/// ($C000-$CFFF). They get their addresses when the program is built, section by section,
/// each variable after the one before it in its section, together with the variables
/// `build()` adds (the animations', the routines'): variables that do not fit make
/// `build()` return [`Error::MemoryFull`] (B17; creating one panicked before).
#[derive(Debug, Clone)]
pub struct VariableManager {
    /// Variables by id; ids are sequential, so iteration follows creation order
    variables: BTreeMap<VarId, Variable>,
    next_id: usize,
    /// Sections in first-use order, each with its variables in creation order
    sections: Vec<(String, Vec<VarId>)>,
}

impl VariableManager {
    pub(crate) fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
            next_id: 0,
            sections: Vec::new(),
        }
    }

    /// The names (WRAM labels) and types of every variable, in creation order
    pub(crate) fn names(&self) -> impl Iterator<Item = (&str, VarType)> {
        self.variables
            .values()
            .map(|var| (var.name.as_str(), var.var_type))
    }

    /// Create an unsigned 8-bit variable
    ///
    /// # Panics
    /// If `name` is not a valid RGBDS identifier, or is a register or keyword name (it is
    /// the variable's label), or a variable of another type has it.
    #[track_caller]
    pub fn create_u8(&mut self, name: &str, initial: u8) -> Var {
        self.create_var(name, VarType::U8, initial as i32, "Variables")
    }

    /// Create an unsigned 16-bit variable (panics as [`create_u8`](Self::create_u8))
    #[track_caller]
    pub fn create_u16(&mut self, name: &str, initial: u16) -> Var {
        self.create_var(name, VarType::U16, initial as i32, "Variables")
    }

    /// Create a signed 8-bit variable (panics as [`create_u8`](Self::create_u8))
    #[track_caller]
    pub fn create_i8(&mut self, name: &str, initial: i8) -> Var {
        self.create_var(name, VarType::I8, initial as i32, "Variables")
    }

    /// Create a signed 16-bit variable (panics as [`create_u8`](Self::create_u8))
    #[track_caller]
    pub fn create_i16(&mut self, name: &str, initial: i16) -> Var {
        self.create_var(name, VarType::I16, initial as i32, "Variables")
    }

    /// Create a variable in a specific section
    ///
    /// # Panics
    /// If `initial` is out of the range of `var_type` (see [`VarType::range`]), if
    /// `section` cannot be a section name (`"`, `\`, `{` or a line break: see
    /// [`Section::new`]), and as [`create_u8`](Self::create_u8).
    #[track_caller]
    pub fn create_in_section(
        &mut self,
        name: &str,
        var_type: VarType,
        initial: i32,
        section: &str,
    ) -> Var {
        // `build()` writes it as `SECTION "section", WRAM0`: checked here, not there
        Section::wram0(section);
        let (min, max) = var_type.range();
        if !(min..=max).contains(&initial) {
            panic!(
                "variable `{}`: the initial value {} is out of range for {:?} ({} to {})",
                name, initial, var_type, min, max
            );
        }
        self.create_var(name, var_type, initial, section)
    }

    /// Create the `u8` variable `name`, which `build()` needs for the code it generates,
    /// unless the program has it: `Err` if the program created it with another type
    pub(crate) fn create_needed(&mut self, name: &str, initial: u8) -> Result<(), Error> {
        match self.variables.values().find(|v| v.name == name) {
            Some(existing) if existing.var_type != VarType::U8 => Err(Error::NameConflict {
                name: name.to_string(),
                first: Definition::Variable(existing.var_type),
                second: Definition::GeneratedVariable(VarType::U8),
            }),
            _ => {
                self.create_u8(name, initial);
                Ok(())
            }
        }
    }

    #[track_caller]
    fn create_var(&mut self, name: &str, var_type: VarType, initial: i32, section: &str) -> Var {
        // A global label: `build()` writes it as a symbol (`ld [name], a`)
        if !is_identifier(name) || check_symbol(name).is_err() {
            panic!(
                "invalid variable name {:?}: it must be a valid RGBDS identifier (a letter or \
                 `_`, then letters, digits, `_`, `#`, `$` or `@`), not a register or keyword",
                name
            );
        }
        if let Some((&id, existing)) = self.variables.iter().find(|(_, v)| v.name == name) {
            assert!(
                existing.var_type == var_type,
                "variable `{}` already exists as {:?}, cannot create it again as {:?}",
                name,
                existing.var_type,
                var_type
            );
            return Var {
                id,
                name: name.to_string(),
                var_type,
            };
        }

        let id = VarId(self.next_id);
        self.next_id += 1;

        let var = Variable {
            name: name.to_string(),
            var_type,
            initial_value: initial,
        };

        self.variables.insert(id, var);
        match self.sections.iter_mut().find(|(s, _)| s == section) {
            Some((_, ids)) => ids.push(id),
            None => self.sections.push((section.to_string(), vec![id])),
        }

        Var {
            id,
            name: name.to_string(),
            var_type,
        }
    }

    /// The address of each variable in WRAM0, section by section, in their order (each
    /// variable after the one before it in its section), as far as they fit; and
    /// [`Error::MemoryFull`] for the first one that does not
    fn addresses(&self) -> (BTreeMap<VarId, u16>, Option<Error>) {
        let mut wram = MemoryAllocator::new(MemoryRegion::Wram0);
        let mut addresses = BTreeMap::new();
        for (_, ids) in &self.sections {
            for id in ids {
                let var = &self.variables[id];
                let size = var.var_type.size();
                let what = format!("variable `{}` ({} bytes)", var.name, size);
                match wram.try_allocate(size.into(), &what) {
                    Ok(address) => {
                        addresses.insert(*id, address);
                    }
                    Err(error) => return (addresses, Some(error)),
                }
            }
        }
        (addresses, None)
    }

    /// `Err` ([`Error::MemoryFull`]) if the variables do not fit in WRAM0
    pub(crate) fn check_layout(&self) -> Result<(), Error> {
        match self.addresses() {
            (_, Some(error)) => Err(error),
            (_, None) => Ok(()),
        }
    }

    /// Get the assembly label name for a variable
    pub fn get_label(&self, id: VarId) -> Option<&str> {
        self.variables.get(&id).map(|v| v.name.as_str())
    }

    /// The WRAM address of a variable in the program as it is now: sections in the order
    /// they were first used, each variable after the one before it in its section (with
    /// one section, in creation order from $C000)
    ///
    /// A variable created later in an earlier section moves the ones after it, and
    /// `build()` adds its own variables after the program's, so the address is final once
    /// every variable is created. `None` for an unknown id, or a variable that does not fit
    /// in WRAM0 (`build()` then returns [`Error::MemoryFull`]).
    pub fn get_address(&self, id: VarId) -> Option<u16> {
        self.addresses().0.get(&id).copied()
    }

    /// Get the variable type
    pub fn get_type(&self, id: VarId) -> Option<VarType> {
        self.variables.get(&id).map(|v| v.var_type)
    }

    /// Generate variable section instructions for the Data chunk
    pub(crate) fn generate_sections(&self) -> Vec<Instr> {
        use crate::gb_asm::Block;

        let mut asm = Block::new();

        for (section_name, var_ids) in &self.sections {
            asm.section(Section::wram0(section_name));

            for id in var_ids {
                if let Some(var) = self.variables.get(id) {
                    // Format: varName: db or varName: dw
                    asm.raw(&format!("{}: {}", var.name, var.var_type.directive()));
                }
            }
        }

        asm.into_instrs()
    }

    /// Generate initialization code for variables with non-zero initial values
    pub(crate) fn generate_init_code(&self) -> Vec<Instr> {
        use crate::gb_asm::Block;

        let mut asm = Block::new();

        // Every variable, even to 0, in creation order
        for var in self.variables.values() {
            store(&mut asm, &var.name, var.var_type, var.initial_value);
        }

        asm.into_instrs()
    }

    /// Check if any variables have been created
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8_variable() {
        let mut vm = VariableManager::new();

        let id = vm.create_u8("wScore", 0).id();

        assert_eq!(vm.get_label(id), Some("wScore"));
        assert_eq!(vm.get_address(id), Some(0xC000));
        assert_eq!(vm.get_type(id), Some(VarType::U8));
    }

    #[test]
    fn test_multiple_variables() {
        let mut vm = VariableManager::new();

        let id1 = vm.create_u8("wVar1", 0).id();
        let id2 = vm.create_u16("wVar2", 0).id();
        let id3 = vm.create_u8("wVar3", 0).id();

        assert_eq!(vm.get_address(id1), Some(0xC000));
        assert_eq!(vm.get_address(id2), Some(0xC001)); // After 1 byte
        assert_eq!(vm.get_address(id3), Some(0xC003)); // After 2 bytes
    }

    #[test]
    fn test_i8_variable() {
        let mut vm = VariableManager::new();

        let id = vm.create_i8("wMomentum", -1).id();

        assert_eq!(vm.get_label(id), Some("wMomentum"));
        assert_eq!(vm.get_type(id), Some(VarType::I8));
    }

    fn lines(instrs: Vec<Instr>) -> Vec<String> {
        instrs.iter().map(|instr| instr.to_string()).collect()
    }

    #[test]
    fn test_creating_a_variable_twice_returns_the_same_one() {
        let mut vm = VariableManager::new();

        let first = vm.create_u8("wKeys", 0).id();
        let second = vm.create_u8("wKeys", 5).id();

        assert_eq!(first, second);
        assert_eq!(
            lines(vm.generate_sections()),
            ["SECTION \"Variables\", WRAM0", "wKeys: db"]
        );
        // The first initial value is kept
        assert_eq!(lines(vm.generate_init_code()), ["ld a, 0", "ld [wKeys], a"]);
    }

    #[test]
    #[should_panic(expected = "already exists as U8")]
    fn test_creating_a_variable_with_another_type_panics() {
        let mut vm = VariableManager::new();
        vm.create_u8("wValue", 0);
        vm.create_u16("wValue", 0);
    }

    // ==================== Var::set / Var::get (B16) ====================

    use crate::gb_asm::test_cpu::TestCpu;
    use crate::rust_boy::panic_message;

    /// A test CPU whose memory holds every variable at its initial value, as the
    /// start-up code leaves it
    fn cpu_after_init(vm: &VariableManager) -> TestCpu {
        let mut cpu = TestCpu::default();
        cpu.run(&vm.generate_init_code());
        cpu.trace.clear();
        cpu
    }

    /// The 16-bit value of `name` in the CPU's memory, little-endian as `dw` stores it
    fn word(cpu: &TestCpu, name: &str) -> u16 {
        u16::from_le_bytes([cpu.mem[name], cpu.mem[&format!("{}+1", name)]])
    }

    #[test]
    fn test_set_writes_both_bytes_of_a_16_bit_variable() {
        // B16: set wrote only the low byte, so the high byte kept its old value
        let mut vm = VariableManager::new();
        let score = vm.create_u16("wScore", 0x1234);
        let speed = vm.create_i16("wSpeed", 0x0102);
        let mut cpu = cpu_after_init(&vm);
        assert_eq!(word(&cpu, "wScore"), 0x1234);

        cpu.run(&score.set(5));
        assert_eq!(word(&cpu, "wScore"), 5);
        cpu.run(&speed.set(-1));
        assert_eq!(
            word(&cpu, "wSpeed"),
            0xFFFF,
            "-1 as a 16-bit two's complement"
        );
        cpu.run(&speed.set(-100));
        assert_eq!(word(&cpu, "wSpeed") as i16, -100);
    }

    #[test]
    fn test_set_takes_any_value_of_the_variable_type() {
        let mut vm = VariableManager::new();
        let score = vm.create_u16("wScore", 0);
        let speed = vm.create_i16("wSpeed", 0);
        let lives = vm.create_u8("wLives", 3);
        let mut cpu = cpu_after_init(&vm);
        for value in [0, 1, 0xFF, 0x100, 0xBEEF, 0xFFFF] {
            cpu.run(&score.set(value));
            assert_eq!(word(&cpu, "wScore"), value);
        }
        for value in [i16::MIN, -300, -1, 0, 300, i16::MAX] {
            cpu.run(&speed.set(value));
            assert_eq!(word(&cpu, "wSpeed") as i16, value);
        }
        // A u8 above 127, without `200u8 as i8`
        cpu.run(&lives.set(200u8));
        assert_eq!(cpu.mem["wLives"], 200);
    }

    #[test]
    fn test_get_loads_a_16_bit_variable_into_hl() {
        // B16: get loaded one byte into a, for every type
        let mut vm = VariableManager::new();
        let score = vm.create_u16("wScore", 0xBEEF);
        let speed = vm.create_i16("wSpeed", -2);
        let mut cpu = cpu_after_init(&vm);
        cpu.run(&score.get());
        assert_eq!(u16::from_be_bytes([cpu.h, cpu.l]), 0xBEEF);
        cpu.run(&speed.get());
        assert_eq!(u16::from_be_bytes([cpu.h, cpu.l]) as i16, -2);
        assert!(cpu.trace.is_empty(), "get writes nothing: {:?}", cpu.trace);
    }

    #[test]
    fn test_set_and_get_an_8_bit_variable() {
        let mut vm = VariableManager::new();
        let lives = vm.create_u8("wLives", 3);
        let momentum = vm.create_i8("wMomentum", 1);
        let mut cpu = cpu_after_init(&vm);
        cpu.run(&lives.set(100));
        cpu.run(&momentum.set(-1));
        assert_eq!((cpu.mem["wLives"], cpu.mem["wMomentum"]), (100, 0xFF));
        assert_eq!(cpu.mem.get("wLives+1"), None, "one byte only");
        cpu.run(&lives.get());
        assert_eq!(cpu.a, 100);
        cpu.run(&momentum.get());
        assert_eq!(cpu.a as i8, -1);
        // The same code as before for 8-bit variables
        assert_eq!(lines(momentum.set(-1)), ["ld a, -1", "ld [wMomentum], a"]);
        assert_eq!(lines(lives.get()), ["ld a, [wLives]"]);
    }

    #[test]
    fn test_initial_values_of_every_type() {
        let mut vm = VariableManager::new();
        vm.create_i8("wByte", -5);
        vm.create_u16("wWord", 0xABCD);
        vm.create_i16("wSigned", -300);
        vm.create_in_section("wOther", VarType::U8, 255, "Other");
        let cpu = cpu_after_init(&vm);
        assert_eq!(cpu.mem["wByte"] as i8, -5);
        assert_eq!(word(&cpu, "wWord"), 0xABCD);
        assert_eq!(word(&cpu, "wSigned") as i16, -300);
        assert_eq!(cpu.mem["wOther"], 255);
    }

    #[test]
    fn test_variables_must_fit_in_wram0() {
        // B17: variables were counted past WRAM0 ($C000-$CFFF, 4 KiB), and every
        // section is a WRAM0 section: rgblink failed on a section too big, or the
        // counter wrapped. Then creating one too many panicked; they are laid out when
        // the program is built, which returns the error
        let mut vm = VariableManager::new();
        for i in 0..4094 {
            vm.create_u8(&format!("wByte{}", i), 0);
        }
        let last = vm.create_u16("wLast", 0);
        assert_eq!(vm.get_address(last.id()), Some(0xCFFE));
        assert_eq!(vm.check_layout(), Ok(()));
        let more = vm.create_u8("wMore", 0);
        assert_eq!(vm.get_address(more.id()), None, "it does not fit");
        let error = vm.check_layout().unwrap_err();
        assert_eq!(
            error,
            Error::MemoryFull {
                region: MemoryRegion::Wram0,
                what: "variable `wMore` (1 bytes)".to_string(),
                needed: 1,
                available: 0,
            }
        );
        assert!(
            error.to_string().starts_with(
                "no room for variable `wMore` (1 bytes): 1 bytes needed, but Wram0 \
                 ($C000-$CFFF, 4096 bytes) has 0 bytes left"
            ),
            "{}",
            error
        );
    }

    #[test]
    fn test_addresses_follow_the_sections() {
        // Each section's variables one after the other, sections in first-use order, as
        // the program lists them: a variable created later in the first section moves
        // the second section (the addresses were in creation order across sections)
        let mut vm = VariableManager::new();
        let a = vm.create_u8("wA", 0);
        let other = vm.create_in_section("wOther", VarType::U16, 0, "Other");
        let b = vm.create_u8("wB", 0);
        let addresses: Vec<Option<u16>> = [&a, &other, &b]
            .iter()
            .map(|var| vm.get_address(var.id()))
            .collect();
        assert_eq!(addresses, [Some(0xC000), Some(0xC002), Some(0xC001)]);
        assert_eq!(
            lines(vm.generate_sections()),
            [
                "SECTION \"Variables\", WRAM0",
                "wA: db",
                "wB: db",
                "SECTION \"Other\", WRAM0",
                "wOther: dw"
            ]
        );
        assert_eq!(vm.get_address(VarId(99)), None);
    }

    #[test]
    fn test_a_variable_name_is_a_symbol() {
        // It panicked only when its code was generated, in build() (or rgbasm failed)
        for name in [
            "w Score",
            "1st",
            "wScore.lo",
            ".local",
            "a",
            "hl",
            "ld",
            "SECTION",
        ] {
            let message = panic_message(|| VariableManager::new().create_u8(name, 0));
            assert!(
                message.contains(&format!("invalid variable name {:?}", name)),
                "{}: {}",
                name,
                message
            );
        }
        let mut vm = VariableManager::new();
        for name in ["wScore", "_tmp", "w#1", "LDA"] {
            vm.create_u8(name, 0);
        }
    }

    #[test]
    fn test_a_variable_build_needs_must_be_a_u8() {
        let mut vm = VariableManager::new();
        vm.create_u8("wCounter", 5);
        assert_eq!(vm.create_needed("wCounter", 0), Ok(()));
        assert_eq!(vm.create_needed("wNew", 0), Ok(()));
        assert_eq!(
            lines(vm.generate_init_code())[0],
            "ld a, 5",
            "the first value"
        );
        vm.create_u16("wWide", 0);
        assert_eq!(
            vm.create_needed("wWide", 0),
            Err(Error::NameConflict {
                name: "wWide".to_string(),
                first: Definition::Variable(VarType::U16),
                second: Definition::GeneratedVariable(VarType::U8),
            })
        );
    }

    #[test]
    #[should_panic(expected = "the initial value 300 is out of range for U8")]
    fn test_an_initial_value_must_fit_the_type() {
        // It was cut to a byte (300 became 44)
        VariableManager::new().create_in_section("wValue", VarType::U8, 300, "Variables");
    }

    #[test]
    fn test_set_panics_on_a_value_out_of_the_type_range() {
        // B16: set took an i8 for every type, so a u8 got a negative value without a word
        let mut vm = VariableManager::new();
        let lives = vm.create_u8("wLives", 0);
        let message = panic_message(|| lives.set(-1));
        assert!(
            message.contains("wLives") && message.contains("out of range"),
            "{}",
            message
        );
    }
}
