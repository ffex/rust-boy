//! A check of the labels in generated assembly, for unit tests.
//!
//! It reads the assembly text and follows the RGBDS label rules, so a test can catch what
//! rgbasm or rgblink would reject without running them:
//! - a global label (no dot) starts a new label scope, which lasts until the next global
//!   label or `SECTION`;
//! - a local label (`.name`) belongs to the current scope, so its full name is
//!   `Scope.name`; a reference to `.name` is looked up in the scope where it appears.
//!
//! Only the targets of `jp`, `jr` and `call` are checked as references.

use std::collections::BTreeSet;

use super::{Asm, Instr};

/// Every label problem in `asm`: a label defined twice, a local label outside any scope,
/// or a `jp` / `jr` / `call` whose target is not defined (in its own scope, for a local
/// target)
pub(crate) fn label_errors(asm: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let mut defined = BTreeSet::new();
    // (line, target as written, full name or None for a local target outside any scope)
    let mut references = Vec::new();
    let mut scope: Option<String> = None;

    for (index, line) in asm.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        let Some(first) = line.split_whitespace().next() else {
            continue;
        };
        if first.starts_with(';') {
            continue;
        }
        if first.eq_ignore_ascii_case("SECTION") {
            scope = None;
            continue;
        }

        // A label definition: `Name:`, `.name:` or `name: db`
        if let Some(name) = first.strip_suffix(':') {
            let name = name.trim_end_matches(':');
            let full = match name.strip_prefix('.') {
                Some(local) => match &scope {
                    Some(scope) => format!("{}.{}", scope, local),
                    None => {
                        errors.push(format!(
                            "line {}: local label {} outside any global label",
                            number, name
                        ));
                        continue;
                    }
                },
                None => name.to_string(),
            };
            if !name.contains('.') {
                scope = Some(name.to_string());
            }
            if !defined.insert(full.clone()) {
                errors.push(format!("line {}: label {} defined twice", number, full));
            }
            continue;
        }

        if ["jp", "jr", "call"].contains(&first.to_ascii_lowercase().as_str()) {
            // `jp target` or `jp cond, target`
            let target = line[first.len()..]
                .rsplit(',')
                .next()
                .unwrap_or_default()
                .trim();
            if target.starts_with('$') || target.eq_ignore_ascii_case("hl") {
                continue;
            }
            let full = match target.strip_prefix('.') {
                Some(local) => scope.as_ref().map(|scope| format!("{}.{}", scope, local)),
                None => Some(target.to_string()),
            };
            references.push((number, target.to_string(), full));
        }
    }

    for (number, target, full) in references {
        match full {
            None => errors.push(format!(
                "line {}: jump to {} outside any global label",
                number, target
            )),
            Some(full) if !defined.contains(&full) => errors.push(format!(
                "line {}: jump to {}, but {} is not defined",
                number, target, full
            )),
            Some(_) => {}
        }
    }
    errors
}

/// Panics, with the assembly, if [`label_errors`] finds any problem in `asm`
pub(crate) fn assert_labels_ok(asm: &str) {
    let errors = label_errors(asm);
    assert!(
        errors.is_empty(),
        "label errors:\n{}\n\nin:\n{}",
        errors.join("\n"),
        asm
    );
}

/// [`assert_labels_ok`] for a piece of generated code, placed after a global label as
/// it is in a program
pub(crate) fn assert_code_labels_ok(code: &[Instr]) {
    let mut asm = Asm::new();
    asm.label("Main").emit_all(code.to_vec());
    assert_labels_ok(&asm.to_asm());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::Condition;

    #[test]
    fn test_accepts_local_labels_in_their_scope() {
        let asm = "
            SECTION \"Code\", ROM0
            Main:
            jp nz, .end_if_0
            call Helper
            .end_if_0:
            jr Main
            Helper:
            .loop:
            jr nz, .loop
            ret
            SECTION \"Variables\", WRAM0
            wCount: db
        ";
        assert_eq!(label_errors(asm), Vec::<String>::new());
    }

    #[test]
    fn test_finds_duplicate_labels() {
        let asm = "Main:\n.a:\n.a:\nOther:\n.a:\nMain:\n";
        assert_eq!(
            label_errors(asm),
            vec![
                "line 3: label Main.a defined twice",
                "line 6: label Main defined twice"
            ]
        );
    }

    #[test]
    fn test_finds_a_global_label_that_breaks_an_if() {
        // The jump looks for Main.end_if_0, but the label is defined as CheckLeft.end_if_0
        let asm = "Main:\njp nz, .end_if_0\nCheckLeft:\n.end_if_0:\n";
        assert_eq!(
            label_errors(asm),
            vec!["line 2: jump to .end_if_0, but Main.end_if_0 is not defined"]
        );
    }

    #[test]
    fn test_code_is_checked_after_a_global_label() {
        let mut asm = Asm::new();
        asm.jp_cond(Condition::NZ, ".skip").ld_a(1).label(".skip");
        assert_code_labels_ok(&asm.get_main_instrs());
    }

    #[test]
    #[should_panic(expected = "label Main.skip defined twice")]
    fn test_code_with_a_duplicate_label_fails() {
        let mut asm = Asm::new();
        asm.label(".skip").label(".skip");
        assert_code_labels_ok(&asm.get_main_instrs());
    }

    #[test]
    fn test_finds_undefined_and_unscoped_labels() {
        let asm = ".early:\njp .early\nMain:\ncall Missing\n";
        assert_eq!(
            label_errors(asm),
            vec![
                "line 1: local label .early outside any global label",
                "line 2: jump to .early outside any global label",
                "line 4: jump to Missing, but Missing is not defined"
            ]
        );
    }
}
