//! Routines as values: a labelled piece of code, the routines it needs, and its calling
//! convention.
//!
//! A [`Routine`] is what a program calls: `Memcopy`, `WaitVBlank`, `UpdateKeys`, … (the
//! `gb_std` routines, which `RustBoy` emits as its builtins), or a routine of your own. It
//! holds:
//! - its **name**, the global label its body defines and a `call` reaches;
//! - its **body**, the instructions, starting with that label;
//! - its **dependencies**, the routines it calls (or jumps to, or reads): a program that
//!   emits the routine must emit them too. They are values, so a routine brings what it
//!   needs with it: [`Routine::with_deps`] lists it and every dependency, each once, and
//!   `RustBoy` emits the dependencies of every routine it emits;
//! - the **variables** it needs (bytes of WRAM, such as `wCurKeys`), which the program
//!   must define (`RustBoy` creates them with the routine);
//! - its **calling convention**, as [`Regs`]: the registers it **reads** (its inputs),
//!   the ones it **returns** (its outputs) and the ones it **clobbers** (changes, with no
//!   meaning for the caller). Every other register is preserved: it holds the same value
//!   after the call. The tests run each `gb_std` routine on a model of the CPU and check
//!   that the registers it does not list keep their values.
//!
//! The same clobber model describes control flow: [`If`](super::flow::If) and the other
//! `If` kinds say which registers they use with a `clobbers()` that returns [`Regs`].
//!
//! # Example
//! ```
//! use rust_boy::gb_asm::{Block, R16};
//! use rust_boy::gb_std::graphics::utility::memcopy;
//! use rust_boy::gb_std::routine::{Regs, Routine};
//! use rust_boy::hw;
//!
//! // A routine that copies the level map to the background: it calls Memcopy
//! let mut body = Block::new();
//! body.label("LoadLevel")
//!     .ld(R16::DE, "LevelMap")
//!     .ld(R16::HL, hw::SCRN0)
//!     .ld(R16::BC, 1024)
//!     .call("Memcopy")
//!     .ret();
//! let load_level = Routine::new("LoadLevel", body)
//!     .with_dep(memcopy())
//!     .with_clobbers(Regs::A | Regs::BC | Regs::DE | Regs::HL | Regs::F);
//!
//! // The routine and what it needs, each once: its dependencies first
//! let names: Vec<&str> = load_level.with_deps().iter().map(|r| r.name()).collect();
//! assert_eq!(names, ["Memcopy", "LoadLevel"]);
//! assert_eq!(load_level.preserves(), Regs::NONE);
//!
//! // Memcopy's convention: bc, de and hl in, the same out, a and the flags clobbered
//! let memcopy = memcopy();
//! assert_eq!(memcopy.reads().to_string(), "bc, de, hl");
//! assert_eq!(memcopy.clobbers().to_string(), "a, f");
//! assert_eq!(memcopy.preserves(), Regs::NONE);
//! ```

use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign, Sub};

use crate::gb_asm::labels::defines;
use crate::gb_asm::{Instr, R8, R16, is_identifier};

use super::flow::Call;

/// A set of CPU registers: the 8-bit registers `a`, `b`, `c`, `d`, `e`, `h`, `l`, and the
/// flags `f` (Z, N, H, C, as one)
///
/// It describes a calling convention ([`Routine`]) or the registers a piece of code uses
/// ([`If::clobbers`](super::flow::If::clobbers)). Combine sets with `|`, take one from
/// another with `-`. `sp` is not in it: every routine returns with the stack as it found it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Regs(u8);

impl Regs {
    /// No register
    pub const NONE: Regs = Regs(0);
    pub const A: Regs = Regs(1 << 0);
    pub const B: Regs = Regs(1 << 1);
    pub const C: Regs = Regs(1 << 2);
    pub const D: Regs = Regs(1 << 3);
    pub const E: Regs = Regs(1 << 4);
    pub const H: Regs = Regs(1 << 5);
    pub const L: Regs = Regs(1 << 6);
    /// The flags: Z, N, H and C
    pub const F: Regs = Regs(1 << 7);
    pub const BC: Regs = Regs(Self::B.0 | Self::C.0);
    pub const DE: Regs = Regs(Self::D.0 | Self::E.0);
    pub const HL: Regs = Regs(Self::H.0 | Self::L.0);
    pub const AF: Regs = Regs(Self::A.0 | Self::F.0);
    /// Every register and the flags
    pub const ALL: Regs = Regs(u8::MAX);

    /// The 8-bit registers, in the order they are printed
    const NAMES: [(Regs, &'static str); 8] = [
        (Regs::A, "a"),
        (Regs::B, "b"),
        (Regs::C, "c"),
        (Regs::D, "d"),
        (Regs::E, "e"),
        (Regs::H, "h"),
        (Regs::L, "l"),
        (Regs::F, "f"),
    ];

    /// The registers of `self` and of `other`
    pub const fn union(self, other: Regs) -> Regs {
        Regs(self.0 | other.0)
    }

    /// Whether every register of `other` is in `self`
    pub const fn contains(self, other: Regs) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether `self` and `other` have a register in common
    pub const fn intersects(self, other: Regs) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether there is no register in it
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The 8-bit register `reg`
    ///
    /// # Panics
    /// On `R8::AtHl`: `[hl]` is memory, not a register.
    pub fn r8(reg: R8) -> Regs {
        match reg {
            R8::A => Regs::A,
            R8::B => Regs::B,
            R8::C => Regs::C,
            R8::D => Regs::D,
            R8::E => Regs::E,
            R8::H => Regs::H,
            R8::L => Regs::L,
            R8::AtHl => panic!("[hl] is memory, not a register"),
        }
    }

    /// The two registers of the pair `pair`
    ///
    /// # Panics
    /// On `R16::SP`: the stack pointer is not part of a calling convention.
    pub fn r16(pair: R16) -> Regs {
        match pair {
            R16::BC => Regs::BC,
            R16::DE => Regs::DE,
            R16::HL => Regs::HL,
            R16::SP => panic!("sp is not part of a calling convention"),
        }
    }

    /// Each 8-bit register of the set, and the flags last
    pub fn iter(self) -> impl Iterator<Item = Regs> {
        Self::NAMES
            .into_iter()
            .map(|(reg, _)| reg)
            .filter(move |reg| self.contains(*reg))
    }
}

impl BitOr for Regs {
    type Output = Regs;

    fn bitor(self, other: Regs) -> Regs {
        self.union(other)
    }
}

impl BitOrAssign for Regs {
    fn bitor_assign(&mut self, other: Regs) {
        *self = self.union(other);
    }
}

impl BitAnd for Regs {
    type Output = Regs;

    fn bitand(self, other: Regs) -> Regs {
        Regs(self.0 & other.0)
    }
}

/// The registers of `self` that are not in `other`
impl Sub for Regs {
    type Output = Regs;

    fn sub(self, other: Regs) -> Regs {
        Regs(self.0 & !other.0)
    }
}

/// The register names, a pair by its name when both halves are in: `a, bc, f`; `none`
/// for no register
impl fmt::Display for Regs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "none");
        }
        let mut names = Vec::new();
        let mut left = *self;
        for (reg, name) in Self::NAMES {
            if !left.contains(reg) {
                continue;
            }
            let pair = [(Regs::BC, "bc"), (Regs::DE, "de"), (Regs::HL, "hl")]
                .into_iter()
                .find(|(pair, _)| pair.contains(reg) && left.contains(*pair));
            match pair {
                Some((pair, pair_name)) => {
                    names.push(pair_name);
                    left = left - pair;
                }
                None => {
                    names.push(name);
                    left = left - reg;
                }
            }
        }
        write!(f, "{}", names.join(", "))
    }
}

impl fmt::Debug for Regs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Regs({})", self)
    }
}

/// A routine: a labelled piece of code, the routines it needs, the variables it uses and
/// its calling convention (see the [module](self) documentation)
///
/// Build one with [`Routine::new`] and the `with_*` methods. A routine is a list of
/// instructions too: `asm.emit_all(routine)` writes its body, and
/// `Vec::<Instr>::from(routine)` gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Routine {
    name: String,
    body: Vec<Instr>,
    deps: Vec<Routine>,
    reads: Regs,
    returns: Regs,
    clobbers: Regs,
    variables: Vec<String>,
}

impl Routine {
    /// The routine `name`, whose code is `body`
    ///
    /// It reads and returns nothing and may change every register ([`Regs::ALL`]) until
    /// [`with_reads`](Self::with_reads), [`with_returns`](Self::with_returns) and
    /// [`with_clobbers`](Self::with_clobbers) say otherwise; it has no dependency and no
    /// variable.
    ///
    /// # Panics
    /// If `name` is not an RGBDS identifier, or `body` does not define the global label
    /// `name` (a `call` to the routine reaches that label).
    #[track_caller]
    pub fn new(name: &str, body: impl Into<Vec<Instr>>) -> Routine {
        if !is_identifier(name) {
            panic!(
                "invalid routine name \"{}\": it must be a valid RGBDS identifier (a letter or \
                 `_`, then letters, digits, `_`, `#`, `$` or `@`)",
                name
            );
        }
        let body = body.into();
        if !defines(&body, name) {
            panic!(
                "routine \"{0}\": its body does not define the label `{0}:`, so a call to `{0}` \
                 could not reach it (start the body with `{0}:`)",
                name
            );
        }
        Routine {
            name: name.to_string(),
            body,
            deps: Vec::new(),
            reads: Regs::NONE,
            returns: Regs::NONE,
            clobbers: Regs::ALL,
            variables: Vec::new(),
        }
    }

    /// The registers the routine reads: its inputs
    pub fn with_reads(mut self, regs: Regs) -> Routine {
        self.reads = regs;
        self
    }

    /// The registers the routine returns a result in
    pub fn with_returns(mut self, regs: Regs) -> Routine {
        self.returns = regs;
        self
    }

    /// The registers the routine changes without a meaning for the caller; every register
    /// it neither returns nor clobbers is preserved
    pub fn with_clobbers(mut self, regs: Regs) -> Routine {
        self.clobbers = regs;
        self
    }

    /// A routine this one needs (it calls it, jumps to it or reads it): a program that
    /// emits this routine emits `dep` too, with its own dependencies
    ///
    /// # Panics
    /// If `dep` is this routine, or another dependency has its name but other code.
    #[track_caller]
    pub fn with_dep(mut self, dep: Routine) -> Routine {
        if dep.name == self.name {
            panic!("routine \"{}\" cannot depend on itself", self.name);
        }
        match self.deps.iter().find(|other| other.name == dep.name) {
            Some(other) if *other != dep => panic!(
                "routine \"{}\": two different dependencies are named \"{}\"",
                self.name, dep.name
            ),
            Some(_) => {}
            None => self.deps.push(dep),
        }
        self
    }

    /// A byte of WRAM the routine reads or writes, which the program must define:
    /// `RustBoy` creates it (a `u8`, 0 at start-up) with the routine, unless the program
    /// defines it
    ///
    /// # Panics
    /// If `name` is not an RGBDS identifier.
    #[track_caller]
    pub fn with_variable(mut self, name: &str) -> Routine {
        if !is_identifier(name) {
            panic!(
                "routine \"{}\": invalid variable name \"{}\"",
                self.name, name
            );
        }
        if !self.variables.iter().any(|var| var == name) {
            self.variables.push(name.to_string());
        }
        self
    }

    /// Its name: the global label a `call` reaches
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its code, starting with its label
    pub fn body(&self) -> &[Instr] {
        &self.body
    }

    /// The routines it needs directly (not theirs: see [`with_deps`](Self::with_deps))
    pub fn deps(&self) -> &[Routine] {
        &self.deps
    }

    /// The registers it reads: its inputs
    pub fn reads(&self) -> Regs {
        self.reads
    }

    /// The registers it returns a result in
    pub fn returns(&self) -> Regs {
        self.returns
    }

    /// The registers it changes without a meaning for the caller
    pub fn clobbers(&self) -> Regs {
        self.clobbers
    }

    /// The registers a call may change: the ones it returns and the ones it clobbers
    pub fn changes(&self) -> Regs {
        self.returns | self.clobbers
    }

    /// The registers a call keeps: every one it neither returns nor clobbers
    pub fn preserves(&self) -> Regs {
        Regs::ALL - self.changes()
    }

    /// The bytes of WRAM it needs, in the order they were given
    pub fn variables(&self) -> &[String] {
        &self.variables
    }

    /// A call to the routine, `call Name`, to use as code ([`Emittable`](super::flow::Emittable))
    pub fn call(&self) -> Call {
        Call::new(&self.name)
    }

    /// The routine and every routine it needs, directly or through others, each once: the
    /// dependencies first, in the order they were given (each one after its own), then the
    /// routine
    ///
    /// # Panics
    /// If two of them have one name but different code, or a routine needs itself through
    /// others (a cycle can only be built by hand: a routine's dependencies exist before it).
    pub fn with_deps(&self) -> Vec<&Routine> {
        fn visit<'a>(routine: &'a Routine, out: &mut Vec<&'a Routine>, path: &mut Vec<&'a str>) {
            if let Some(seen) = out.iter().find(|r| r.name == routine.name) {
                if *seen != routine {
                    panic!(
                        "two different routines are named \"{}\" among the dependencies of \"{}\"",
                        routine.name,
                        path.first().copied().unwrap_or(routine.name.as_str())
                    );
                }
                return;
            }
            if path.contains(&routine.name.as_str()) {
                panic!("routine \"{}\" depends on itself", routine.name);
            }
            path.push(&routine.name);
            for dep in &routine.deps {
                visit(dep, out, path);
            }
            path.pop();
            out.push(routine);
        }
        let mut out = Vec::new();
        visit(self, &mut out, &mut Vec::new());
        out
    }

    /// The code of [`with_deps`](Self::with_deps): the routine's body and every dependency's,
    /// each once, the routine first (so the code can be run, or placed, from its label)
    pub fn code_with_deps(&self) -> Vec<Instr> {
        let all = self.with_deps();
        let (routine, deps) = all.split_last().expect("with_deps holds the routine");
        let mut code = routine.body.clone();
        for dep in deps {
            code.extend(dep.body.iter().cloned());
        }
        code
    }
}

impl From<Routine> for Vec<Instr> {
    fn from(routine: Routine) -> Vec<Instr> {
        routine.body
    }
}

/// Its body: `asm.emit_all(memcopy())` writes the routine
impl IntoIterator for Routine {
    type Item = Instr;
    type IntoIter = std::vec::IntoIter<Instr>;

    fn into_iter(self) -> Self::IntoIter {
        self.body.into_iter()
    }
}

#[cfg(test)]
pub(crate) mod tests;
