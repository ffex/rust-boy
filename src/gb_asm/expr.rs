//! Expressions: the numbers, symbols and arithmetic an operand can hold.
//!
//! An [`Expr`] is printed as RGBDS reads it. It is built in a typed way:
//! - numbers: [`Expr::num`] (decimal, also negative), [`Expr::hex`] (`$9800`),
//!   [`Expr::bin`] (`%11100100`), or a Rust integer (`5`, `200u8`, `-1`);
//! - symbols: [`Expr::sym`] (`wScore`, `_OAMRAM`, `PADF_LEFT`, a `DEF` constant, a local
//!   label `.loop`), checked to be a valid RGBDS name and not a register (`a`, `hl`, …);
//! - arithmetic with the Rust operators `+ - * << >> & | ^`, unary `-` and `!` (`~` in
//!   RGBDS), and [`Expr::low`] / [`Expr::high`]: `Expr::sym("_OAMRAM") + 4` prints
//!   `_OAMRAM+4`, `Expr::sym("TilesEnd") - "Tiles"` prints `TilesEnd - Tiles`;
//! - text (`Expr::from("wScore")`, or any `&str` where an operand is expected): a symbol name or a number
//!   in any RGBDS form (`"$9800"`, `"%1010"`, `"-1"`); anything else panics.
//!
//! [`Expr::raw`] is the escape hatch: RGBDS expression text, printed as it is and never
//! checked (`Expr::raw("BANK(Tiles)")`, `Expr::raw("@ - Start")`).
//!
//! Parentheses are added where RGBDS needs them. Its precedence is not C's: `&`, `|` and
//! `^` bind tighter than `+` and `-` (rgbasm(5)), so `(a + b) | c` keeps its parentheses
//! and `a + (b | c)` loses them.
//!
//! # Example
//! ```
//! use rust_boy::gb_asm::Expr;
//!
//! assert_eq!((Expr::sym("_OAMRAM") + 4).to_string(), "_OAMRAM+4");
//! let lcdc = Expr::sym("LCDCF_ON") | "LCDCF_BGON" | "LCDCF_OBJON";
//! assert_eq!(lcdc.to_string(), "LCDCF_ON | LCDCF_BGON | LCDCF_OBJON");
//! assert_eq!(Expr::hex(0x9800).to_string(), "$9800");
//! assert_eq!(Expr::high(Expr::hex(0x9800) + 33).value(), Some(0x98));
//! assert_eq!(Expr::from("-1"), Expr::num(-1));
//! ```

use std::fmt;
use std::ops;

use super::labels::{KEYWORDS, RESERVED_WORDS, is_identifier};

/// How a number is written
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radix {
    /// Decimal: `144`, `-1`
    Dec,
    /// Hexadecimal with at least this many digits: `$9800`, `$0F`
    Hex(u8),
    /// Binary with at least this many digits: `%11100100`
    Bin(u8),
}

/// A binary operator of RGBDS expressions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    /// `<<`
    Shl,
    /// `>>`, the arithmetic (signed) shift
    Shr,
    /// `&`
    And,
    /// `|`
    Or,
    /// `^`
    Xor,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::And => "&",
            BinOp::Or => "|",
            BinOp::Xor => "^",
        }
    }

    /// The RGBDS precedence (rgbasm(5)), higher binds tighter: `*`, then `<<` `>>`, then
    /// `&` `|` `^` (one level, left to right), then `+` `-`
    fn precedence(self) -> u8 {
        match self {
            BinOp::Mul => 4,
            BinOp::Shl | BinOp::Shr => 3,
            BinOp::And | BinOp::Or | BinOp::Xor => 2,
            BinOp::Add | BinOp::Sub => 1,
        }
    }
}

/// A constant or address expression; see the [module documentation](self)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A number, and how it is written
    Num(i32, Radix),
    /// A symbol: a label, a variable, a `DEF` constant or a `hardware.inc` name
    Sym(String),
    /// `-e`
    Neg(Box<Expr>),
    /// `~e`, the bitwise complement
    Not(Box<Expr>),
    /// `left op right`
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `LOW(e)`: the low byte
    Low(Box<Expr>),
    /// `HIGH(e)`: the high byte of the low 16 bits
    High(Box<Expr>),
    /// RGBDS expression text, printed as it is and never checked ([`Expr::raw`])
    Raw(String),
}

/// The register and condition names, which RGBDS never reads as symbols
const REGISTER_NAMES: [&str; 17] = [
    "a", "b", "c", "d", "e", "h", "l", "af", "bc", "de", "hl", "sp", "hli", "hld", "z", "nz", "nc",
];

/// `Ok` if `name` can be an [`Expr::Sym`]: an RGBDS identifier (`wScore`), a local label
/// (`.loop`) or a scoped one (`Main.loop`), and not a register or condition name nor
/// another RGBDS keyword (`ld`, `LOW`, `DEF`, `SECTION`, …)
fn check_symbol(name: &str) -> Result<(), String> {
    let lower = name.to_ascii_lowercase();
    if REGISTER_NAMES.contains(&lower.as_str()) {
        return Err(format!(
            "{:?} is a register or condition, not a symbol: use the typed operand \
             (`R8::A`, `R16::HL`, `Condition::Z`, …)",
            name
        ));
    }
    if KEYWORDS.contains(&lower.as_str()) || RESERVED_WORDS.contains(&lower.as_str()) {
        return Err(format!(
            "{:?} is an RGBDS keyword, not a symbol (a function such as `LOW(x)` is \
             `Expr::low(..)`, or write it with Expr::raw)",
            name
        ));
    }
    let valid = match name.split_once('.') {
        Some(("", local)) => is_identifier(local),
        Some((scope, local)) => is_identifier(scope) && is_identifier(local),
        None => is_identifier(name),
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{:?} is not a symbol name (a letter or `_`, then letters, digits, `_`, `#`, `$` \
             or `@`; `.name` for a local label)",
            name
        ))
    }
}

impl Expr {
    /// A number, written in decimal: `Expr::num(-1)` is `-1`
    pub fn num(value: i32) -> Expr {
        Expr::Num(value, Radix::Dec)
    }

    /// A number written in hexadecimal: `$0F` up to `$FF`, `$9800` above
    pub fn hex(value: u16) -> Expr {
        let digits = if value <= 0xFF { 2 } else { 4 };
        Expr::Num(i32::from(value), Radix::Hex(digits))
    }

    /// A byte written in binary, 8 digits: `%11100100`
    pub fn bin(value: u8) -> Expr {
        Expr::Num(i32::from(value), Radix::Bin(8))
    }

    /// The symbol `name`: a label, a variable, a `DEF` constant, a `hardware.inc` name,
    /// or a local label (`.loop`)
    ///
    /// # Panics
    /// If `name` is not a valid RGBDS symbol name, or is a register or condition name
    /// (`a`, `hl`, `nz`, …: use the typed operands, `R8::A`, `R16::HL`).
    #[track_caller]
    pub fn sym(name: impl Into<String>) -> Expr {
        let name = name.into();
        if let Err(error) = check_symbol(&name) {
            panic!("Expr::sym: {}", error);
        }
        Expr::Sym(name)
    }

    /// RGBDS expression text, printed as it is: the escape hatch for what the typed
    /// constructors do not build (`BANK(Tiles)`, `@ - Start`, a macro argument)
    ///
    /// Nothing checks it, so a mistake shows only when rgbasm reads it; a `raw` expression
    /// inside another one is put in parentheses.
    pub fn raw(text: impl Into<String>) -> Expr {
        Expr::Raw(text.into())
    }

    /// `LOW(e)`: the low byte of `e`
    pub fn low(e: impl Into<Expr>) -> Expr {
        Expr::Low(Box::new(e.into()))
    }

    /// `HIGH(e)`: the high byte of the low 16 bits of `e`
    pub fn high(e: impl Into<Expr>) -> Expr {
        Expr::High(Box::new(e.into()))
    }

    /// The value of a constant expression, as RGBDS computes it (32-bit, wrapping); `None`
    /// if it holds a symbol or raw text, or shifts by less than 0 or more than 31 bits
    pub fn value(&self) -> Option<i32> {
        match self {
            Expr::Num(value, _) => Some(*value),
            Expr::Sym(_) | Expr::Raw(_) => None,
            Expr::Neg(e) => e.value().map(i32::wrapping_neg),
            Expr::Not(e) => e.value().map(|v| !v),
            Expr::Low(e) => e.value().map(|v| v & 0xFF),
            Expr::High(e) => e.value().map(|v| (v >> 8) & 0xFF),
            Expr::Binary(op, left, right) => {
                let (l, r) = (left.value()?, right.value()?);
                let shift = || u32::try_from(r).ok().filter(|r| *r < 32);
                Some(match op {
                    BinOp::Add => l.wrapping_add(r),
                    BinOp::Sub => l.wrapping_sub(r),
                    BinOp::Mul => l.wrapping_mul(r),
                    BinOp::Shl => l.wrapping_shl(shift()?),
                    BinOp::Shr => l >> shift()?,
                    BinOp::And => l & r,
                    BinOp::Or => l | r,
                    BinOp::Xor => l ^ r,
                })
            }
        }
    }

    fn binary(op: BinOp, left: Expr, right: Expr) -> Expr {
        Expr::Binary(op, Box::new(left), Box::new(right))
    }

    /// Whether this is a symbol, or a symbol plus or minus offsets: the left side of
    /// a compact `sym+n`
    fn is_symbol_offset(&self) -> bool {
        match self {
            Expr::Sym(_) => true,
            Expr::Binary(BinOp::Add | BinOp::Sub, left, right) => {
                left.is_symbol_offset() && right.is_offset()
            }
            _ => false,
        }
    }

    /// Whether this is a decimal number from 0 up: the right side of a compact `sym+n`
    fn is_offset(&self) -> bool {
        matches!(self, Expr::Num(value, Radix::Dec) if *value >= 0)
    }

    /// Write `self` as an operand of an operator of precedence `parent` (unary operators
    /// and function arguments: 5), in parentheses if it binds less tightly or is raw text;
    /// `right` for the right operand of a binary operator, which needs them on a tie too
    fn fmt_operand(&self, f: &mut fmt::Formatter<'_>, parent: u8, right: bool) -> fmt::Result {
        let parens = match self {
            Expr::Binary(op, ..) => {
                op.precedence() < parent || (right && op.precedence() == parent)
            }
            Expr::Raw(_) => true,
            Expr::Num(value, _) => *value < 0 && parent == 5,
            _ => false,
        };
        if parens {
            write!(f, "({})", self)
        } else {
            write!(f, "{}", self)
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Num(value, Radix::Dec) => write!(f, "{}", value),
            Expr::Num(value, Radix::Hex(digits)) => {
                write!(f, "${:0width$X}", value, width = usize::from(*digits))
            }
            Expr::Num(value, Radix::Bin(digits)) => {
                write!(f, "%{:0width$b}", value, width = usize::from(*digits))
            }
            Expr::Sym(text) | Expr::Raw(text) => write!(f, "{}", text),
            Expr::Neg(e) => {
                write!(f, "-")?;
                e.fmt_operand(f, 5, false)
            }
            Expr::Not(e) => {
                write!(f, "~")?;
                e.fmt_operand(f, 5, false)
            }
            Expr::Low(e) => write!(f, "LOW({})", e),
            Expr::High(e) => write!(f, "HIGH({})", e),
            Expr::Binary(op, left, right) => {
                let precedence = op.precedence();
                left.fmt_operand(f, precedence, false)?;
                // A symbol and its offsets are written together, as an address:
                // `_OAMRAM+4`, `wScore+1`; any other operation with spaces
                let offset = matches!(op, BinOp::Add | BinOp::Sub);
                if offset && left.is_symbol_offset() && right.is_offset() {
                    write!(f, "{}", op.symbol())?;
                } else {
                    write!(f, " {} ", op.symbol())?;
                }
                right.fmt_operand(f, precedence, true)
            }
        }
    }
}

/// The value of an RGBDS number literal (`$FF`, `0xFF`, `%101`, `0b101`, `&17`, `0o17`,
/// decimal, `_` between digits), and how to write it again; `None` if it is not one
pub(crate) fn parse_number(text: &str) -> Option<(i32, Radix)> {
    let prefixes: [(&str, u32); 8] = [
        ("$", 16),
        ("0x", 16),
        ("0X", 16),
        ("%", 2),
        ("0b", 2),
        ("0B", 2),
        ("&", 8),
        ("0o", 8),
    ];
    let (digits, radix) = prefixes
        .iter()
        .find_map(|(prefix, radix)| text.strip_prefix(prefix).map(|rest| (rest, *radix)))
        .unwrap_or((text, 10));
    if !digits.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return None;
    }
    let clean: String = digits.chars().filter(|&c| c != '_').collect();
    let value = u32::from_str_radix(&clean, radix).ok()?;
    let width = u8::try_from(clean.len()).unwrap_or(u8::MAX);
    let written = match radix {
        16 => Radix::Hex(width),
        2 => Radix::Bin(width),
        _ => Radix::Dec,
    };
    // A 32-bit number, as RGBDS reads it ($FFFFFFFF is -1)
    Some((value as i32, written))
}

/// A symbol name or an RGBDS number literal: `"wScore"`, `"PADF_LEFT"`, `".loop"`,
/// `"$9800"`, `"%11100100"`, `"144"`, `"-1"`; spaces around it are ignored
///
/// Every function that takes an `impl Into<Expr>` reads text this way (`IfConst`, `IfA`,
/// `TileRef::set_tile_label`, `cp_in_memory`, the operands of `Asm` and `Block`, …), and
/// so does `is_specific_tile` for its tile ids.
///
/// # Panics
/// On anything else, with what to write instead: a register name (`"a"`, `"hl"`: use
/// `R8::A`, `R16::HL`), another RGBDS keyword (`"LOW"`, `"ld"`), an expression
/// (`"TilesEnd - Tiles"`, `"LOW(X)"`: build it, `Expr::sym("TilesEnd") - "Tiles"`,
/// `Expr::low(..)`, or use [`Expr::raw`]), a memory operand (`"[wScore]"`: use
/// `Mem::addr("wScore")`), a number that does not fit in 32 bits. A character literal
/// (`"'A'"`) and a raw identifier (`"#name"`) are not read either: write them with
/// [`Expr::raw`].
impl From<&str> for Expr {
    #[track_caller]
    fn from(text: &str) -> Expr {
        let text = text.trim();
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        if let Some((value, radix)) = parse_number(unsigned) {
            return match (negative, radix) {
                (false, _) => Expr::Num(value, radix),
                (true, Radix::Dec) => Expr::num(value.wrapping_neg()),
                (true, _) => -Expr::Num(value, radix),
            };
        }
        let starts_like_a_number = unsigned.starts_with(|c: char| c.is_ascii_digit())
            || (unsigned.len() > 1 && unsigned.starts_with(['$', '%', '&']));
        if starts_like_a_number {
            panic!(
                "{:?} is not an RGBDS number that fits in 32 bits (`$FF`, `%101`, `&17`, `255`; \
                 an expression is built with Expr, or written with Expr::raw)",
                text
            );
        }
        match check_symbol(text) {
            Ok(()) => Expr::Sym(text.to_string()),
            Err(error) => panic!(
                "{} (an expression is built with Expr: `Expr::sym(\"TilesEnd\") - \"Tiles\"`, \
                 or written with Expr::raw; a memory operand is `Mem::addr(..)`)",
                error
            ),
        }
    }
}

impl From<String> for Expr {
    #[track_caller]
    fn from(text: String) -> Expr {
        Expr::from(text.as_str())
    }
}

impl From<&String> for Expr {
    #[track_caller]
    fn from(text: &String) -> Expr {
        Expr::from(text.as_str())
    }
}

impl From<&Expr> for Expr {
    fn from(e: &Expr) -> Expr {
        e.clone()
    }
}

macro_rules! from_integer {
    ($($t:ty),*) => {$(
        /// A number, written in decimal
        impl From<$t> for Expr {
            fn from(value: $t) -> Expr {
                Expr::num(i32::from(value))
            }
        }
    )*};
}
from_integer!(u8, i8, u16, i16, i32);

macro_rules! binary_operator {
    ($($trait:ident $method:ident $op:ident),*) => {$(
        impl<T: Into<Expr>> ops::$trait<T> for Expr {
            type Output = Expr;

            #[track_caller]
            fn $method(self, right: T) -> Expr {
                Expr::binary(BinOp::$op, self, right.into())
            }
        }
    )*};
}
binary_operator!(
    Add add Add,
    Sub sub Sub,
    Mul mul Mul,
    Shl shl Shl,
    Shr shr Shr,
    BitAnd bitand And,
    BitOr bitor Or,
    BitXor bitxor Xor
);

impl ops::Neg for Expr {
    type Output = Expr;

    fn neg(self) -> Expr {
        Expr::Neg(Box::new(self))
    }
}

/// `!e` is RGBDS's `~e`, the bitwise complement
impl ops::Not for Expr {
    type Output = Expr;

    fn not(self) -> Expr {
        Expr::Not(Box::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(e: Expr) -> String {
        e.to_string()
    }

    #[test]
    fn test_numbers() {
        assert_eq!(text(Expr::num(144)), "144");
        assert_eq!(text(Expr::num(-1)), "-1");
        assert_eq!(text(Expr::hex(0x9800)), "$9800");
        assert_eq!(text(Expr::hex(0x0F)), "$0F");
        assert_eq!(text(Expr::hex(0x100)), "$0100");
        assert_eq!(text(Expr::bin(0b1110_0100)), "%11100100");
        assert_eq!(text(Expr::bin(0b1111)), "%00001111");
        assert_eq!(text(5u8.into()), "5");
        assert_eq!(text(1000u16.into()), "1000");
        assert_eq!(text((-128i8).into()), "-128");
    }

    #[test]
    fn test_text_is_a_symbol_or_a_number() {
        let cases = [
            ("wScore", Expr::Sym("wScore".to_string())),
            (".loop", Expr::Sym(".loop".to_string())),
            ("Main.loop", Expr::Sym("Main.loop".to_string())),
            ("_OAMRAM", Expr::Sym("_OAMRAM".to_string())),
            ("$9800", Expr::Num(0x9800, Radix::Hex(4))),
            ("$00", Expr::Num(0, Radix::Hex(2))),
            ("0x1A", Expr::Num(0x1A, Radix::Hex(2))),
            ("%11100100", Expr::Num(0xE4, Radix::Bin(8))),
            ("0b101", Expr::Num(5, Radix::Bin(3))),
            ("&17", Expr::Num(15, Radix::Dec)),
            ("0o17", Expr::Num(15, Radix::Dec)),
            ("1_000", Expr::num(1000)),
            ("144", Expr::num(144)),
            ("-1", Expr::num(-1)),
            ("-$10", -Expr::Num(0x10, Radix::Hex(2))),
        ];
        for (text, expr) in cases {
            assert_eq!(Expr::from(text), expr, "{}", text);
        }
        // Spaces around the text are ignored
        assert_eq!(Expr::from(" BRICK "), Expr::sym("BRICK"));
        assert_eq!(Expr::from("\t$10 "), Expr::hex(0x10));
        assert_eq!(Expr::from(" -1"), Expr::num(-1));
        // Written back the same way
        for text in ["wScore", "$9800", "$00", "%11100100", "144", "-1", "-$10"] {
            assert_eq!(Expr::from(text).to_string(), text);
        }
    }

    /// The panic message of `f`
    fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let error = std::panic::catch_unwind(f).expect_err("it should panic");
        error
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    }

    #[test]
    fn test_registers_and_expressions_are_not_symbols() {
        // A register in a symbol was the bug of the string helpers: `cp_label("b")` was
        // an expression `b`, which `instr_size` counted as an immediate (2 bytes, not 1)
        for name in ["a", "B", "hl", "SP", "hli", "nz", "c"] {
            let message = panic_message(|| {
                let _ = Expr::from(name);
            });
            assert!(
                message.contains("is a register or condition"),
                "{}",
                message
            );
            let message = panic_message(|| {
                let _ = Expr::sym(name);
            });
            assert!(
                message.contains("is a register or condition"),
                "{}",
                message
            );
        }
        for text in [
            "TilesEnd - Tiles",
            "[wScore]",
            "_OAMRAM+4",
            "",
            "$",
            "a.b.c",
            "'A'",
        ] {
            let message = panic_message(|| {
                let _ = Expr::from(text);
            });
            assert!(message.contains("is not a symbol name"), "{}", message);
        }
        // A number that is not one, or does not fit in 32 bits
        for text in ["1abc", "4294967296", "$1_0000_0000", "%2"] {
            let message = panic_message(|| {
                let _ = Expr::from(text);
            });
            assert!(message.contains("is not an RGBDS number"), "{}", message);
        }
        // RGBDS keywords: functions, mnemonics, directives, in any case
        for text in ["LOW", "high", "ld", "DEF", "Section", "db", "BANK"] {
            for message in [
                panic_message(|| {
                    let _ = Expr::from(text);
                }),
                panic_message(|| {
                    let _ = Expr::sym(text);
                }),
            ] {
                assert!(message.contains("is an RGBDS keyword"), "{}", message);
            }
        }
        // Raw text is never checked
        assert_eq!(text(Expr::raw("BANK(Tiles)")), "BANK(Tiles)");
    }

    #[test]
    fn test_operators() {
        let sym = Expr::sym;
        assert_eq!(text(sym("_OAMRAM") + 4), "_OAMRAM+4");
        assert_eq!(text(sym("_OAMRAM") + 4 + 1), "_OAMRAM+4+1");
        assert_eq!(text(sym("_OAMRAM") + 0), "_OAMRAM+0");
        assert_eq!(text(sym("wScore") - 1), "wScore-1");
        assert_eq!(text(sym("TilesEnd") - "Tiles"), "TilesEnd - Tiles");
        assert_eq!(text(Expr::num(16) + 1), "16 + 1");
        assert_eq!(text(sym("X") + Expr::hex(4)), "X + $04");
        assert_eq!(text(sym("X") + (-1)), "X + -1");
        assert_eq!(
            text(sym("LCDCF_ON") | "LCDCF_BGON" | "LCDCF_OBJON"),
            "LCDCF_ON | LCDCF_BGON | LCDCF_OBJON"
        );
        assert_eq!(text(Expr::low(sym("Tiles") + 2)), "LOW(Tiles+2)");
        assert_eq!(text(Expr::high(sym("Tiles"))), "HIGH(Tiles)");
        assert_eq!(text(-sym("X")), "-X");
        assert_eq!(text(!sym("PADF_A")), "~PADF_A");
        assert_eq!(text(-(sym("X") + 1)), "-(X+1)");
        assert_eq!(text(-Expr::num(-1)), "-(-1)");
        // Parentheses where RGBDS needs them: `|` binds tighter than `+`
        assert_eq!(text((sym("A1") + 1) | "B1"), "(A1+1) | B1");
        assert_eq!(text(sym("A1") + (sym("B1") | 2)), "A1 + B1 | 2");
        assert_eq!(text(sym("A1") - (sym("B1") - 1)), "A1 - (B1-1)");
        assert_eq!(text((sym("A1") - "B1") - 1), "A1 - B1 - 1");
        assert_eq!(text((sym("A1") << 2) * 3), "(A1 << 2) * 3");
        assert_eq!(text(Expr::raw("@ - Start") + 1), "(@ - Start) + 1");
    }

    #[test]
    fn test_value() {
        assert_eq!(Expr::num(-1).value(), Some(-1));
        assert_eq!((Expr::hex(0x9800) + 33).value(), Some(0x9821));
        assert_eq!(Expr::low(Expr::hex(0x1234)).value(), Some(0x34));
        assert_eq!(Expr::high(Expr::hex(0x1234)).value(), Some(0x12));
        assert_eq!(Expr::high(Expr::num(-1)).value(), Some(0xFF));
        assert_eq!((Expr::num(1) << 4 | 1).value(), Some(17));
        assert_eq!((Expr::num(-16) >> 2).value(), Some(-4));
        assert_eq!((!Expr::num(0)).value(), Some(-1));
        assert_eq!((Expr::num(1) << 32).value(), None);
        assert_eq!((Expr::sym("X") + 1).value(), None);
        assert_eq!(Expr::raw("1").value(), None);
        assert_eq!(Expr::from("$FFFFFFFF").value(), Some(-1));
    }
}
