//! Labels for generated code: unique local labels, and the RGBDS rules for names.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Hands out unique local labels for code that can be generated more than once
///
/// A snippet such as a key check or a limited move can appear several times in one
/// program, and inside an `If` body, so the labels it jumps to:
/// - are **local** (`.name`): a global label starts a new RGBDS label scope, and an
///   enclosing `If` would then not find its own `.end_if_N`;
/// - are **numbered** by an allocator: two copies of a snippet never define the same
///   label.
///
/// Clones share one counter, so all the generators that hold a clone of a program's
/// allocator draw from the same sequence. The numbers only depend on the order of the
/// calls, so the same program always gets the same labels.
///
/// # Example
/// ```
/// use rust_boy::gb_asm::LabelAllocator;
///
/// let labels = LabelAllocator::new();
/// let shared = labels.clone();
/// assert_eq!(labels.local("check_left"), ".check_left_0");
/// assert_eq!(shared.local("check_left"), ".check_left_1");
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
    /// A snippet that needs several labels takes one and adds a suffix for the others
    /// (`.check_left_3`, `.check_left_3_end`). `stem` must be made of letters, digits
    /// and `_`.
    pub fn local(&self, stem: &str) -> String {
        format!(".{}_{}", stem, self.next_id())
    }
}

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

/// The global symbols a piece of code (from [`code_lines`]) names: each word made of
/// symbol characters that is an identifier. `Scope.local` names `Scope`; a local label
/// (`.name`) and a number (`$FF`, `10`) name none. Mnemonics and registers are words too.
pub(crate) fn symbol_words(code: &str) -> impl Iterator<Item = &str> {
    code.split(|c: char| !(c.is_ascii_alphanumeric() || "_#$@.".contains(c)))
        .map(|word| word.split('.').next().unwrap_or_default())
        .filter(|word| is_identifier(word))
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
}
