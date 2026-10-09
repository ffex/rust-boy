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
//!   a `SECTION` ends the scope);
//! - the target must be in the same section, with only instructions of known size
//!   ([`Instr::size`]) between them, and the offset from the end of the `jr` must be in
//!   -128..=127.
//!
//! Every other `jr` becomes a `jp` with the same condition and target: a target out of
//! range, in another section, unknown to the program (an external symbol, a label of an
//! `INCLUDE`d file or of a raw line), defined twice, an absolute address, or a jump over a
//! `ds $150 - @`, a `db` with a string, an `INCBIN` without a length, an `INCLUDE` or a raw
//! line with code (their size is known only to RGBDS). A `jp` is never made a `jr`: what
//! the code says `jp` stays `jp`.
//!
//! The relaxation is iterative: a `jr` that grows to a `jp` takes one more byte, which can
//! push another `jr` over the same bytes out of range. It starts with every `jr` short and
//! grows the ones out of reach until none is; offsets only grow when a jump grows, so this
//! ends (at most once per `jr`) on the fewest `jp`s.

use std::collections::BTreeMap;

use super::instr::{Instr, JumpTarget};

/// `program` with every `jr` / `jr cc` that does not provably reach its target turned into
/// a `jp` / `jp cc` (see the [module documentation](self) for the rules)
pub(crate) fn relax_jumps(program: &[Instr]) -> Vec<Instr> {
    let long = long_jumps(program);
    program
        .iter()
        .zip(long)
        .map(|(instr, long)| match instr {
            Instr::Jr { target } if long => Instr::Jp {
                target: target.clone(),
            },
            Instr::JrCond { condition, target } if long => Instr::JpCond {
                condition: condition.clone(),
                target: target.clone(),
            },
            _ => instr.clone(),
        })
        .collect()
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

/// For each instruction of `program`, whether it is a `jr` that must become a `jp`
fn long_jumps(program: &[Instr]) -> Vec<bool> {
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

    // The label each `jr` jumps to, if the program defines it once
    let targets: Vec<Option<usize>> = program
        .iter()
        .zip(&scopes)
        .map(|(instr, scope)| match instr {
            Instr::Jr {
                target: JumpTarget::Label(name),
            }
            | Instr::JrCond {
                target: JumpTarget::Label(name),
                ..
            } => full_name(*scope, name).and_then(|full| labels.get(&full).copied().flatten()),
            _ => None,
        })
        .collect();

    let sizes: Vec<Option<usize>> = program.iter().map(Instr::size).collect();
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
            return long;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::label_check::{jr_range_errors, rgbds_rom};
    use crate::gb_asm::{Asm, Chunk, Condition, R8};

    /// A program in one ROM0 section at $0000, starting with the global label `Main`,
    /// then the code `build` writes
    fn one_section(build: impl FnOnce(&mut Asm)) -> Asm {
        let mut asm = Asm::new();
        asm.section("Code", "ROM0[$0000]").label("Main");
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
        asm.section("Code", "ROM0[$0000]")
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
            // An absolute address
            .emit(Instr::Jr {
                target: JumpTarget::Addr(0x0000),
            })
            // What can be measured keeps its jr: a raw comment, data of plain numbers and
            // symbols, `ds` of a number, comments
            .jr(".measured")
            .raw("    ; a comment")
            .db("1, $FF, Main")
            .dw("Main, 2")
            .ds("4", "0")
            .comment("note")
            .label(".measured")
            // A jp stays a jp, however close
            .jp(".measured")
            .ret()
            .section("Other", "ROM0[$0100]")
            .label("Other")
            .ret();
        let program = asm.program();
        for target in ["Other", "External", ".after_raw", ".after_string"] {
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
                    // the raw `nop` and the string "ab" are 1 and 2 bytes
                    Instr::Raw { line } if line.contains("nop") => 1,
                    Instr::Db { values } if values.contains('"') => 2,
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
            asm.chunk(Chunk::Header).section("Code", "ROM0[$0000]");
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
                    values: text("1, $FF, Main"),
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
                    value: text("Main, 2"),
                },
                Some(4),
            ),
            (
                Instr::Ds {
                    num_bytes: text("4"),
                    starter_point: text("0"),
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
            (
                Instr::Section {
                    name: text("Code"),
                    mem_type: text("ROM0"),
                },
                Some(0),
            ),
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
                    num_bytes: text("$150 - @"),
                    starter_point: text("0"),
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
}
