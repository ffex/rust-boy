//! Variable management with automatic WRAM allocation

use std::collections::BTreeMap;

use crate::gb_asm::{Asm, Instr, Operand, Register};

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
        let mut asm = Asm::new();
        store(&mut asm, &self.name, self.var_type, value);
        asm.get_main_instrs()
    }

    /// The code that loads the variable's value
    ///
    /// - 8-bit (`U8`, `I8`): into `a` (`ld a, [name]`).
    /// - 16-bit (`U16`, `I16`): into `hl` (`h` = the high byte, `l` = the low byte);
    ///   `a` is changed too (it holds the high byte).
    ///
    /// The `If` comparisons test `a`: they work on 8-bit variables only.
    pub fn get(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&self.name);
        if self.var_type.size() == 2 {
            asm.ld(Operand::Reg(Register::L), Operand::Reg(Register::A));
            asm.ld_a_addr_def(&format!("{}+1", self.name));
            asm.ld(Operand::Reg(Register::H), Operand::Reg(Register::A));
        }
        asm.get_main_instrs()
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
fn store(asm: &mut Asm, name: &str, var_type: VarType, value: i32) {
    match var_type {
        VarType::U8 | VarType::I8 => {
            if value < 0 {
                asm.ld_a_label(&format!("{}", value));
            } else {
                asm.ld_a(value as u8);
            }
            asm.ld_addr_def_a(name);
        }
        VarType::U16 | VarType::I16 => {
            let [low, high] = (value as u16).to_le_bytes();
            asm.ld_a(low);
            asm.ld_addr_def_a(name);
            asm.ld_a(high);
            asm.ld_addr_def_a(&format!("{}+1", name));
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
    pub wram_address: u16,
}

/// Manages variables with automatic WRAM allocation
///
/// A name is one WRAM label: creating a variable whose name already exists returns the
/// existing variable (its first initial value and section are kept). Creating it again
/// with a different type panics.
#[derive(Debug)]
pub struct VariableManager {
    /// Variables by id; ids are sequential, so iteration follows creation order
    variables: BTreeMap<VarId, Variable>,
    next_id: usize,
    next_wram_addr: u16,
    /// Sections in first-use order, each with its variables in creation order
    sections: Vec<(String, Vec<VarId>)>,
}

impl VariableManager {
    pub(crate) fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
            next_id: 0,
            next_wram_addr: 0xC000,
            sections: Vec::new(),
        }
    }

    /// The names (WRAM labels) of every variable, in creation order
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.variables.values().map(|var| var.name.as_str())
    }

    /// Create an unsigned 8-bit variable
    pub fn create_u8(&mut self, name: &str, initial: u8) -> Var {
        self.create_var(name, VarType::U8, initial as i32, "Variables")
    }

    /// Create an unsigned 16-bit variable
    pub fn create_u16(&mut self, name: &str, initial: u16) -> Var {
        self.create_var(name, VarType::U16, initial as i32, "Variables")
    }

    /// Create a signed 8-bit variable
    pub fn create_i8(&mut self, name: &str, initial: i8) -> Var {
        self.create_var(name, VarType::I8, initial as i32, "Variables")
    }

    /// Create a signed 16-bit variable
    pub fn create_i16(&mut self, name: &str, initial: i16) -> Var {
        self.create_var(name, VarType::I16, initial as i32, "Variables")
    }

    /// Create a variable in a specific section
    ///
    /// # Panics
    /// If `initial` is out of the range of `var_type` (see [`VarType::range`]).
    pub fn create_in_section(
        &mut self,
        name: &str,
        var_type: VarType,
        initial: i32,
        section: &str,
    ) -> Var {
        let (min, max) = var_type.range();
        if !(min..=max).contains(&initial) {
            panic!(
                "variable `{}`: the initial value {} is out of range for {:?} ({} to {})",
                name, initial, var_type, min, max
            );
        }
        self.create_var(name, var_type, initial, section)
    }

    fn create_var(&mut self, name: &str, var_type: VarType, initial: i32, section: &str) -> Var {
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

        let addr = self.next_wram_addr;
        self.next_wram_addr += var_type.size();

        let id = VarId(self.next_id);
        self.next_id += 1;

        let var = Variable {
            name: name.to_string(),
            var_type,
            initial_value: initial,
            wram_address: addr,
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

    /// Get the assembly label name for a variable
    pub fn get_label(&self, id: VarId) -> Option<&str> {
        self.variables.get(&id).map(|v| v.name.as_str())
    }

    /// Get the WRAM address for a variable
    pub fn get_address(&self, id: VarId) -> Option<u16> {
        self.variables.get(&id).map(|v| v.wram_address)
    }

    /// Get the variable type
    pub fn get_type(&self, id: VarId) -> Option<VarType> {
        self.variables.get(&id).map(|v| v.var_type)
    }

    /// Generate variable section instructions for the Data chunk
    pub(crate) fn generate_sections(&self) -> Vec<Instr> {
        use crate::gb_asm::Asm;

        let mut asm = Asm::new();

        for (section_name, var_ids) in &self.sections {
            asm.section(section_name, "WRAM0");

            for id in var_ids {
                if let Some(var) = self.variables.get(id) {
                    // Format: varName: db or varName: dw
                    asm.raw(&format!("{}: {}", var.name, var.var_type.directive()));
                }
            }
        }

        asm.get_main_instrs()
    }

    /// Generate initialization code for variables with non-zero initial values
    pub(crate) fn generate_init_code(&self) -> Vec<Instr> {
        use crate::gb_asm::Asm;

        let mut asm = Asm::new();

        // Every variable, even to 0, in creation order
        for var in self.variables.values() {
            store(&mut asm, &var.name, var.var_type, var.initial_value);
        }

        asm.get_main_instrs()
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

    /// The message `f` panics with; panics if it does not
    fn panic_message(f: impl FnOnce() -> Vec<Instr> + std::panic::UnwindSafe) -> String {
        let Err(err) = std::panic::catch_unwind(f) else {
            panic!("it did not panic");
        };
        err.downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
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
