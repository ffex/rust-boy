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

/// The code of one line of assembly, as RGBDS reads it: without its comment (from a `;`
/// outside a string) and with the contents of its strings (`"…"`, `'…'`) blanked, so
/// that neither is taken for code
pub(crate) fn code_of_line(line: &str) -> String {
    let mut code = String::with_capacity(line.len());
    let mut quote = None;
    let mut escaped = false;
    for c in line.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                    code.push(c);
                    continue;
                }
                code.push(' ');
            }
            None => match c {
                ';' => break,
                '"' | '\'' => {
                    quote = Some(c);
                    code.push(c);
                }
                _ => code.push(c),
            },
        }
    }
    code
}

/// The global label a line of code (from [`code_of_line`]) starts with (`Name:`,
/// `Name::`, `Name: db 1`), and the rest of the line
pub(crate) fn split_label(code: &str) -> (Option<&str>, &str) {
    match code.split_once(':') {
        Some((label, rest)) if is_identifier(label.trim()) => {
            (Some(label.trim()), rest.trim_start_matches(':'))
        }
        _ => (None, code),
    }
}

/// The global symbols a piece of code (from [`code_of_line`]) names: each word made of
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

    #[test]
    fn test_code_of_line() {
        assert_eq!(code_of_line("ld a, 1 ; one"), "ld a, 1 ");
        assert_eq!(
            code_of_line("db \"a;b\", LOW(Delay)"),
            "db \"   \", LOW(Delay)"
        );
        assert_eq!(
            code_of_line("db 'x', \"say \\\"hi\\\"\" ; c"),
            "db ' ', \"          \" "
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
