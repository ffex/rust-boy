//! Jump relaxation: each `jr` that cannot be shown to reach its target becomes a `jp`.
//!
//! A `jr` (2 bytes) reaches -128 to 127 bytes from its end; a `jp` (3 bytes) reaches
//! anywhere. Code generators write `jr` where they expect a short jump, and
//! [`Asm::to_asm`](super::Asm::to_asm) calls [`relax_jumps`] on the whole program, once,
//! so a `jr` that turns out to be too far assembles anyway.
//!
//! The rules, for each `jr` / `jr cc` of the program:
//! - its target must be a label the program defines once with [`Instr::Label`], found by
//!   the RGBDS scope rules (a local `.name` belongs to the last global label before it;
//!   a `SECTION` ends the scope), or an offset from the jump itself (`@`, `@+n`, `@-n`, see
//!   below);
//! - the target must be in the same section, with only instructions of known size
//!   ([`Instr::size`]) between them, and the offset from the end of the `jr` must be in
//!   -128..=127.
//!
//! Every other `jr` becomes a `jp` with the same condition and target: a target out of
//! range, in another section, unknown to the program (an external symbol, a label of an
//! `INCLUDE`d file or of a raw line, an anonymous label `:+`), defined twice, an absolute
//! address, or a jump over a `ds $150 - @`, a `db` with a string, an `INCBIN` without a
//! length, an `INCLUDE` or a raw line with code (their size is known only to RGBDS). Each
//! of these targets means the same place after the jump grows: a label moves with the code,
//! an address does not move. A `jp` is never made a `jr`: what the code says `jp` stays
//! `jp`.
//!
//! **Targets written from `@`.** `@` is the address of the jump itself, so `jr nz, @+4`
//! means "4 bytes from the start of this jump": a jump that grows by one byte, or another
//! jump that grows between the two, would move that target off the instruction it meant.
//! A target `@`, `@+n` or `@-n` (`n` a number) of any jump (`jr`, `jp`, `call`) is
//! therefore measured like a label: it is the instruction `n` bytes from the jump, and the
//! offset is written again for the relaxed code (`jr nz, @+4` that grows over an `ld a, 1`
//! is printed `jp nz, @+5`; it is left as written when nothing in between grows). If the
//! instruction cannot be found (a size only RGBDS knows in between, another section, an
//! offset inside an instruction), or a jump's target is any other expression (`Label + 2`,
//! `@ * 2`), no jump of the program is changed: the program is printed as written, and
//! rgbasm reports a `jr` out of range as before, loudly. The same holds when `@` appears
//! anywhere else in the code (a raw line with code, `db` / `dw`, an operand such as
//! `ld hl, @ + 5`, a `DEF`): such an offset cannot be written again, and would move if a
//! `jr` in its span grew. The one exception is the padding `ds N - @` (`N` a number), which
//! fills up to the address `N` and so keeps its meaning. `@` in a comment, in a string or
//! inside a name (`a@b`) is not one.
//!
//! **Data.** A `dw` whose items are plain numbers, or labels the program defines once (an
//! address is 2 bytes, and such a name cannot be an `EQUS`), has a known size; any other
//! symbol in data could be an `EQUS` of several values, so its size is unknown.
//!
//! The relaxation is iterative: a `jr` that grows to a `jp` takes one more byte, which can
//! push another `jr` over the same bytes out of range. It starts with every `jr` short and
//! grows the ones out of reach until none is; offsets only grow when a jump grows, so this
//! ends (at most once per `jr`) on the fewest `jp`s. A `jp` takes one more cycle than a `jr`
//! when it is taken (4 M-cycles, `jr` 3), and one more byte: code whose timing or size is
//! fixed (an `rst` vector, a cycle-counted loop) should write its jumps so they reach.

use std::collections::BTreeMap;

use super::expr::parse_number;
use super::instr::{Instr, JumpTarget};
use super::labels::{code_lines, is_identifier};

/// What a jump's target is, for the relaxation
#[derive(Debug, Clone, PartialEq)]
enum Target {
    /// A label name, plain (`Name`, `.local`, `Scope.local`) or anonymous (`:+`, `:--`): it
    /// moves with the code it names
    Name(String),
    /// `@`, `@+n`, `@-n`: `n` bytes from the start of the jump
    Here(i64),
    /// An address (a number): it does not move
    Fixed,
    /// Any other expression
    Expression,
}

/// The target of a jump (`jr`, `jp`, `call`, with or without a condition), if `instr` is one
fn jump_target(instr: &Instr) -> Option<&JumpTarget> {
    match instr {
        Instr::Jr { target }
        | Instr::JrCond { target, .. }
        | Instr::Jp { target }
        | Instr::JpCond { target, .. }
        | Instr::Call { target }
        | Instr::CallCond { target, .. } => Some(target),
        _ => None,
    }
}

/// What `target` is (see [`Target`])
fn classify(target: &JumpTarget) -> Target {
    let JumpTarget::Label(text) = target else {
        return Target::Fixed;
    };
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('@') {
        // Spaces around the operator only: `@ + 4`, not `@+ 1 0` (which rgbasm rejects)
        let rest = rest.trim_start();
        if rest.is_empty() {
            return Target::Here(0);
        }
        let (sign, digits) = if let Some(digits) = rest.strip_prefix('+') {
            (1, digits)
        } else if let Some(digits) = rest.strip_prefix('-') {
            (-1, digits)
        } else {
            return Target::Expression;
        };
        return match parse_number(digits.trim_start()) {
            Some((value, _)) if value >= 0 => Target::Here(sign * i64::from(value)),
            _ => Target::Expression,
        };
    }
    if parse_number(text).is_some() {
        return Target::Fixed;
    }
    let is_name = match text.split_once('.') {
        Some(("", local)) => is_identifier(local),
        Some((scope, local)) => is_identifier(scope) && is_identifier(local),
        None => is_identifier(text),
    };
    let anonymous = text.len() > 1
        && text.starts_with(':')
        && (text[1..].chars().all(|c| c == '+') || text[1..].chars().all(|c| c == '-'));
    if is_name || anonymous {
        Target::Name(text.to_string())
    } else {
        Target::Expression
    }
}

/// Whether a line of code (from [`code_lines`]: comments and strings removed) uses `@`, the
/// address of the current instruction: an `@` that does not continue a name (`a@b` is one)
fn uses_here(code: &str) -> bool {
    let chars: Vec<char> = code.chars().collect();
    chars.iter().enumerate().any(|(i, &c)| {
        c == '@'
            && (i == 0 || !(chars[i - 1].is_ascii_alphanumeric() || "_#$@.".contains(chars[i - 1])))
    })
}

/// Whether `instr` is the padding `ds N - @` (up to the address N): it keeps its meaning
/// when the code before it grows
fn is_padding(instr: &Instr) -> bool {
    let Instr::Ds { count, fill } = instr else {
        return false;
    };
    let padding = count
        .split_once('-')
        .is_some_and(|(end, here)| here.trim() == "@" && parse_number(end.trim()).is_some());
    padding
        && !fill
            .iter()
            .any(|fill| code_lines(fill).iter().any(|code| uses_here(code)))
}

/// Whether `program` uses `@` anywhere but in a jump target (which [`classify`] reads) or a
/// padding `ds N - @`: a raw line, a `db` / `dw`, an operand (`ld hl, @ + 5`). Such an
/// offset would move if a `jr` in its span grew, so the program is then left as written.
fn uses_here_elsewhere(program: &[Instr]) -> bool {
    program.iter().any(|instr| {
        if jump_target(instr).is_some() || is_padding(instr) || instr.check().is_err() {
            return false;
        }
        code_lines(&instr.to_string())
            .iter()
            .any(|code| uses_here(code))
    })
}

/// The index of the instruction `offset` bytes from the start of the jump at `jump`, as
/// the program is written: `None` if a size between them is unknown, a `SECTION` is in the
/// way, or no instruction starts there
fn here_index(
    program: &[Instr],
    sizes: &[Option<usize>],
    jump: usize,
    offset: i64,
) -> Option<usize> {
    let mut bytes = 0i64;
    let index = if offset >= 0 {
        let mut index = jump;
        while bytes < offset {
            bytes += i64::try_from(sizes.get(index).copied()??).ok()?;
            index += 1;
            if matches!(program.get(index), None | Some(Instr::Section { .. })) {
                return None;
            }
        }
        index
    } else {
        let mut index = jump;
        while bytes < -offset {
            index = index.checked_sub(1)?;
            if matches!(program[index], Instr::Section { .. }) {
                return None;
            }
            bytes += i64::try_from(sizes[index]?).ok()?;
        }
        index
    };
    (bytes == offset.abs()).then_some(index)
}

/// `@`, `@+n` or `@-n`
fn here_text(offset: i64) -> String {
    match offset {
        0 => "@".to_string(),
        n if n > 0 => format!("@+{}", n),
        n => format!("@-{}", -n),
    }
}

/// The full RGBDS name of a label written `name` under the global label `scope`
/// (`Scope.local` for a local label); `None` for a local label outside any scope
fn full_name(scope: Option<&str>, name: &str) -> Option<String> {
    if name.starts_with('.') {
        scope.map(|scope| format!("{}{}", scope, name))
    } else {
        Some(name.to_string())
    }
}

/// Whether `instr` is a `jr` or `jr cc`
fn is_jr(instr: &Instr) -> bool {
    matches!(instr, Instr::Jr { .. } | Instr::JrCond { .. })
}

/// `program` with every `jr` / `jr cc` that does not provably reach its target turned into
/// a `jp` / `jp cc`, and the `@` targets written again for the new sizes (see the
/// [module documentation](self) for the rules)
pub(crate) fn relax_jumps(program: &[Instr]) -> Vec<Instr> {
    let Some((long, places, here)) = relax(program) else {
        return program.to_vec();
    };
    program
        .iter()
        .enumerate()
        .map(|(index, instr)| {
            let mut instr = match instr {
                Instr::Jr { target } if long[index] => Instr::Jp {
                    target: target.clone(),
                },
                Instr::JrCond { condition, target } if long[index] => Instr::JpCond {
                    condition: condition.clone(),
                    target: target.clone(),
                },
                _ => instr.clone(),
            };
            // An `@` target: the offset of its instruction in the relaxed code
            if let Some((offset, target)) = here[index] {
                let new_offset = places[target].1 as i64 - places[index].1 as i64;
                if new_offset != offset {
                    let text = JumpTarget::Label(here_text(new_offset));
                    match &mut instr {
                        Instr::Jr { target }
                        | Instr::JrCond { target, .. }
                        | Instr::Jp { target }
                        | Instr::JpCond { target, .. }
                        | Instr::Call { target }
                        | Instr::CallCond { target, .. } => *target = text,
                        _ => unreachable!("only jumps have an @ target"),
                    }
                }
            }
            instr
        })
        .collect()
}

/// Which `jr` must become a `jp`, where each instruction is then ((block, offset), see
/// below), and for each jump with an `@` target, (its offset as written, the index of the
/// instruction it means); `None` if the program must be left as it is written
#[allow(clippy::type_complexity)]
fn relax(program: &[Instr]) -> Option<(Vec<bool>, Vec<(usize, usize)>, Vec<Option<(i64, usize)>>)> {
    if uses_here_elsewhere(program) {
        return None;
    }
    // The scope of each instruction, and the index of each label by its full name (`None`
    // when it is defined twice: rgbasm rejects it, and it is no target to measure)
    let mut scopes = Vec::with_capacity(program.len());
    let mut labels: BTreeMap<String, Option<usize>> = BTreeMap::new();
    let mut scope: Option<&str> = None;
    for (index, instr) in program.iter().enumerate() {
        match instr {
            Instr::Section { .. } => scope = None,
            Instr::Label { name } => {
                // `Name::` (exported) is the label `Name`
                let name = name.trim_end_matches(':');
                if !name.contains('.') {
                    scope = Some(name);
                }
                if let Some(full) = full_name(scope, name) {
                    labels
                        .entry(full)
                        .and_modify(|place| *place = None)
                        .or_insert(Some(index));
                }
            }
            _ => {}
        }
        scopes.push(scope);
    }

    // A `dw` of labels the program defines is 2 bytes each (such a name is no `EQUS`)
    let sizes: Vec<Option<usize>> = program
        .iter()
        .zip(&scopes)
        .map(|(instr, scope)| {
            instr.size().or_else(|| {
                instr.dw_size_with(|name| {
                    full_name(*scope, name)
                        .is_some_and(|full| matches!(labels.get(&full), Some(Some(_))))
                })
            })
        })
        .collect();
    // The `@` target of each jump, as (offset, instruction); `targets`: the instruction each
    // `jr` jumps to, if the program defines it once
    let mut here = vec![None; program.len()];
    let mut targets = vec![None; program.len()];
    for (index, instr) in program.iter().enumerate() {
        let Some(target) = jump_target(instr) else {
            continue;
        };
        match classify(target) {
            Target::Expression => return None,
            Target::Here(offset) => {
                let place = here_index(program, &sizes, index, offset)?;
                here[index] = Some((offset, place));
                targets[index] = Some(place);
            }
            Target::Name(name) => {
                targets[index] = full_name(scopes[index], &name)
                    .and_then(|full| labels.get(&full).copied().flatten());
            }
            Target::Fixed => {}
        }
    }

    // A `jr` without a known target is long from the start
    let mut long: Vec<bool> = program
        .iter()
        .zip(&targets)
        .map(|(instr, target)| is_jr(instr) && target.is_none())
        .collect();

    loop {
        // Where each instruction is: (block, offset in the block). A block is a run of
        // instructions of known size in one section: a `SECTION` starts a new one, and so
        // does the instruction after one of unknown size.
        let mut places = Vec::with_capacity(program.len());
        let (mut block, mut offset) = (0usize, 0usize);
        for (index, instr) in program.iter().enumerate() {
            if matches!(instr, Instr::Section { .. }) {
                block += 1;
                offset = 0;
            }
            places.push((block, offset));
            match if long[index] { Some(3) } else { sizes[index] } {
                Some(size) => offset += size,
                None => {
                    block += 1;
                    offset = 0;
                }
            }
        }

        let mut grown = false;
        for (index, instr) in program.iter().enumerate() {
            if long[index] || !is_jr(instr) {
                continue;
            }
            let Some(target) = targets[index] else {
                continue;
            };
            let (from_block, from) = places[index];
            let (to_block, to) = places[target];
            // The offset counts from the end of the `jr`, 2 bytes long
            let reaches = from_block == to_block
                && (-128..=127).contains(&(to as isize - (from as isize + 2)));
            if !reaches {
                long[index] = true;
                grown = true;
            }
        }
        if !grown {
            return Some((long, places, here));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::label_check::{jr_range_errors, rgbds_rom};
    use crate::gb_asm::{Asm, Chunk, Condition, R8, Section};

    /// A program in one ROM0 section at $0000, starting with the global label `Main`,
    /// then the code `build` writes
    fn one_section(build: impl FnOnce(&mut Asm)) -> Asm {
        let mut asm = Asm::new();
        asm.section(Section::rom0("Code").at(0x0000)).label("Main");
        build(&mut asm);
        asm
    }

    /// `count` one-byte instructions
    fn nops(asm: &mut Asm, count: usize) {
        for _ in 0..count {
            asm.nop();
        }
    }

    /// "jr" or "jp": what the one jump to `target` (as written) is in `program`
    fn jump_to(program: &[Instr], target: &str) -> &'static str {
        let label = JumpTarget::Label(target.to_string());
        let mut kinds = program.iter().filter_map(|instr| match instr {
            Instr::Jr { target } | Instr::JrCond { target, .. } if *target == label => Some("jr"),
            Instr::Jp { target } | Instr::JpCond { target, .. } if *target == label => Some("jp"),
            _ => None,
        });
        let kind = kinds.next().expect("no jump to the target");
        assert!(kinds.next().is_none(), "several jumps to {}", target);
        kind
    }

    /// Checks the relaxed `asm`: no `jr` out of range ([`jr_range_errors`]), and with
    /// RGBDS (when `RGBDS_LINK_CHECK` is set) it assembles and links with every warning an
    /// error, and each jump in the ROM is the opcode the relaxed program says, landing on
    /// the address of its label as the sizes of [`Instr::size`] place it. Every size must
    /// be known, and the first section must be at $0000.
    fn check_with_rgbds(asm: &Asm) {
        let program = asm.program();
        assert_eq!(jr_range_errors(&program), Vec::<String>::new());
        let Some(rom) = rgbds_rom(&asm.to_asm()) else {
            return;
        };
        // Address and scope of each instruction, and the address of each label
        let mut places = Vec::new();
        let mut labels = BTreeMap::new();
        let (mut address, mut scope) = (0usize, String::new());
        for (index, instr) in program.iter().enumerate() {
            if index > 0 && matches!(instr, Instr::Section { .. }) {
                break; // only the first section is checked
            }
            if let Instr::Label { name } = instr {
                if !name.starts_with('.') {
                    scope = name.clone();
                }
                labels.insert(full_name(Some(&scope), name).unwrap(), address);
            }
            places.push((address, scope.clone()));
            address += instr.size().expect("a known size");
        }
        let mut jumps = 0;
        for (instr, (address, scope)) in program.iter().zip(places) {
            let (opcodes, target) = match instr {
                Instr::Jr { target } | Instr::JrCond { target, .. } => {
                    ([0x18, 0x20, 0x28, 0x30, 0x38], target)
                }
                Instr::Jp { target } | Instr::JpCond { target, .. } => {
                    ([0xC3, 0xC2, 0xCA, 0xD2, 0xDA], target)
                }
                _ => continue,
            };
            let JumpTarget::Label(name) = target else {
                continue;
            };
            let Some(&to) = full_name(Some(&scope), name).and_then(|full| labels.get(&full)) else {
                continue; // not a label of the first section
            };
            assert!(
                opcodes.contains(&rom[address]),
                "{} at ${:04x}: opcode ${:02X}",
                instr,
                address,
                rom[address]
            );
            let reached = if is_jr(instr) {
                (address as isize + 2 + isize::from(rom[address + 1] as i8)) as usize
            } else {
                usize::from(u16::from_le_bytes([rom[address + 1], rom[address + 2]]))
            };
            assert_eq!(reached, to, "{} at ${:04x}", instr, address);
            jumps += 1;
        }
        assert!(jumps > 0, "no jump checked");
    }

    #[test]
    fn test_a_jr_reaches_127_bytes_ahead_and_128_back() {
        // Forward: the offset counts from the end of the jr, which is 2 bytes long
        for (gap, kind) in [(126, "jr"), (127, "jr"), (128, "jp"), (129, "jp")] {
            let asm = one_section(|asm| {
                asm.jr(".ahead");
                nops(asm, gap);
                asm.label(".ahead").ret();
            });
            assert_eq!(
                jump_to(&asm.program(), ".ahead"),
                kind,
                "{} bytes ahead",
                gap
            );
            check_with_rgbds(&asm);
        }
        // Backward: the label, then `back - 2` bytes, then the jr
        for (back, kind) in [(127, "jr"), (128, "jr"), (129, "jp"), (130, "jp")] {
            let asm = one_section(|asm| {
                asm.label(".back");
                nops(asm, back - 2);
                asm.jr_cond(Condition::NZ, ".back").ret();
            });
            let program = asm.program();
            assert_eq!(jump_to(&program, ".back"), kind, "{} bytes back", back);
            if kind == "jp" {
                // The condition is kept
                assert!(program.contains(&Instr::JpCond {
                    condition: Condition::NZ,
                    target: JumpTarget::Label(".back".to_string()),
                }));
            }
            check_with_rgbds(&asm);
        }
    }

    #[test]
    fn test_a_jr_that_grows_pushes_others_out_of_range() {
        // Forward: each jump is in range until the one inside its span grows. A single
        // pass would see only the first problem (the jump to .t3); the iteration finds
        // all three.
        //   Main:  jr .t1        $0000, 127 bytes to .t1
        //          jr z, .t2     $0002, 127 bytes to .t2
        //          125 nops
        //   .t1:   jr c, .t3     $0081, 130 bytes to .t3: grows, which pushes .t2 to 128
        //   .t2:   130 nops             bytes: that jump grows, and pushes .t1 to 128
        //   .t3:   jr .near      127 bytes, and nothing in between grows
        let asm = one_section(|asm| {
            asm.jr(".t1").jr_cond(Condition::Z, ".t2");
            nops(asm, 125);
            asm.label(".t1").jr_cond(Condition::C, ".t3").label(".t2");
            nops(asm, 130);
            asm.label(".t3").jr(".near");
            nops(asm, 127);
            asm.label(".near").ret();
        });
        // Before the relaxation, only the jump to .t3 is out of range
        let code = asm.get_main_instrs();
        assert_eq!(
            jr_range_errors(&code),
            ["jr c, .t3: offset 130 is out of range"]
        );
        let program = asm.program();
        for target in [".t1", ".t2", ".t3"] {
            assert_eq!(jump_to(&program, target), "jp", "{}", target);
        }
        assert_eq!(jump_to(&program, ".near"), "jr", "nothing pushes it");
        check_with_rgbds(&asm);

        // Backward: a jr 128 bytes back, with a jump in between that grows
        let asm = one_section(|asm| {
            asm.label(".back");
            nops(asm, 124);
            asm.jr(".far").jr(".back"); // 124 + 2 + 2 = 128 bytes back
            nops(asm, 128);
            asm.label(".far").ret();
        });
        let code = asm.get_main_instrs();
        assert_eq!(
            jr_range_errors(&code),
            ["jr .far: offset 130 is out of range"]
        );
        let program = asm.program();
        assert_eq!(jump_to(&program, ".far"), "jp");
        assert_eq!(
            jump_to(&program, ".back"),
            "jp",
            "129 bytes back once .far grows"
        );
        check_with_rgbds(&asm);
    }

    #[test]
    fn test_a_chain_of_jumps_grows_one_by_one() {
        // A ladder of ten jumps: jump n is 127 bytes from its label .tn, which comes
        // right after jump n + 1, so only jump n + 1 is in its way; the last one is
        // `last` bytes from .t9. With 128, jump 9 grows, which pushes .t8 one byte
        // further, so jump 8 grows, and so on down to jump 0: one more each pass.
        //   jr .t0  125 nops  jr .t1  .t0:  125 nops  jr .t2  .t1:  ...  jr .t9  .t8:
        //   `last` nops  .t9:
        let ladder = |last: usize| {
            one_section(|asm| {
                for step in 0..10 {
                    asm.jr(&format!(".t{}", step));
                    if step > 0 {
                        asm.label(&format!(".t{}", step - 1));
                    }
                    nops(asm, if step < 9 { 125 } else { last });
                }
                asm.label(".t9").ret();
            })
        };
        let asm = ladder(128);
        let code = asm.get_main_instrs();
        assert_eq!(
            jr_range_errors(&code),
            ["jr .t9: offset 128 is out of range"]
        );
        let program = asm.program();
        for step in 0..10 {
            assert_eq!(jump_to(&program, &format!(".t{}", step)), "jp", "{}", step);
        }
        check_with_rgbds(&asm);

        // One byte less, and none of them grows: the relaxation adds no jp it does not need
        let asm = ladder(127);
        assert_eq!(asm.program(), asm.get_main_instrs()[..]);
        check_with_rgbds(&asm);
    }

    #[test]
    fn test_what_cannot_be_measured_becomes_jp() {
        let mut asm = Asm::new();
        asm.raw("DEF S EQUS \"1, 2, 3, 4\"")
            .section(Section::rom0("Code").at(0x0000))
            .label("Main")
            // A label of another section, right after in the text
            .jr("Other")
            // A label the program does not define with a Label (here a raw line)
            .jr("External")
            .raw("External:")
            // Behind a raw line with code, or a string
            .jr(".after_raw")
            .raw("    nop")
            .label(".after_raw")
            .jr(".after_string")
            .db("\"ab\"")
            .label(".after_string")
            // Behind a symbol in data (here an EQUS of 4 values), or a comment whose text
            // goes on to a line of code
            .jr(".after_equs")
            .db("S")
            .label(".after_equs")
            .jr(".after_comment")
            .comment("x\n    nop")
            .label(".after_comment")
            // An absolute address
            .emit(Instr::Jr {
                target: JumpTarget::Addr(0x0000),
            })
            // What can be measured keeps its jr: a raw comment, data of plain numbers,
            // `ds` of a number, comments
            .jr(".measured")
            .raw("    ; a comment")
            .db("1, $FF, %101")
            .dw("$8000, 2")
            .ds_fill("4", "0")
            .comment("note")
            .label(".measured")
            // A jp stays a jp, however close
            .jp(".measured")
            .ret()
            .section(Section::rom0("Other").at(0x0100))
            .label("Other")
            .ret();
        let program = asm.program();
        for target in [
            "Other",
            "External",
            ".after_raw",
            ".after_string",
            ".after_equs",
            ".after_comment",
        ] {
            assert_eq!(jump_to(&program, target), "jp", "{}", target);
        }
        assert!(program.contains(&Instr::Jp {
            target: JumpTarget::Addr(0x0000)
        }));
        let measured = JumpTarget::Label(".measured".to_string());
        assert!(program.contains(&Instr::Jr {
            target: measured.clone()
        }));
        assert!(program.contains(&Instr::Jp { target: measured }));
        assert_eq!(jr_range_errors(&program), Vec::<String>::new());
        // RGBDS assembles it, and the jr skips the 11 measured bytes: 3 (db) + 4 (dw)
        // + 4 (ds); the raw comment and the comment are none
        if let Some(rom) = rgbds_rom(&asm.to_asm()) {
            let at: usize = program
                .iter()
                .take_while(|instr| !matches!(instr, Instr::Jr { .. }))
                .map(|instr| match instr {
                    // the raw `nop`, the string "ab", the EQUS and the comment's `nop`
                    Instr::Raw { line } if line.contains("nop") => 1,
                    Instr::Db { values } if values.contains('"') => 2,
                    Instr::Db { values } if values == "S" => 4,
                    Instr::Comment { text } if text.contains("nop") => 1,
                    other => other.size().unwrap_or_default(),
                })
                .sum();
            assert_eq!(&rom[at..at + 2], [0x18, 11]);
        }
    }

    #[test]
    fn test_labels_are_found_in_their_scope() {
        // Each routine jumps back to its own .loop, close by; the other .loop is far
        let asm = one_section(|asm| {
            asm.label("First").label(".loop").dec(R8::B);
            asm.jr_cond(Condition::NZ, ".loop").ret();
            nops(asm, 200);
            asm.label("Second").label(".loop").dec(R8::C);
            asm.jr_cond(Condition::NZ, ".loop").ret();
        });
        assert_eq!(asm.program(), asm.get_main_instrs()[..]);
        check_with_rgbds(&asm);

        // A label defined twice cannot be measured (and rgbasm rejects it)
        let asm = one_section(|asm| {
            asm.label("Twice").label("Twice").jr("Twice");
        });
        assert_eq!(jump_to(&asm.program(), "Twice"), "jp");
    }

    #[test]
    fn test_the_chunks_are_relaxed_as_one_program() {
        // A jump from the main loop to a label in the functions: the chunks are one
        // program, in their order, and each one keeps its own instructions
        let build = || {
            let mut asm = Asm::new();
            asm.chunk(Chunk::Header)
                .section(Section::rom0("Code").at(0x0000));
            asm.chunk(Chunk::MainLoop).label("Main").jr("Done");
            asm.chunk(Chunk::Functions);
            nops(&mut asm, 200);
            asm.label("Done").jr("Main");
            asm.chunk(Chunk::Init).label("Init").jr("Main");
            asm
        };
        let text = build().to_asm();
        assert_eq!(text, build().to_asm(), "the same program, the same text");
        assert!(text.contains("    jp Done\n\n    nop\n"), "{}", text);
        assert!(text.contains("    jp Main\n\n"), "{}", text);
        assert!(text.contains("Init:\n    jr Main\n\n    Main:"), "{}", text);
        check_with_rgbds(&build());
    }

    #[test]
    fn test_directive_sizes() {
        // A directive has a size when it is written with plain numbers and symbols; the
        // relaxation measures across it (the sizes are checked with RGBDS above, in
        // test_what_cannot_be_measured_becomes_jp)
        let text = |s: &str| s.to_string();
        let cases = [
            (
                Instr::Db {
                    values: text("1, $FF, %101"),
                },
                Some(3),
            ),
            (
                Instr::Db {
                    values: text("%1010 ; bits"),
                },
                Some(1),
            ),
            (
                Instr::Dw {
                    value: text("$8000, 2"),
                },
                Some(4),
            ),
            // A symbol can be an EQUS of several values; a line break starts a line of code
            (
                Instr::Dw {
                    value: text("Main, 2"),
                },
                None,
            ),
            (Instr::Db { values: text("S") }, None),
            (
                Instr::Comment {
                    text: text("x\n    nop"),
                },
                None,
            ),
            (
                Instr::Label {
                    name: text("A\n    nop\nB"),
                },
                None,
            ),
            (
                Instr::Ds {
                    count: text("4"),
                    fill: Some(text("0")),
                },
                Some(4),
            ),
            (
                Instr::Ds {
                    count: text("4"),
                    fill: None,
                },
                Some(4),
            ),
            (
                Instr::Incbin {
                    file: text("tiles.2bpp"),
                    offset: Some(16),
                    length: Some(32),
                },
                Some(32),
            ),
            (
                Instr::Raw {
                    line: text("    ; a comment"),
                },
                Some(0),
            ),
            (Instr::Raw { line: text("") }, Some(0)),
            (Instr::Section(Section::rom0("Code")), Some(0)),
            (Instr::Comment { text: text("note") }, Some(0)),
            // Only RGBDS knows these
            (
                Instr::Db {
                    values: text("\"ab\""),
                },
                None,
            ),
            (
                Instr::Db {
                    values: text("LOW(Main)"),
                },
                None,
            ),
            (Instr::Db { values: text("") }, None),
            (
                Instr::Ds {
                    count: text("$150 - @"),
                    fill: Some(text("0")),
                },
                None,
            ),
            (
                Instr::Incbin {
                    file: text("tiles.2bpp"),
                    offset: None,
                    length: None,
                },
                None,
            ),
            (
                Instr::Include {
                    file: text("x.inc"),
                },
                None,
            ),
            (
                Instr::Raw {
                    line: text("    nop"),
                },
                None,
            ),
        ];
        for (instr, size) in cases {
            assert_eq!(instr.size(), size, "{:?}", instr);
        }
    }

    /// Checks the one jump of `asm` whose target is written from `@`: in the relaxed
    /// program it lands on the label `.want` (a marker, which takes no room), by its text
    /// and, with RGBDS, in the ROM. Returns that jump as printed.
    fn check_at_target(asm: &Asm) -> String {
        let program = asm.program();
        assert_eq!(jr_range_errors(&program), Vec::<String>::new());
        let mut addresses = Vec::new();
        let mut address = 0usize;
        for instr in &program {
            addresses.push(address);
            address += instr.size().expect("a known size");
        }
        let want = program
            .iter()
            .position(|instr| {
                *instr
                    == Instr::Label {
                        name: ".want".into(),
                    }
            })
            .map(|index| addresses[index])
            .expect("a .want label");
        let (index, jump) = program
            .iter()
            .enumerate()
            .find(|(_, instr)| {
                jump_target(instr).is_some_and(|t| matches!(classify(t), Target::Here(_)))
            })
            .expect("a jump to @");
        let Target::Here(offset) = classify(jump_target(jump).unwrap()) else {
            unreachable!()
        };
        let start = addresses[index];
        assert_eq!(
            start as i64 + offset,
            want as i64,
            "{} at ${:04x}",
            jump,
            start
        );
        if let Some(rom) = rgbds_rom(&asm.to_asm()) {
            let reached = if is_jr(jump) {
                (start as isize + 2 + isize::from(rom[start + 1] as i8)) as usize
            } else {
                usize::from(u16::from_le_bytes([rom[start + 1], rom[start + 2]]))
            };
            assert_eq!(reached, want, "{} at ${:04x} in the ROM", jump, start);
        }
        jump.to_string()
    }

    #[test]
    fn test_targets_from_at_keep_their_meaning() {
        // `@` is the start of the jump: `@+4` is 4 bytes from it. Turned into a jp (one byte
        // longer) without a new offset, it landed one byte early, inside `ld a, 1`.
        let asm = one_section(|asm| {
            asm.jr_cond(Condition::NZ, "@+4")
                .ld_a(1)
                .label(".want")
                .ret();
        });
        assert_eq!(
            check_at_target(&asm),
            "jr nz, @+4",
            "nothing grows: as written"
        );
        if let Some(rom) = rgbds_rom(&asm.to_asm()) {
            assert_eq!(rom[..5], [0x20, 0x02, 0x3E, 0x01, 0xC9]);
        }

        // Forward over a jr that grows: the offset grows with it
        let asm = one_section(|asm| {
            asm.jr_cond(Condition::NZ, "@+6")
                .jr(".far")
                .ld_a(1)
                .label(".want")
                .ret();
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(check_at_target(&asm), "jr nz, @+7");
        assert_eq!(jump_to(&asm.program(), ".far"), "jp");

        // A jr to `@` that grows itself: its forward offset counts its own new byte
        let asm = one_section(|asm| {
            asm.jr_cond(Condition::C, "@+131");
            nops(asm, 129);
            asm.label(".want").ret();
        });
        assert_eq!(check_at_target(&asm), "jp c, @+132");

        // Backward over a jr that grows, and a backward jr that grows itself (its target,
        // before it, does not move)
        let asm = one_section(|asm| {
            asm.label(".want")
                .ld_a(1)
                .jr(".far")
                .jr_cond(Condition::Z, "@-4");
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(check_at_target(&asm), "jr z, @-5");
        let asm = one_section(|asm| {
            asm.label(".want");
            nops(asm, 130);
            asm.jr("@-130").ret();
        });
        assert_eq!(check_at_target(&asm), "jp @-130");

        // `jp` and `call` written from `@`, over a jr that grows; `@` alone
        let asm = one_section(|asm| {
            asm.call("@+5").jr(".far").label(".want").ret();
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(check_at_target(&asm), "call @+6");
        let asm = one_section(|asm| {
            asm.jr(".far").label(".want").jp_cond(Condition::NC, "@");
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(check_at_target(&asm), "jp nc, @");
    }

    #[test]
    fn test_a_target_it_cannot_follow_leaves_the_program_as_written() {
        // `@+3` is inside `ld a, 1`, `Main + 2` is an expression: a jr that grows could
        // move them, so no jump of the program changes (rgbasm then reports the far jr)
        for target in ["@+3", "Main + 2", "@ * 2"] {
            let asm = one_section(|asm| {
                asm.jr_cond(Condition::NZ, target).ld_a(1).jr(".far");
                nops(asm, 128);
                asm.label(".far").ret();
            });
            assert_eq!(asm.program(), asm.get_main_instrs()[..], "{}", target);
        }
        // An `@` target over a size only RGBDS knows, or into another section
        let asm = one_section(|asm| {
            asm.jr("@+3").raw("    nop").jr(".far");
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(asm.program(), asm.get_main_instrs()[..]);
        let asm = one_section(|asm| {
            asm.jr(".far")
                .jr("@+2")
                .section(Section::rom0("Other"))
                .label("Other");
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(asm.program(), asm.get_main_instrs()[..]);
    }

    #[test]
    fn test_target_kinds() {
        let kind = |text: &str| classify(&JumpTarget::Label(text.to_string()));
        assert_eq!(kind("@"), Target::Here(0));
        assert_eq!(kind("@ + $10"), Target::Here(16));
        assert_eq!(kind("@-4"), Target::Here(-4));
        assert_eq!(kind("Main"), Target::Name("Main".into()));
        assert_eq!(kind(".loop"), Target::Name(".loop".into()));
        assert_eq!(kind("Main.loop"), Target::Name("Main.loop".into()));
        assert_eq!(kind(":++"), Target::Name(":++".into()));
        assert_eq!(kind("$0150"), Target::Fixed);
        assert_eq!(classify(&JumpTarget::Addr(0x150)), Target::Fixed);
        for expression in ["Main + 2", "@ * 2", "@+x", "LOW(Main)", ":+-"] {
            assert_eq!(kind(expression), Target::Expression, "{}", expression);
        }
    }

    /// `asm` as RGBDS sees it: `Err` with its errors if it does not assemble and link
    /// (`None` without `RGBDS_LINK_CHECK`)
    fn rgbds_result(asm: &Asm) -> Option<Result<Vec<u8>, String>> {
        std::env::var_os("RGBDS_LINK_CHECK")?;
        let text = asm.to_asm();
        let result = std::panic::catch_unwind(|| rgbds_rom(&text).unwrap());
        Some(result.map_err(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default()
        }))
    }

    #[test]
    fn test_an_offset_from_at_elsewhere_leaves_the_program_as_written() {
        // `@` in a raw line, an operand or data: the relaxation cannot write it again, and a
        // jr that grew in its span would move its target (each landed one byte early), so
        // the program is printed as written and rgbasm reports the far jr, loudly
        type AtCode = fn(&mut Asm);
        let cases: [(&str, AtCode); 3] = [
            ("raw line", |asm| {
                asm.raw("    jr nz, @+4");
            }),
            ("operand", |asm| {
                asm.ld(crate::gb_asm::R16::HL, crate::gb_asm::Expr::raw("@+5"));
            }),
            ("data", |asm| {
                asm.dw("@+4");
            }),
        ];
        for (name, at_code) in cases {
            let asm = one_section(|asm| {
                at_code(asm);
                asm.jr(".far").ld_a(1);
                nops(asm, 128);
                asm.label(".far").ret();
            });
            assert_eq!(asm.program(), asm.get_main_instrs()[..], "{}", name);
            if let Some(result) = rgbds_result(&asm) {
                let error = result.expect_err(name);
                assert!(
                    error.contains("`JR` target must be between -128 and 127 bytes away"),
                    "{}: {}",
                    name,
                    error
                );
            }
        }

        // The padding `ds N - @`, `@` in a comment or a string, and a name with `@` in it
        // keep the relaxation
        let asm = one_section(|asm| {
            asm.ds_fill("$10 - @", "0")
                .comment("@param a: nothing")
                .db("\"a@b\"")
                .label("a@b")
                .jr(".far");
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(jump_to(&asm.program(), ".far"), "jp");
        assert!(!uses_here_elsewhere(&asm.get_main_instrs()));
        if let Some(result) = rgbds_result(&asm) {
            result.expect("it assembles");
        }
    }

    #[test]
    fn test_a_dw_of_labels_has_a_known_size() {
        // A name the program defines as a label is an address, 2 bytes in a dw (it cannot
        // also be an EQUS); an undefined name could be one
        let asm = one_section(|asm| {
            asm.jr(".after")
                .dw("Main, .after, $1234")
                .label(".after")
                .ret();
            asm.jr(".end").dw("Unknown").label(".end").ret();
        });
        let program = asm.program();
        assert_eq!(jump_to(&program, ".after"), "jr");
        assert_eq!(jump_to(&program, ".end"), "jp");
        assert_eq!(jr_range_errors(&program), Vec::<String>::new());
        let defined = rgbds_result(&one_section(|asm| {
            asm.jr(".after")
                .dw("Main, .after, $1234")
                .label(".after")
                .ret();
        }));
        if let Some(result) = defined {
            assert_eq!(result.expect("it assembles")[..2], [0x18, 6]);
        }
    }

    #[test]
    fn test_an_at_offset_is_read_as_rgbasm_reads_it() {
        let kind = |text: &str| classify(&JumpTarget::Label(text.to_string()));
        assert_eq!(kind("@ + 10"), Target::Here(10));
        assert_eq!(kind("@+$0A"), Target::Here(10));
        // rgbasm rejects `1 0`: it is no offset, and the program is left as written
        assert_eq!(kind("@+ 1 0"), Target::Expression);
        let asm = one_section(|asm| {
            asm.jr_cond(Condition::NZ, "@+ 1 0").jr(".far");
            nops(asm, 128);
            asm.label(".far").ret();
        });
        assert_eq!(asm.program(), asm.get_main_instrs()[..]);
        // The test helper reads the same numbers
        let asm = one_section(|asm| {
            asm.jr("@+$04").nop().nop().ret();
        });
        assert_eq!(jr_range_errors(&asm.program()), Vec::<String>::new());
    }
}
