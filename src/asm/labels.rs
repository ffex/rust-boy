//! Labels for generated code: unique local labels, and the RGBDS rules for names.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Hands out the generated labels of a program: unique local labels, numbered
///
/// Every label that generated code makes up comes from one allocator per program: the
/// labels of `If` (`.end_if_N`, `.else_N`, `.then_N`), of the snippets (key checks,
/// limited moves, the OAM clear loop) and of the animation dispatcher. The program's
/// [`Asm`](super::Asm) owns it ([`Asm::labels`](super::Asm::labels)), and
/// [`Emittable::emit`](super::Emittable::emit) takes it. The labels:
/// - are **local** (`.name`): a global label starts a new RGBDS label scope, and an
///   enclosing `If` would then not find its own `.end_if_N`;
/// - are **unique by construction**: each call takes the next number `N` of the
///   sequence, and every label it returns is `.{stem}_N`, with distinct stems made of
///   identifier characters. The number is the text after the last `_`, so two labels
///   are equal only if they have the same stem and the same number, which no two calls
///   share.
///
/// Clones share one counter, so all the generators that hold a clone of a program's
/// allocator draw from the same sequence. The numbers only depend on the order of the
/// calls, so the same program always gets the same labels.
///
/// A routine (a global label emitted once, such as `Memcopy` or an animation function)
/// keeps its own fixed local labels (`.copy`): they live in the scope of its global
/// label, which is unique.
///
/// # Example
/// ```
/// use rust_boy::asm::LabelAllocator;
///
/// let labels = LabelAllocator::new();
/// let shared = labels.clone();
/// assert_eq!(labels.local("check_left"), ".check_left_0");
/// assert_eq!(shared.local("check_left"), ".check_left_1");
/// let [end, other] = labels.locals(["end_if", "else"]);
/// assert_eq!((end.as_str(), other.as_str()), (".end_if_2", ".else_2"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct LabelAllocator {
    next: Arc<AtomicUsize>,
}

impl LabelAllocator {
    /// A new allocator, whose sequence starts at 0
    pub fn new() -> Self {
        Self::default()
    }

    /// The next number of the sequence (0, 1, 2, ...), shared by every clone
    pub fn next_id(&self) -> usize {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// A local label `.{stem}_{n}` that no other call returns
    ///
    /// # Panics
    /// If `stem` is empty or has a character other than letters, digits, `_`, `#`, `$`
    /// and `@` (the characters of an RGBDS identifier).
    #[track_caller]
    pub fn local(&self, stem: &str) -> String {
        let [label] = self.locals([stem]);
        label
    }

    /// Several local labels with one number: `.{stem}_{n}` for each stem, for a
    /// snippet that needs more than one (`.check_left_3`, `.check_left_end_3`)
    ///
    /// # Panics
    /// If a stem is not made of identifier characters (see [`LabelAllocator::local`]), or
    /// two stems are the same.
    #[track_caller]
    pub fn locals<const N: usize>(&self, stems: [&str; N]) -> [String; N] {
        for (i, stem) in stems.iter().enumerate() {
            if stem.is_empty()
                || !stem
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '#' | '$' | '@'))
            {
                panic!(
                    "invalid label stem {:?}: use letters, digits, `_`, `#`, `$` or `@`",
                    stem
                );
            }
            if stems[..i].contains(stem) {
                panic!("label stem {:?} given twice for one label number", stem);
            }
        }
        let n = self.next_id();
        stems.map(|stem| format!(".{}_{}", stem, n))
    }

    /// A new allocator that goes on with this sequence from where it is, without sharing
    /// it: the numbers it hands out do not advance this one
    ///
    /// For code generated again on each `build()` (the start-up code, the animation
    /// dispatcher): it takes the numbers after every label handed out so far, and two
    /// builds of the same program give the same labels.
    pub fn fork(&self) -> Self {
        Self {
            next: Arc::new(AtomicUsize::new(self.next.load(Ordering::Relaxed))),
        }
    }
}

/// Words of an operand that are not symbols, compared without case: registers,
/// conditions, and every function and section keyword of the RGBDS 1.0.4 lexer
/// (`src/asm/lexer.cpp`, the `OP_*` and section tokens)
pub(crate) const KEYWORDS: &[&str] = &[
    // Registers and conditions
    "a",
    "b",
    "c",
    "d",
    "e",
    "h",
    "l",
    "af",
    "bc",
    "de",
    "hl",
    "sp",
    "hli",
    "hld",
    "z",
    "nz",
    "nc",
    // Symbols and sections
    "def",
    "bank",
    "sizeof",
    "startof",
    "fragment",
    "align",
    "isconst",
    "high",
    "low",
    // Fixed-point math
    "round",
    "ceil",
    "floor",
    "div",
    "mul",
    "fmod",
    "pow",
    "log",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "bitwidth",
    "tzcount",
    // Strings and charmaps
    "bytelen",
    "readfile",
    "strbyte",
    "strcat",
    "strchar",
    "strcmp",
    "strfind",
    "strfmt",
    "strin",
    "strlen",
    "strlwr",
    "strrfind",
    "strrin",
    "strrpl",
    "strslice",
    "strsub",
    "strupr",
    "charcmp",
    "charlen",
    "charsize",
    "charsub",
    "charval",
    "incharmap",
    "revchar",
];

/// The instruction mnemonics and the directives of RGBDS 1.0.4, compared without case:
/// they cannot be symbol names either (`ld`, `DEF`, `SECTION`, ...)
pub(crate) const RESERVED_WORDS: &[&str] = &[
    // Instructions
    "adc",
    "add",
    "and",
    "bit",
    "call",
    "ccf",
    "cp",
    "cpl",
    "daa",
    "dec",
    "di",
    "ei",
    "halt",
    "inc",
    "jp",
    "jr",
    "ld",
    "ldh",
    "ldi",
    "ldd",
    "nop",
    "or",
    "pop",
    "push",
    "res",
    "ret",
    "reti",
    "rl",
    "rla",
    "rlc",
    "rlca",
    "rr",
    "rra",
    "rrc",
    "rrca",
    "rst",
    "sbc",
    "scf",
    "set",
    "sla",
    "sra",
    "srl",
    "stop",
    "sub",
    "swap",
    "xor",
    // Directives
    "section",
    "include",
    "incbin",
    "equ",
    "equs",
    "redef",
    "rb",
    "rw",
    "db",
    "dw",
    "dl",
    "ds",
    "macro",
    "endm",
    "rept",
    "endr",
    "for",
    "break",
    "if",
    "elif",
    "else",
    "endc",
    "export",
    "purge",
    "print",
    "println",
    "assert",
    "static_assert",
    "fail",
    "warn",
    "shift",
    "opt",
    "pushs",
    "pops",
    "pusho",
    "popo",
    "charmap",
    "newcharmap",
    "setcharmap",
    "pushc",
    "popc",
    "union",
    "nextu",
    "endu",
    "load",
    "endl",
    "rsreset",
    "rsset",
    "endsection",
];

/// Whether `name` is a valid RGBDS symbol name: a letter or `_` first, then letters,
/// digits, `_`, `#`, `$` or `@`
///
/// RGBDS keywords (`ld`, `a`, `SECTION`, ...) are not checked here: they have valid
/// characters, but cannot be used alone as a symbol name.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '#' | '$' | '@'))
}

/// The code of each line of `text`, as RGBDS reads it, one entry per line of `text`
///
/// Comments are removed: from a `;` to the end of the line, and `/* … */` block comments,
/// which can span lines. The contents of strings (`"…"`, `'…'`, within one line) are
/// blanked. So neither is taken for code. A line that ends with `\` (outside a string
/// and a comment) continues on the next one: the joined code is the entry of its first
/// line, and the entries of the lines it took are empty, so entries keep their line
/// numbers. (Triple-quoted and raw strings, and macros, are not handled.)
pub(crate) fn code_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_block = false;
    // Index of the line a `\` continues, if any
    let mut continued: Option<usize> = None;
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut code = String::with_capacity(line.len());
        let mut quote = None;
        let mut escaped = false;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if in_block {
                if c == '*' && next == Some('/') {
                    in_block = false;
                    code.push(' ');
                    i += 1;
                }
            } else if let Some(q) = quote {
                let closes = !escaped && c == q;
                escaped = !escaped && c == '\\';
                if closes {
                    quote = None;
                    code.push(c);
                } else {
                    code.push(' ');
                }
            } else if c == ';' {
                break;
            } else if c == '/' && next == Some('*') {
                in_block = true;
                i += 1;
            } else {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                }
                code.push(c);
            }
            i += 1;
        }
        // A `\` at the end of the code continues the line
        let continues = !in_block && quote.is_none() && code.trim_end().ends_with('\\');
        if continues {
            let end = code.trim_end().len() - 1;
            code.truncate(end);
        }
        match continued {
            Some(first) => {
                lines[first] = format!("{} {}", lines[first], code);
                lines.push(String::new());
            }
            None => lines.push(code),
        }
        continued = match (continues, continued) {
            (true, Some(first)) => Some(first),
            (true, None) => Some(lines.len() - 1),
            (false, _) => None,
        };
    }
    lines
}

/// The global label a line of code (from [`code_lines`]) starts with (`Name:`,
/// `Name::`, `Name: db 1`), and the rest of the line
pub(crate) fn split_label(code: &str) -> (Option<&str>, &str) {
    match code.split_once(':') {
        Some((label, rest)) if is_identifier(label.trim()) => {
            (Some(label.trim()), rest.trim_start_matches(':'))
        }
        _ => (None, code),
    }
}

/// The symbol a line of code (from [`code_lines`], after its label) defines with `DEF`
/// (or `REDEF`), and the expression that follows the operator: `DEF Name EQU 5`,
/// `DEF Name = 5`, `DEF Name += 1`, `DEF Name EQUS "…"`, `DEF Name RB 2`, … (keywords
/// without case). `None` for any other line. The text of an `EQUS` string is not read
/// (strings are blanked), so a symbol it names is not seen.
pub(crate) fn split_def(code: &str) -> Option<(&str, &str)> {
    let code = code.trim_start();
    let (keyword, rest) = code.split_once(char::is_whitespace)?;
    if !keyword.eq_ignore_ascii_case("DEF") && !keyword.eq_ignore_ascii_case("REDEF") {
        return None;
    }
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || "_#$@".contains(c)))
        .unwrap_or(rest.len());
    let (name, after) = rest.split_at(end);
    if !is_identifier(name) {
        return None;
    }
    // The operator: a word (EQU, EQUS, RB, RW, RL) or symbols (=, +=, <<=, …)
    let after = after.trim_start();
    let value = match after.split_once(char::is_whitespace) {
        Some((word, value)) if word.chars().all(|c| c.is_ascii_alphabetic()) => value,
        _ => after.trim_start_matches(|c: char| "=+-*/%&|^<>!".contains(c)),
    };
    let value = if after.chars().all(|c| c.is_ascii_alphabetic()) {
        "" // `DEF Name RB`: no expression
    } else {
        value
    };
    Some((name, value))
}

/// The global symbols a piece of code (from [`code_lines`]) names: each word made of
/// symbol characters that is an identifier. `Scope.local` names `Scope`; a local label
/// (`.name`) and a number (`$FF`, `10`) name none. Mnemonics and registers are words too.
pub(crate) fn symbol_words(code: &str) -> impl Iterator<Item = &str> {
    code.split(|c: char| !(c.is_ascii_alphanumeric() || "_#$@.".contains(c)))
        .map(|word| word.split('.').next().unwrap_or_default())
        .filter(|word| is_identifier(word))
}

/// Add to `refs` the global symbols `instr` refers to, and to `defs` the global labels and
/// constants it defines
///
/// Typed operands are read by their type: a jump or call target, and the symbols of an
/// [`Expr`] (`ld hl, Name`, `ld a, [Name]`, `cp LOW(Name)`); a local symbol (`.end_if_0`)
/// is skipped, and `Scope.local` refers to `Scope`. A [`Label`](Instr::Label) defines its
/// name when it is global. Registers, numbers and mnemonics refer to nothing. Sections,
/// comments and file names (`INCLUDE`, `INCBIN`) refer to nothing either.
///
/// What is only text, the instructions RGBDS reads as written (a raw line, `db`, `dw`,
/// `ds`, the value of a `DEF`, an [`Expr::raw`] operand, a jump target that is not a plain
/// name), is read as RGBDS reads it, line by line ([`code_lines`]): comments (`;`,
/// `/* … */`) and the contents of strings are skipped, a line that starts with `Name:`
/// defines `Name`, as does `DEF Name …` (any form, [`split_def`]), and every other word
/// that is an identifier is a reference (mnemonics and registers too, which never name a
/// function). So a raw line of one or several lines counts like typed code.
pub(crate) fn symbols(
    instr: &super::Instr,
    refs: &mut Vec<String>,
    defs: &mut std::collections::BTreeSet<String>,
) {
    use super::{AluOperand, Dst, Instr, JumpTarget, Mem, Operand};

    let mem = |mem: &Mem, refs: &mut Vec<String>| {
        if let Mem::Addr(address) = mem {
            expr_symbols(address, refs);
        }
    };
    match instr {
        Instr::Label { name } => {
            if !name.starts_with('.') {
                defs.insert(name.clone());
            }
        }
        Instr::Def { label, value } => {
            defs.insert(label.clone());
            text_symbols(value, refs, defs);
        }
        Instr::Raw { .. } | Instr::Db { .. } | Instr::Dw { .. } | Instr::Ds { .. } => {
            text_symbols(&instr.to_string(), refs, defs);
        }
        Instr::Jp { target }
        | Instr::JpCond { target, .. }
        | Instr::Jr { target }
        | Instr::JrCond { target, .. }
        | Instr::Call { target }
        | Instr::CallCond { target, .. } => match target {
            JumpTarget::Label(name) => text_symbols(name, refs, defs),
            JumpTarget::Addr(_) => {}
        },
        Instr::Ld { dst, src } | Instr::Ldh { dst, src } => {
            if let Dst::Mem(address) = dst {
                mem(address, refs);
            }
            match src {
                Operand::Imm(value) => expr_symbols(value, refs),
                Operand::Mem(address) => mem(address, refs),
                Operand::R8(_) | Operand::R16(_) => {}
            }
        }
        Instr::Add { src }
        | Instr::Adc { src }
        | Instr::Sub { src }
        | Instr::Sbc { src }
        | Instr::And { src }
        | Instr::Xor { src }
        | Instr::Or { src }
        | Instr::Cp { src } => {
            if let AluOperand::Imm(value) = src {
                expr_symbols(value, refs);
            }
        }
        // No symbol in the operands: registers, bit numbers, `rst` vectors, offsets;
        // sections, comments and file names name no symbol of the program
        _ => {}
    }
}

/// The global symbols of the expression `e` (the text of an [`Expr::raw`] as RGBDS reads it)
fn expr_symbols(e: &super::Expr, refs: &mut Vec<String>) {
    use super::Expr;
    match e {
        Expr::Num(..) => {}
        Expr::Sym(name) => refs.extend(symbol_words(name).map(str::to_string)),
        Expr::Raw(text) => {
            for code in code_lines(text) {
                refs.extend(symbol_words(&code).map(str::to_string));
            }
        }
        Expr::Neg(e) | Expr::Not(e) | Expr::Low(e) | Expr::High(e) => expr_symbols(e, refs),
        Expr::Binary(_, left, right) => {
            expr_symbols(left, refs);
            expr_symbols(right, refs);
        }
    }
}

/// The symbols of RGBDS text, read line by line as RGBDS reads it (see [`symbols`])
fn text_symbols(text: &str, refs: &mut Vec<String>, defs: &mut std::collections::BTreeSet<String>) {
    for code in code_lines(text) {
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

/// Whether `code` defines the global symbol `name`: a label (`Name:`, also in a raw line)
/// or a `DEF`
pub(crate) fn defines(code: &[super::Instr], name: &str) -> bool {
    let mut defs = std::collections::BTreeSet::new();
    for instr in code {
        symbols(instr, &mut Vec::new(), &mut defs);
    }
    defs.contains(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The code of a single line
    fn code_of_line(line: &str) -> String {
        let lines = code_lines(line);
        assert_eq!(lines.len(), 1);
        lines[0].clone()
    }

    #[test]
    fn test_code_lines() {
        assert_eq!(code_of_line("ld a, 1 ; one"), "ld a, 1 ");
        assert_eq!(
            code_of_line("db \"a;b\", LOW(Delay)"),
            "db \"   \", LOW(Delay)"
        );
        assert_eq!(
            code_of_line("db 'x', \"say \\\"hi\\\"\" ; c"),
            "db ' ', \"          \" "
        );
        // Block comments, on one line and over several; a `/*` in a string is not one
        assert_eq!(code_of_line("R: /* call Delay */ ret"), "R:   ret");
        assert_eq!(
            code_lines("ld a, 1 /* call A\ncall B ; still\n*/ call C\ndb \"/*\", D"),
            ["ld a, 1 ", "", "  call C", "db \"  \", D"]
        );
        // DEF in all its forms; other lines define nothing
        for (code, def) in [
            ("DEF Delay EQU 5", Some(("Delay", "5"))),
            ("  def Delay = Base + 1", Some(("Delay", " Base + 1"))),
            ("DEF Delay=5", Some(("Delay", "5"))),
            ("REDEF Count += Step", Some(("Count", " Step"))),
            ("DEF wKeys RB 2", Some(("wKeys", "2"))),
            ("DEF wKeys RB", Some(("wKeys", ""))),
            ("DEF Text EQUS \"   \"", Some(("Text", "\"   \""))),
            ("ld a, DEF", None),
            ("DEFINE x", None),
        ] {
            assert_eq!(split_def(code), def, "{}", code);
        }
        // A `\` at the end of a line continues it; the entries keep their line numbers
        assert_eq!(
            code_lines("db 1, \\ ; first\n   Next, \\\n   Last\nret"),
            ["db 1,     Next,     Last", "", "", "ret"]
        );
        assert_eq!(split_label("Name:: db 1"), (Some("Name"), " db 1"));
        assert_eq!(split_label(".local: ret"), (None, ".local: ret"));
        let words: Vec<&str> = symbol_words("jp nz, .end_if_0").collect();
        assert_eq!(words, ["jp", "nz"]);
        let words: Vec<&str> = symbol_words("ld hl, Scope.local + $10 + 2").collect();
        assert_eq!(words, ["ld", "hl", "Scope"]);
    }

    #[test]
    fn test_local_labels_are_unique_and_shared_by_clones() {
        let labels = LabelAllocator::new();
        let clone = labels.clone();
        assert_eq!(labels.local("move"), ".move_0");
        assert_eq!(clone.local("move"), ".move_1");
        assert_eq!(labels.next_id(), 2);
        // A separate allocator has its own sequence
        assert_eq!(LabelAllocator::new().local("move"), ".move_0");
        // Several labels share one number
        assert_eq!(
            labels.locals(["end_if", "else", "then"]),
            [".end_if_3", ".else_3", ".then_3"]
        );
        // A fork goes on from the same number, without moving the original
        let fork = labels.fork();
        assert_eq!(fork.local("anim_end"), ".anim_end_4");
        assert_eq!(fork.local("anim_end"), ".anim_end_5");
        assert_eq!(labels.local("move"), ".move_4");
    }

    #[test]
    #[should_panic(expected = "invalid label stem \"check left\"")]
    fn test_a_label_stem_must_be_made_of_identifier_characters() {
        LabelAllocator::new().local("check left");
    }

    #[test]
    #[should_panic(expected = "label stem \"end\" given twice")]
    fn test_the_stems_of_one_number_are_distinct() {
        LabelAllocator::new().locals(["end", "end"]);
    }

    #[test]
    fn test_is_identifier() {
        for name in [
            "Coin",
            "_hidden",
            "Walk2",
            "player_left",
            "a#b",
            "x$y",
            "a@b",
        ] {
            assert!(is_identifier(name), "{} should be valid", name);
        }
        for name in [
            "",
            "2Walk",
            "my sprite",
            "Spin-Left",
            ".local",
            "a.b",
            "#raw",
            "é",
        ] {
            assert!(!is_identifier(name), "{:?} should be invalid", name);
        }
    }

    #[test]
    fn test_symbols_of_an_instruction() {
        use crate::asm::{Block, Condition, Expr, Mem, R8, R16};
        use std::collections::BTreeSet;

        let mut asm = Block::new();
        asm.label("Start")
            .label(".loop")
            .call("Func")
            .jp_cond(Condition::NZ, ".loop")
            .jr("Other.local")
            .jr_cond(Condition::Z, "@+4")
            .ld(R16::HL, Expr::sym("Table") + 2)
            .ld(R16::BC, Expr::sym("TilesEnd") - "Tiles")
            // Typed operands: an address either way, an ALU value, LOW / HIGH, raw text
            .ld(R8::A, Mem::addr("Source"))
            .ld(Mem::addr(Expr::sym("Dest") + 1), R8::A)
            .cp(Expr::low("LowByte") | Expr::high("HighByte"))
            .ld(R8::A, Expr::raw("BANK(Banked) ; NotAReference"))
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
        for instr in asm.into_instrs() {
            symbols(&instr, &mut refs, &mut defs);
        }
        for name in [
            "Func",
            "Other",
            "Table",
            "TilesEnd",
            "Tiles",
            "Source",
            "Dest",
            "LowByte",
            "HighByte",
            "Banked",
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
            "4",
        ] {
            assert!(!refs.iter().any(|r| r == name), "{} in {:?}", name, refs);
        }
        assert_eq!(
            defs.into_iter().collect::<Vec<_>>(),
            ["Blocked", "CONSTANT", "Raw", "Second", "Start"]
        );

        // Typed instructions refer to their symbols only, not to their mnemonic, registers or
        // functions (raw text has those words too: they never name a function)
        let mut typed = Block::new();
        typed
            .call("Func")
            .jr_cond(Condition::Z, "Main.loop")
            .ld(R16::HL, Expr::sym("Table") + 2)
            .ld(Mem::addr("Dest"), R8::A)
            .cp(Expr::low("LowByte") | Expr::high("HighByte"))
            .push(crate::asm::R16Stack::BC)
            .add_hl(R16::DE)
            .ld(R8::B, 5);
        let mut refs = Vec::new();
        for instr in typed.into_instrs() {
            symbols(&instr, &mut refs, &mut BTreeSet::new());
        }
        assert_eq!(
            refs,
            ["Func", "Main", "Table", "Dest", "LowByte", "HighByte"]
        );
    }
}
