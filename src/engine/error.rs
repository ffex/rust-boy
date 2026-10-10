//! What can go wrong when `RustBoy::build` puts a program together: [`Error`]

use std::fmt;

use super::memory::MemoryRegion;
use super::variables::VarType;

/// A problem of the program as a whole, found by [`RustBoy::build`](super::RustBoy::build)
///
/// # Panic or `Err`: the rule
///
/// - **A method panics** when the call itself is wrong: an argument is invalid (a name
///   that is not an RGBDS identifier, a value out of range, an id this program did not
///   hand out, an odd tile count in 8x16 mode, an instruction built by hand that
///   `Instr::check` rejects), or the call contradicts an earlier call on the same object (a
///   variable created again with another type, a sprite name used twice, another routine
///   under a taken name). The bug is at that line, and the panic says what is wrong with
///   it. VRAM tiles and OAM entries are allocated by the call that adds them, because that
///   call needs them at once (a sprite's tile index and OAM entry go into the code its
///   moves and animations generate), so running out of them panics there too.
/// - **`build()` returns an `Error`** for what only the whole program shows, once every
///   call is made: one name defined in two places that do not see each other (a function
///   and a variable, a constant, a raw label, an external symbol), a variable `build()`
///   needs that the program created with another type, a function named by `call` /
///   `call_args` / `keep_function` that nothing defines (it may be defined after the call,
///   so the call does not check), variables that do not fit in their memory (they are laid
///   out by `build()`, with the ones it adds), and a section that holds what it cannot
///   (code or data in RAM, a section name used twice).
///
/// `build()` itself does not panic on what a program contains: every problem it finds is
/// an `Err`. (A panic in `build()` is a bug of the library.)
///
/// # Example
/// ```
/// use rust_boy::asm::Block;
/// use rust_boy::engine::{Definition, Error, RustBoy, VarType};
///
/// let mut gb = RustBoy::new();
/// let mut body = Block::new();
/// body.label("Jump").ret();
/// gb.define_function("Jump", body.into_instrs());
/// let call = gb.call("Jump");
/// gb.add_to_main_loop(call);
/// // Each call is fine; the program is not: `call Jump` would reach the variable
/// gb.vars.create_u8("Jump", 0);
/// assert_eq!(
///     gb.build(),
///     Err(Error::NameConflict {
///         name: "Jump".to_string(),
///         first: Definition::Function,
///         second: Definition::Variable(VarType::U8),
///     })
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A function named by `RustBoy::call`, `call_args` or `keep_function` is not a
    /// builtin, a user function, nor a function `build()` generates (an animation)
    UnknownFunction {
        /// The name that was used
        name: String,
        /// Every function the program has, sorted
        available: Vec<String>,
    },
    /// One name has two definitions, so a reference to it could reach only one of them
    NameConflict {
        /// The name defined twice
        name: String,
        /// The definition found first
        first: Definition,
        /// The other one
        second: Definition,
    },
    /// The variables do not fit in their memory region
    MemoryFull {
        /// Where they go
        region: MemoryRegion,
        /// The first one that does not fit, e.g. ``variable `wScore` (2 bytes)``
        what: String,
        /// Its size in bytes
        needed: usize,
        /// The bytes left in the region before it
        available: usize,
    },
    /// The program puts something in a section that cannot hold it: code or data in a
    /// RAM section (it only reserves space), or a section name used twice; the message is
    /// the asm layer's
    Section(String),
}

/// What a name is defined as, in an [`Error::NameConflict`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Definition {
    /// A user function (`define_function`, `define_function_from`, `define_routine`)
    Function,
    /// A variable the program created, of this type
    Variable(VarType),
    /// A variable `build()` creates for the code it generates, of this type (the
    /// animations' `wFrameCounter` and `wAnim_*`)
    GeneratedVariable(VarType),
    /// A symbol defined outside the generated code (`RustBoy::external_symbol`)
    ExternalSymbol,
    /// A constant or a label of the program's code (`define_const`, a `DEF` or a label in
    /// `raw()` code)
    CodeSymbol,
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Definition::Function => write!(f, "a function"),
            Definition::Variable(var_type) => write!(f, "a {:?} variable", var_type),
            Definition::GeneratedVariable(var_type) => {
                write!(f, "a {:?} variable that build() needs", var_type)
            }
            Definition::ExternalSymbol => write!(
                f,
                "an external symbol (a function is defined either with define_function or \
                 outside the generated code, with external_symbol)"
            ),
            Definition::CodeSymbol => write!(
                f,
                "a constant or a label of the program (define_const, a DEF, or a label in \
                 raw code)"
            ),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownFunction { name, available } => write!(
                f,
                "unknown function '{}': define it, or call one of {}",
                name,
                available.join(", ")
            ),
            Error::NameConflict {
                name,
                first,
                second,
            } => write!(
                f,
                "`{}` is {} and also {}: a reference to it would reach only one of them; \
                 rename one of them",
                name, first, second
            ),
            Error::MemoryFull {
                region,
                what,
                needed,
                available,
            } => write!(
                f,
                "no room for {}: {} bytes needed, but {:?} (${:04X}-${:04X}, {} bytes) has {} \
                 bytes left",
                what,
                needed,
                region,
                region.start_address(),
                region.end_address() - 1,
                region.size(),
                available
            ),
            Error::Section(message) => write!(f, "invalid program: {}", message),
        }
    }
}

impl std::error::Error for Error {}
