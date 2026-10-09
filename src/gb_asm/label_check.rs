//! A check of the labels in generated assembly, for unit tests.
//!
//! It reads the assembly text and follows the RGBDS label rules, so a test can catch what
//! rgbasm or rgblink would reject without running them:
//! - a global label (no dot) starts a new label scope, which lasts until the next global
//!   label or `SECTION`;
//! - a local label (`.name`) belongs to the current scope, so its full name is
//!   `Scope.name`; a reference to `.name` is looked up in the scope where it appears.
//!
//! [`label_errors`] checks the labels: each defined once, and the targets of `jp`, `jr`
//! and `call` defined, local ones in their scope.
//!
//! [`undefined_symbols`] checks every other global symbol the code uses (variables,
//! constants, `ld hl, Name`, `dw Name`, …): each must be defined in the program (a
//! label, `name: db`, `DEF`) or by `hardware.inc`. Comments (`;`, `/* … */`) and strings
//! are skipped. [`assert_links`] runs both checks, and with `RGBDS_LINK_CHECK` set also
//! assembles and links the program with rgbasm and rgblink.
//!
//! [`jr_range_errors`] also checks that each `jr` reaches its target, which rgbasm
//! requires: it works on instructions, whose sizes it knows ([`instr_size`]).
//!
//! [`rgbds_rom`] returns the ROM bytes RGBDS makes of a program, so a test can compare
//! the encoding of the instructions with the SM83 opcode table (`gb_asm::isa_tests`).

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

/// Words of an operand that are not symbols, compared without case: registers,
/// conditions, and every function and section keyword of the RGBDS 1.0.4 lexer
/// (`src/asm/lexer.cpp`, the `OP_*` and section tokens)
const KEYWORDS: &[&str] = &[
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

/// Every global symbol that `asm` uses but neither defines (a label, `name: db`, `DEF`)
/// nor gets from `hardware.inc`: what rgbasm or rgblink would report as undefined (the
/// targets of `jp` / `jr` / `call`, variables, constants, `ld hl, Name`, `dw Name`, …).
/// Comments and strings are skipped; local labels are left to [`label_errors`].
pub(crate) fn undefined_symbols(asm: &str) -> Vec<String> {
    use super::labels::{code_lines, split_def, split_label, symbol_words};

    let hardware_code = code_lines(include_str!("../../include/hardware.inc"));
    let hardware: BTreeSet<&str> = hardware_code
        .iter()
        .filter_map(|code| split_def(code))
        .map(|(name, _)| name)
        .collect();
    let mut defined = BTreeSet::new();
    let mut used = Vec::new();
    for (index, code) in code_lines(asm).iter().enumerate() {
        let (label, rest) = split_label(code);
        if let Some(label) = label {
            defined.insert(label.to_string());
        }
        // Skip a local label definition (`.loop:`), then split the mnemonic or directive
        // from its operands
        let mut rest = rest.trim();
        if rest.starts_with('.') {
            if let Some((_, after)) = rest.split_once(':') {
                rest = after.trim_start_matches(':').trim();
            }
        }
        // `DEF NAME EQU value`, in every form
        if let Some((name, value)) = split_def(rest) {
            defined.insert(name.to_string());
            used.extend(
                symbol_words(value)
                    .filter(|word| !KEYWORDS.contains(&word.to_ascii_lowercase().as_str()))
                    .map(|word| (index + 1, word.to_string())),
            );
            continue;
        }
        let (first, operands) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        if matches!(
            first.to_ascii_uppercase().as_str(),
            "" | "SECTION" | "INCLUDE" | "INCBIN"
        ) {
            continue;
        }
        for word in symbol_words(operands) {
            if !KEYWORDS.contains(&word.to_ascii_lowercase().as_str()) {
                used.push((index + 1, word.to_string()));
            }
        }
    }
    used.into_iter()
        .filter(|(_, word)| !defined.contains(word) && !hardware.contains(word.as_str()))
        .map(|(number, word)| format!("line {}: {} is not defined", number, word))
        .collect()
}

/// Panics, with the assembly, unless `asm` would assemble and link as far as its symbols
/// go: no [`label_errors`] and no [`undefined_symbols`]
///
/// With the environment variable `RGBDS_LINK_CHECK` set (and `rgbasm` / `rgblink` on the
/// `PATH`), it also assembles and links `asm` with RGBDS, `include/` on the include path.
pub(crate) fn assert_links(asm: &str) {
    assert_links_with(asm, &[]);
}

/// [`assert_links`] for a program that `INCLUDE`s other files: `files` are their
/// (name, text). The checks read them after the program; RGBDS gets them next to it.
pub(crate) fn assert_links_with(asm: &str, files: &[(&str, &str)]) {
    let mut all = asm.to_string();
    for (_, text) in files {
        all.push('\n');
        all.push_str(text);
    }
    let mut errors = label_errors(&all);
    errors.extend(undefined_symbols(&all));
    assert!(
        errors.is_empty(),
        "link errors:\n{}\n\nin:\n{}",
        errors.join("\n"),
        all
    );
    if std::env::var_os("RGBDS_LINK_CHECK").is_some() {
        rgbds_link(asm, files, &[]);
    }
}

/// The ROM that RGBDS makes of `asm`, assembled with every rgbasm warning turned into an
/// error (`-Weverything -Werror`: a truncated operand fails too) and linked:
/// `None` unless the environment variable `RGBDS_LINK_CHECK` is set (with `rgbasm` and
/// `rgblink` on the `PATH`). Panics with the RGBDS errors if it fails.
pub(crate) fn rgbds_rom(asm: &str) -> Option<Vec<u8>> {
    std::env::var_os("RGBDS_LINK_CHECK").map(|_| rgbds_link(asm, &[], &["-Weverything", "-Werror"]))
}

/// Assemble `asm` (`rgbasm_flags` added), with `files` next to it, and link it, with
/// RGBDS in a new temporary directory, and return the ROM; panics with the RGBDS errors
/// if it fails
fn rgbds_link(asm: &str, files: &[(&str, &str)], rgbasm_flags: &[&str]) -> Vec<u8> {
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rust-boy-link-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("cannot create a temporary directory");
    std::fs::write(dir.join("main.asm"), asm).expect("cannot write main.asm");
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("cannot write an included file");
    }
    let include = concat!(env!("CARGO_MANIFEST_DIR"), "/include");
    let mut rgbasm = rgbasm_flags.to_vec();
    rgbasm.extend(["-I", include, "-o", "main.o", "main.asm"]);
    let steps: [(&str, Vec<&str>); 2] = [
        ("rgbasm", rgbasm),
        ("rgblink", vec!["-o", "main.gb", "main.o"]),
    ];
    for (tool, args) in steps {
        let output = Command::new(tool)
            .args(&args)
            .current_dir(&dir)
            .output()
            .unwrap_or_else(|error| panic!("cannot run {}: {}", tool, error));
        assert!(
            output.status.success(),
            "{} failed:\n{}\n\nin:\n{}",
            tool,
            String::from_utf8_lossy(&output.stderr),
            asm
        );
    }
    let rom = std::fs::read(dir.join("main.gb")).expect("cannot read main.gb");
    let _ = std::fs::remove_dir_all(&dir);
    rom
}

/// [`assert_labels_ok`] for a piece of generated code, placed after a global label as
/// it is in a program
pub(crate) fn assert_code_labels_ok(code: &[Instr]) {
    let mut asm = Asm::new();
    asm.label("Main").emit_all(code.to_vec());
    assert_labels_ok(&asm.to_asm());
}

/// Size in bytes of `instr` once assembled; panics on what it does not know
pub(crate) fn instr_size(instr: &Instr) -> usize {
    use Operand::{Addr, AddrDef, AddrReg, AddrRegDec, AddrRegInc, Imm, Imm16, Label, Reg};
    let wide = |reg: &Register| {
        matches!(
            reg,
            Register::BC | Register::DE | Register::HL | Register::SP | Register::AF
        )
    };
    // `op a, src`: register or [hl] 1 byte, immediate 2
    let alu = |src: &Operand| match src {
        Reg(_) | AddrReg(Register::HL) => 1,
        Imm(_) | Label(_) => 2,
        other => panic!("jr_range_errors: unknown size of an ALU operand {}", other),
    };
    match instr {
        Instr::Label { .. } | Instr::Comment { .. } | Instr::Def { .. } => 0,
        Instr::Ld { dst, src } => match (dst, src) {
            (Reg(r), Imm(_) | Imm16(_) | Label(_)) if wide(r) => 3,
            (Reg(_), Reg(_)) => 1,
            (Reg(_), Imm(_) | Label(_)) => 2,
            (Reg(Register::A), Addr(_) | AddrDef(_))
            | (Addr(_) | AddrDef(_), Reg(Register::A | Register::SP)) => 3,
            (Reg(_), AddrReg(_) | AddrRegInc(_) | AddrRegDec(_))
            | (AddrReg(_) | AddrRegInc(_) | AddrRegDec(_), Reg(_)) => 1,
            (AddrReg(Register::HL), Imm(_) | Label(_)) => 2,
            (dst, src) => panic!("jr_range_errors: unknown size of ld {}, {}", dst, src),
        },
        // `ldh a, [c]` / `ldh [c], a` 1 byte, `ldh a, [n8]` / `ldh [n8], a` 2
        Instr::Ldh {
            dst: AddrReg(Register::C),
            ..
        }
        | Instr::Ldh {
            src: AddrReg(Register::C),
            ..
        } => 1,
        Instr::Ldh { .. } => 2,
        Instr::Add { src }
        | Instr::Adc { src }
        | Instr::Sub { src }
        | Instr::Sbc { src }
        | Instr::And { src }
        | Instr::Xor { src }
        | Instr::Or { src }
        | Instr::Cp { src } => alu(src),
        Instr::Inc { .. }
        | Instr::Dec { .. }
        | Instr::AddHl { .. }
        | Instr::Push { .. }
        | Instr::Pop { .. }
        | Instr::Rlca
        | Instr::Rrca
        | Instr::Rla
        | Instr::Rra
        | Instr::Daa
        | Instr::Cpl
        | Instr::Scf
        | Instr::Ccf
        | Instr::Nop
        | Instr::Halt
        | Instr::Di
        | Instr::Ei
        | Instr::JpHl
        | Instr::Ret
        | Instr::RetCond { .. }
        | Instr::Reti
        | Instr::Rst { .. } => 1,
        // rgbasm follows `stop` with a $00 byte
        Instr::Stop | Instr::AddSp { .. } | Instr::LdHlSp { .. } => 2,
        // The `$CB`-prefixed instructions
        Instr::Rlc { .. }
        | Instr::Rrc { .. }
        | Instr::Rl { .. }
        | Instr::Rr { .. }
        | Instr::Sla { .. }
        | Instr::Sra { .. }
        | Instr::Swap { .. }
        | Instr::Srl { .. }
        | Instr::Bit { .. }
        | Instr::Set { .. }
        | Instr::Res { .. } => 2,
        Instr::Jr { .. } | Instr::JrCond { .. } => 2,
        Instr::Jp { .. } | Instr::JpCond { .. } | Instr::Call { .. } | Instr::CallCond { .. } => 3,
        other => panic!("jr_range_errors: unknown size of {}", other),
    }
}

/// Every `jr` in `code` whose target, a label in `code`, is out of its reach: the
/// offset from the end of the `jr` must be in -128..=127 (rgbasm rejects the others).
/// A local label (`.name`) belongs to the last global label before it, as in RGBDS, so
/// the same local name in two routines is two labels.
pub(crate) fn jr_range_errors(code: &[Instr]) -> Vec<String> {
    // Full name of a label used under the global label `scope` ("" before the first)
    let full_name = |scope: &str, name: &str| {
        if name.starts_with('.') {
            format!("{}{}", scope, name)
        } else {
            name.to_string()
        }
    };
    // Address and scope of each instruction, and every label by its full name
    let mut places = Vec::with_capacity(code.len());
    let mut labels = BTreeMap::new();
    let mut scope = "";
    let mut address = 0;
    for instr in code {
        if let Instr::Label { name } = instr {
            if !name.starts_with('.') {
                scope = name;
            }
            labels.insert(full_name(scope, name), address);
        }
        places.push((address, scope));
        address += instr_size(instr);
    }

    let mut errors = Vec::new();
    for (instr, (address, scope)) in code.iter().zip(places) {
        if let Instr::Jr { target } | Instr::JrCond { target, .. } = instr {
            let JumpTarget::Label(name) = target else {
                continue;
            };
            let Some(&to) = labels.get(&full_name(scope, name)) else {
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
    use crate::gb_asm::{Condition, R16};

    #[test]
    fn test_undefined_symbols() {
        let asm = "
            INCLUDE \"hardware.inc\"
            DEF LIMIT EQU 10 + OTHER
            SECTION \"Code\", ROM0
            Main:
            .loop: ld a, [wCount] ; call NotUsed
            ld [rLCDC], a
            cp LIMIT
            jr nz, .loop
            call Helper
            ld hl, Table + 2
            db \"call Quoted\", LOW(Main)
            ld a, [hli]
            dw MUL(3.0, 2.0), STRLEN(\"abc\"), SIZEOF(\"Code\"), BANK(Main), high(Main)
            jp Main
            SECTION \"Variables\", WRAM0
            wCount: db
        ";
        assert_eq!(
            undefined_symbols(asm),
            [
                "line 3: OTHER is not defined",
                "line 10: Helper is not defined",
                "line 11: Table is not defined",
            ]
        );
    }

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

    #[test]
    fn test_jr_range_resolves_local_labels_in_their_scope() {
        // Two routines with their own `.end`: each `jr .end` reaches the `.end` of its
        // routine, 2 bytes ahead, not the other one (which would be out of range)
        let mut asm = Asm::new();
        asm.label("First").jr(".end").ld_a(1).label(".end").ret();
        for _ in 0..70 {
            asm.ld_a(0); // 140 bytes
        }
        asm.label("Second").jr(".end").ld_a(2).label(".end").ret();
        assert_eq!(
            jr_range_errors(&asm.get_main_instrs()),
            Vec::<String>::new()
        );

        // A local label of another scope is not found
        let mut asm = Asm::new();
        asm.label("First").label(".end").label("Second").jr(".end");
        assert_eq!(
            jr_range_errors(&asm.get_main_instrs()),
            vec!["jr .end: target not found"]
        );
    }

    #[test]
    fn test_instr_size_of_ldh_and_add() {
        let a = || Operand::Reg(Register::A);
        let c = || Operand::AddrReg(Register::C);
        let ldh = |dst, src| Instr::Ldh { dst, src };
        // ldh a, [c] and ldh [c], a: 1 byte; ldh with an 8-bit address: 2
        assert_eq!(instr_size(&ldh(a(), c())), 1);
        assert_eq!(instr_size(&ldh(c(), a())), 1);
        assert_eq!(
            instr_size(&ldh(a(), Operand::AddrDef("rLY".to_string()))),
            2
        );
        // add sp, e8: 2 bytes; add hl, r16: 1; add a, n8: 2
        // (every instruction family is checked against rgbasm in `gb_asm::isa_tests`)
        assert_eq!(instr_size(&Instr::AddSp { offset: 4 }), 2);
        assert_eq!(instr_size(&Instr::AddHl { src: R16::DE }), 1);
        assert_eq!(
            instr_size(&Instr::Add {
                src: Operand::Imm(4)
            }),
            2
        );
    }
}
