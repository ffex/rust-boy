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
//!
//! [`jr_range_errors`] also checks that each `jr` reaches its target, which rgbasm
//! requires: it works on instructions, whose sizes it knows.

use std::collections::{BTreeMap, BTreeSet};

use super::{Asm, Instr, JumpTarget, Operand, Register};

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

/// Size in bytes of `instr` once assembled; panics on what it does not know
fn instr_size(instr: &Instr) -> usize {
    use Operand::{Addr, AddrDef, AddrReg, AddrRegInc, Imm, Imm16, Label, Reg};
    let wide = |reg: &Register| {
        matches!(
            reg,
            Register::BC | Register::DE | Register::HL | Register::SP | Register::AF
        )
    };
    // `op a, src` and `op src` (and, cp): register or [hl] 1 byte, immediate 2
    let alu = |src: &Operand| match src {
        Reg(_) | AddrReg(Register::HL) => 1,
        Imm(_) | Label(_) => 2,
        other => panic!("jr_range_errors: unknown size of an ALU operand {}", other),
    };
    match instr {
        Instr::Label { .. } | Instr::Comment { .. } | Instr::Def { .. } => 0,
        Instr::Ld { dst, src } => match (dst, src) {
            (Reg(r), Imm16(_) | Label(_)) if wide(r) => 3,
            (Reg(_), Reg(_)) => 1,
            (Reg(_), Imm(_) | Label(_)) => 2,
            (Reg(Register::A), Addr(_) | AddrDef(_)) | (Addr(_) | AddrDef(_), Reg(Register::A)) => {
                3
            }
            (Reg(_), AddrReg(_) | AddrRegInc(_)) | (AddrReg(_) | AddrRegInc(_), Reg(_)) => 1,
            (AddrReg(Register::HL), Imm(_) | Label(_)) => 2,
            (dst, src) => panic!("jr_range_errors: unknown size of ld {}, {}", dst, src),
        },
        Instr::Ldh { .. } => 2,
        Instr::Add { dst: Reg(r), .. } if wide(r) => 1,
        Instr::Add { src, .. }
        | Instr::Adc { src, .. }
        | Instr::Sub { src, .. }
        | Instr::Or { src, .. }
        | Instr::Xor { src, .. } => alu(src),
        Instr::AdcA { operand } | Instr::And { operand } | Instr::Cp { operand } => alu(operand),
        Instr::Inc { .. } | Instr::Dec { .. } | Instr::Daa | Instr::Ret | Instr::RetCond { .. } => {
            1
        }
        Instr::Srl { .. } | Instr::Swap { .. } => 2,
        Instr::Jr { .. } | Instr::JrCond { .. } => 2,
        Instr::Jp { .. } | Instr::JpCond { .. } | Instr::Call { .. } => 3,
        other => panic!("jr_range_errors: unknown size of {}", other),
    }
}

/// Every `jr` in `code` whose target, a label in `code`, is out of its reach: the
/// offset from the end of the `jr` must be in -128..=127 (rgbasm rejects the others).
/// Labels are matched by name; `label_errors` checks their scopes.
pub(crate) fn jr_range_errors(code: &[Instr]) -> Vec<String> {
    let mut addresses = Vec::with_capacity(code.len());
    let mut labels = BTreeMap::new();
    let mut address = 0;
    for instr in code {
        addresses.push(address);
        if let Instr::Label { name } = instr {
            labels.insert(name.as_str(), address);
        }
        address += instr_size(instr);
    }

    let mut errors = Vec::new();
    for (instr, address) in code.iter().zip(addresses) {
        if let Instr::Jr { target } | Instr::JrCond { target, .. } = instr {
            let JumpTarget::Label(name) = target else {
                continue;
            };
            let Some(&to) = labels.get(name.as_str()) else {
                errors.push(format!("{}: target not found", instr));
                continue;
            };
            let offset = to as isize - (address as isize + 2);
            if !(-128..=127).contains(&offset) {
                errors.push(format!("{}: offset {} is out of range", instr, offset));
            }
        }
    }
    errors
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

    /// `jr` to a label `gap` bytes after it (`nop`-sized `inc a`s in between), or
    /// before it when `gap` is negative
    fn jr_over(gap: isize) -> Vec<Instr> {
        let filler = || Instr::Inc {
            operand: Operand::Reg(Register::A),
        };
        let jr = Instr::Jr {
            target: JumpTarget::Label(".target".to_string()),
        };
        let label = Instr::Label {
            name: ".target".to_string(),
        };
        let mut code = Vec::new();
        if gap >= 0 {
            code.push(jr);
            code.extend((0..gap).map(|_| filler()));
            code.push(label);
        } else {
            // the offset counts from the end of the jr, which is 2 bytes long
            code.push(label);
            code.extend((0..(-gap - 2)).map(|_| filler()));
            code.push(jr);
        }
        code
    }

    #[test]
    fn test_jr_range_limits() {
        // The limits RGBDS 1.0.4 applies to the same code, checked by hand: rgbasm
        // rejects -129, rgblink 128 (a forward target is resolved when linking)
        assert_eq!(jr_range_errors(&jr_over(127)), Vec::<String>::new());
        assert_eq!(
            jr_range_errors(&jr_over(128)),
            vec!["jr .target: offset 128 is out of range"]
        );
        assert_eq!(jr_range_errors(&jr_over(-128)), Vec::<String>::new());
        assert_eq!(
            jr_range_errors(&jr_over(-129)),
            vec!["jr .target: offset -129 is out of range"]
        );
    }

    #[test]
    fn test_jr_range_counts_instruction_sizes() {
        // ld a, [n16] 3 + cp n8 2 + call 3 + jp 3 + ld a, n8 2 = 13 bytes
        let mut asm = Asm::new();
        asm.jr_cond(Condition::Z, ".end")
            .ld_a_addr_def("wCount")
            .cp_imm(1)
            .call("Helper")
            .jp("Main")
            .ld_a(0)
            .label(".end");
        let code = asm.get_main_instrs();
        let sizes: usize = code.iter().map(instr_size).sum();
        assert_eq!(sizes, 2 + 13);
        assert_eq!(jr_range_errors(&code), Vec::<String>::new());
    }
}
