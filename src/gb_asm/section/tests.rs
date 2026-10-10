//! Tests of [`Section`] and [`SectionTracker`]: every form, printed and placed by RGBDS,
//! and every rejected form, rejected by RGBDS too (with `RGBDS_LINK_CHECK`).

use super::*;
use crate::gb_asm::Asm;
use crate::gb_asm::label_check::{rgbds_accepts, rgbds_rom_and_symbols};

/// What `f` returns, or the message of its panic
fn catch<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|err| {
        err.downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

/// The message of the panic of `f`, or `None` if it returns
fn panic_of<R>(f: impl FnOnce() -> R) -> Option<String> {
    catch(f).err()
}

/// One line of content for a section of `memory`: code in ROM, a reservation in RAM
fn content(memory: MemoryType) -> &'static str {
    if memory.is_rom() { "nop" } else { "ds 1" }
}

/// Emit one byte of content for `memory` (see [`content`])
fn emit_content(asm: &mut Asm, memory: MemoryType) {
    if memory.is_rom() {
        asm.nop();
    } else {
        asm.ds("1");
    }
}

/// Every form of section, with the text it prints
fn forms() -> Vec<(Section, &'static str)> {
    vec![
        (Section::rom0("Rom0"), r#"SECTION "Rom0", ROM0"#),
        (
            Section::rom0("Rom0 fixed").at(0x0200),
            r#"SECTION "Rom0 fixed", ROM0[$200]"#,
        ),
        (
            Section::rom0("Rom0 aligned").align(8),
            r#"SECTION "Rom0 aligned", ROM0, ALIGN[8]"#,
        ),
        (
            Section::rom0("Rom0 offset").align_offset(4, 3),
            r#"SECTION "Rom0 offset", ROM0, ALIGN[4, 3]"#,
        ),
        (
            Section::rom0("Rom0 fixed aligned").at(0x0300).align(8),
            r#"SECTION "Rom0 fixed aligned", ROM0[$300], ALIGN[8]"#,
        ),
        (
            Section::rom0("Rom0 fragment").fragment(),
            r#"SECTION FRAGMENT "Rom0 fragment", ROM0"#,
        ),
        (Section::romx("Romx"), r#"SECTION "Romx", ROMX"#),
        (
            Section::romx("Romx bank").bank(2),
            r#"SECTION "Romx bank", ROMX, BANK[2]"#,
        ),
        (
            Section::romx("Romx fixed").at(0x4100).bank(1),
            r#"SECTION "Romx fixed", ROMX[$4100], BANK[1]"#,
        ),
        (
            Section::romx("Romx aligned")
                .bank(3)
                .align_offset(16, 0x4000),
            r#"SECTION "Romx aligned", ROMX, BANK[3], ALIGN[16, 16384]"#,
        ),
        (
            Section::romx("Romx fragment").bank(2).fragment(),
            r#"SECTION FRAGMENT "Romx fragment", ROMX, BANK[2]"#,
        ),
        (
            Section::vram("Vram").at(0x8800),
            r#"SECTION "Vram", VRAM[$8800]"#,
        ),
        (
            Section::vram("Vram bank").bank(1),
            r#"SECTION "Vram bank", VRAM, BANK[1]"#,
        ),
        (Section::sram("Sram"), r#"SECTION "Sram", SRAM"#),
        (
            Section::sram("Sram bank").at(0xA000).bank(1),
            r#"SECTION "Sram bank", SRAM[$A000], BANK[1]"#,
        ),
        (Section::wram0("Wram0"), r#"SECTION "Wram0", WRAM0"#),
        (
            Section::wram0("Wram0 fixed").at(0xC100),
            r#"SECTION "Wram0 fixed", WRAM0[$C100]"#,
        ),
        (
            Section::wram0("Wram0 aligned").align(8),
            r#"SECTION "Wram0 aligned", WRAM0, ALIGN[8]"#,
        ),
        (
            Section::wram0("Wram0 union").union(),
            r#"SECTION UNION "Wram0 union", WRAM0"#,
        ),
        (
            Section::wram0("Wram0 fragment").fragment(),
            r#"SECTION FRAGMENT "Wram0 fragment", WRAM0"#,
        ),
        (
            Section::wramx("Wramx").bank(2),
            r#"SECTION "Wramx", WRAMX, BANK[2]"#,
        ),
        (
            Section::wramx("Wramx fixed").at(0xD000),
            r#"SECTION "Wramx fixed", WRAMX[$D000]"#,
        ),
        (Section::oam("Oam"), r#"SECTION "Oam", OAM"#),
        (
            Section::oam("Oam fixed").at(0xFE00),
            r#"SECTION "Oam fixed", OAM[$FE00]"#,
        ),
        (
            Section::hram("Hram").at(0xFF90),
            r#"SECTION "Hram", HRAM[$FF90]"#,
        ),
        (
            Section::hram("Hram union").union().align_offset(7, 0x20),
            r#"SECTION UNION "Hram union", HRAM, ALIGN[7, 32]"#,
        ),
        (
            Section::new("Any text: 1 + 2 = 3, é }", MemoryType::Rom0),
            r#"SECTION "Any text: 1 + 2 = 3, é }", ROM0"#,
        ),
        (
            Section::new("A\ttab", MemoryType::Rom0),
            "SECTION \"A\ttab\", ROM0",
        ),
    ]
}

/// Whether RGBDS put the section's first label at a place the section allows: in its
/// memory type, in its bank, at its address, with its alignment
fn assert_placed(section: &Section, label: &str, (bank, address): (u32, u16)) {
    let place = format!("{} at {:02X}:{:04X}", label, bank, address);
    assert!(
        section.memory().range().contains(&address),
        "{}: {}",
        section,
        place
    );
    match section.bank_number() {
        Some(expected) => assert_eq!(bank, expected, "{}: {}", section, place),
        None => {
            if let Some(banks) = section.memory().banks() {
                assert!(banks.contains(&bank), "{}: {}", section, place);
            }
        }
    }
    if let Some(expected) = section.address() {
        assert_eq!(address, expected, "{}: {}", section, place);
    }
    if let Some(align) = section.alignment() {
        assert_eq!(
            u32::from(address) & align.mask(),
            u32::from(align.offset),
            "{}: {}",
            section,
            place
        );
    }
}

#[test]
fn test_every_form_prints_and_links() {
    // Each form, opened twice when it is a UNION or a FRAGMENT, a label before each piece
    let mut asm = Asm::new();
    for (index, (section, text)) in forms().into_iter().enumerate() {
        assert_eq!(section.to_string(), text);
        assert_eq!(section.check(), Ok(()));
        assert_eq!(Instr::Section(section.clone()).to_string(), text);
        asm.section(section.clone()).label(&format!("S{}", index));
        emit_content(&mut asm, section.memory());
        if section.kind() != SectionKind::Plain {
            asm.section(section.clone())
                .label(&format!("S{}_again", index));
            emit_content(&mut asm, section.memory());
        }
    }
    let program = asm.to_asm();
    let Some((_, symbols)) = rgbds_rom_and_symbols(&program) else {
        return;
    };
    for (index, (section, _)) in forms().into_iter().enumerate() {
        let label = format!("S{}", index);
        let place = symbols[&label];
        assert_placed(&section, &label, place);
        let again = symbols.get(&format!("S{}_again", index)).copied();
        match section.kind() {
            SectionKind::Plain => assert_eq!(again, None),
            // Every piece of a union starts at the same address
            SectionKind::Union => assert_eq!(again, Some(place), "{}", section),
            // The fragments of one file follow each other, in order
            SectionKind::Fragment => {
                assert_eq!(again, Some((place.0, place.1 + 1)), "{}", section)
            }
        }
    }
}

#[test]
fn test_ds_without_fill_reserves_in_ram_and_pads_in_rom() {
    let mut asm = Asm::new();
    asm.section(Section::wram0("Vars").at(0xC000))
        .label("wA")
        .ds("1")
        .label("wB")
        .ds("2")
        // `db` / `dw` without values reserve 1 / 2 bytes, as a typed line or a raw one
        .raw("wC: db")
        .label("wD")
        .dw("")
        .label("wE")
        .db("")
        .label("wF")
        .comment("comments and DEFs take no room")
        .def("SIZE", 3)
        .ds("SIZE")
        .label("wEnd");
    // In ROM, `ds n` is n bytes of rgblink's padding value (0 by default)
    asm.section(Section::rom0("Code").at(0x1000))
        .label("Code")
        .ds("2")
        .db("$AA")
        .ds_fill("2", "$55");
    let text = asm.to_asm();
    assert!(text.contains("    ds 2\n"), "{}", text);
    assert!(text.contains("    ds 2, $55\n"), "{}", text);
    let Some((rom, symbols)) = rgbds_rom_and_symbols(&text) else {
        return;
    };
    let addresses: Vec<u16> = ["wA", "wB", "wC", "wD", "wE", "wF", "wEnd"]
        .iter()
        .map(|name| symbols[*name].1)
        .collect();
    assert_eq!(
        addresses,
        [0xC000, 0xC001, 0xC003, 0xC004, 0xC006, 0xC007, 0xC00A]
    );
    assert_eq!(&rom[0x1000..0x1005], [0x00, 0x00, 0xAA, 0x55, 0x55]);
}

/// A section built field by field, as the methods of `Section` never make it
fn raw_section(memory: MemoryType) -> Section {
    Section {
        name: "Bad".to_string(),
        memory,
        kind: SectionKind::Plain,
        address: None,
        bank: None,
        align: None,
    }
}

#[test]
fn test_check_rejects_what_rgbds_rejects() {
    use MemoryType::*;
    let with = |memory: MemoryType, change: &dyn Fn(&mut Section)| {
        let mut section = raw_section(memory);
        change(&mut section);
        section
    };
    let bank = |bank: u32| move |s: &mut Section| s.bank = Some(bank);
    let at = |address: u16| move |s: &mut Section| s.address = Some(address);
    let align =
        |bits: u8, offset: u16| move |s: &mut Section| s.align = Some(Align { bits, offset });
    let cases: Vec<(Section, &str)> = vec![
        // A bank where there are none, or out of range
        (with(Rom0, &bank(0)), "ROM0 has no banks"),
        (with(Rom0, &bank(1)), "ROM0 has no banks"),
        (with(Wram0, &bank(0)), "WRAM0 has no banks"),
        (with(Oam, &bank(0)), "OAM has no banks"),
        (with(Hram, &bank(0)), "HRAM has no banks"),
        (with(Romx, &bank(0)), "ROMX banks are 1 to 65535, not 0"),
        (with(Vram, &bank(2)), "VRAM banks are 0 to 1, not 2"),
        (with(Sram, &bank(256)), "SRAM banks are 0 to 255, not 256"),
        (with(Wramx, &bank(0)), "WRAMX banks are 1 to 7, not 0"),
        (with(Wramx, &bank(8)), "WRAMX banks are 1 to 7, not 8"),
        // An address outside the memory type
        (
            with(Rom0, &at(0x4000)),
            "$4000 is outside ROM0 ($0000-$3FFF)",
        ),
        (with(Romx, &at(0x3FFF)), "$3FFF is outside ROMX"),
        (with(Vram, &at(0xA000)), "$A000 is outside VRAM"),
        (with(Sram, &at(0x9FFF)), "$9FFF is outside SRAM"),
        (with(Wram0, &at(0xD000)), "$D000 is outside WRAM0"),
        (with(Wramx, &at(0xCFFF)), "$CFFF is outside WRAMX"),
        (with(Oam, &at(0xFEA0)), "$FEA0 is outside OAM"),
        (with(Hram, &at(0xFF7F)), "$FF7F is outside HRAM"),
        (with(Hram, &at(0xFFFF)), "$FFFF is outside HRAM"),
        // An alignment out of range, not the fixed address's, or that no address has
        (
            with(Rom0, &align(17, 0)),
            "the alignment must be 0 to 16 bits",
        ),
        (
            with(Rom0, &align(8, 256)),
            "the alignment offset must be below 2^8 = 256",
        ),
        (
            with(Rom0, &|s| {
                s.address = Some(0x0106);
                s.align = Some(Align { bits: 2, offset: 0 });
            }),
            "the fixed address does not have this alignment",
        ),
        (with(Romx, &align(16, 0)), "no address of ROMX"),
        (with(Hram, &align(8, 0)), "no address of HRAM"),
        (with(Hram, &align(7, 0x7F)), "no address of HRAM"),
        (with(Oam, &align(8, 0xA0)), "no address of OAM"),
        // UNION in ROM
        (
            with(Rom0, &|s| s.kind = SectionKind::Union),
            "a UNION cannot be in ROM",
        ),
        (
            with(Romx, &|s| s.kind = SectionKind::Union),
            "a UNION cannot be in ROM",
        ),
        // A name that is no plain string
        (
            with(Rom0, &|s| s.name = "a\"b".to_string()),
            "cannot be written in an RGBDS string",
        ),
        (
            with(Rom0, &|s| s.name = "{x}".to_string()),
            "cannot be written in an RGBDS string",
        ),
        (
            with(Rom0, &|s| s.name = "a\\q".to_string()),
            "cannot be written in an RGBDS string",
        ),
        (
            with(Rom0, &|s| s.name = "a\nb".to_string()),
            "cannot be written in an RGBDS string",
        ),
    ];
    for (section, why) in cases {
        let error = section.check().expect_err(&section.to_string());
        assert!(error.contains(why), "{}: {}", section, error);
        // `Instr::check` rejects it too, so `Asm::emit` and the output panic on it
        assert!(Instr::Section(section.clone()).check().is_err());
        let text = format!("{}\n    {}\n", section, content(section.memory()));
        assert_ne!(rgbds_accepts(&text), Some(true), "RGBDS accepts:\n{}", text);
    }
}

#[test]
#[should_panic(expected = "invalid section: SECTION \"X\", ROM0, BANK[1]: ROM0 has no banks")]
fn test_a_bank_on_rom0_panics() {
    Section::rom0("X").bank(1);
}

#[test]
#[should_panic(expected = "$FFFF is outside HRAM ($FF80-$FFFE)")]
fn test_an_address_outside_its_memory_panics() {
    Section::hram("X").at(0xFFFF);
}

#[test]
#[should_panic(expected = "the fixed address does not have this alignment")]
fn test_an_alignment_the_address_has_not_panics() {
    Section::rom0("X").at(0x0106).align(2);
}

#[test]
#[should_panic(expected = "no address of HRAM ($FF80-$FFFE) has this alignment")]
fn test_an_alignment_no_address_has_panics() {
    Section::hram("X").align(8);
}

#[test]
#[should_panic(expected = "a UNION cannot be in ROM")]
fn test_a_union_in_rom_panics() {
    Section::romx("X").union();
}

#[test]
#[should_panic(expected = "cannot be written in an RGBDS string")]
fn test_a_name_with_a_quote_panics() {
    Section::wram0("say \"hi\"");
}

/// The text of `section` with `instrs` after it, printed one per line
fn text_of(sections: &[(Section, Vec<Instr>)]) -> String {
    let mut text = String::new();
    for (section, instrs) in sections {
        text.push_str(&format!("{}\n", section));
        for instr in instrs {
            text.push_str(&format!("    {}\n", instr));
        }
    }
    text
}

/// An `Asm` of `sections`, each with its instructions; `Err` with the panic message if
/// it does not print
fn print(sections: &[(Section, Vec<Instr>)]) -> Result<String, String> {
    let mut asm = Asm::new();
    for (section, instrs) in sections {
        asm.section(section.clone()).emit_all(instrs.clone());
    }
    catch(|| asm.to_asm())
}

#[test]
fn test_code_or_data_in_ram_is_rejected() {
    let text = |s: &str| s.to_string();
    let code_and_data = [
        Instr::Nop,
        Instr::Ld {
            dst: crate::gb_asm::Dst::R8(crate::gb_asm::R8::A),
            src: crate::gb_asm::Operand::from(1),
        },
        Instr::Ret,
        Instr::Ds {
            count: text("2"),
            fill: Some(text("0")),
        },
        Instr::Db { values: text("1") },
        Instr::Dw {
            value: text("$1234"),
        },
        Instr::Incbin {
            file: text("tiles.2bpp"),
            offset: None,
            length: None,
        },
    ];
    for memory in MemoryType::ALL {
        let section = Section::new("Area", memory);
        for instr in &code_and_data {
            let sections = [(section.clone(), vec![instr.clone()])];
            let printed = print(&sections);
            if memory.is_rom() {
                assert!(printed.is_ok(), "{}: {:?}", instr, printed);
                continue;
            }
            let error = printed.expect_err(&instr.to_string());
            assert!(
                error.contains(&format!(
                    "`{}` in the {} section \"Area\": a RAM section holds no code or data",
                    instr, memory
                )),
                "{}",
                error
            );
            // RGBDS rejects it too (the INCBIN has no file, so it is left out)
            if !matches!(instr, Instr::Incbin { .. }) {
                let text = text_of(&sections);
                assert!(!rgbds_accepts(&text).unwrap_or(false), "{}", text);
            }
        }
    }
}

#[test]
fn test_a_section_name_is_used_once() {
    let wram_union = Section::wram0("Shared").union();
    let wram_fragment = Section::wram0("Shared").fragment();
    let cases: Vec<(Vec<Section>, bool)> = vec![
        (vec![Section::rom0("Code"), Section::rom0("Code")], false),
        (
            vec![Section::rom0("Code"), Section::rom0("Code").fragment()],
            false,
        ),
        (vec![wram_union.clone(), wram_union.clone()], true),
        (vec![wram_fragment.clone(), wram_fragment.clone()], true),
        (vec![wram_union.clone(), wram_fragment.clone()], false),
        (
            vec![
                Section::rom0("Code").fragment(),
                Section::romx("Code").fragment(),
            ],
            false,
        ),
        (
            vec![
                Section::romx("Code").bank(2).fragment(),
                Section::romx("Code").bank(3).fragment(),
            ],
            false,
        ),
        (
            vec![
                Section::romx("Code").bank(2).fragment(),
                Section::romx("Code").fragment(),
            ],
            true,
        ),
        // Another section in between does not change it
        (
            vec![
                Section::rom0("Code"),
                Section::rom0("Other"),
                Section::rom0("Code"),
            ],
            false,
        ),
    ];
    for (sections, valid) in cases {
        let sections: Vec<(Section, Vec<Instr>)> = sections
            .into_iter()
            .map(|section| {
                let content = if section.memory().is_rom() {
                    Instr::Nop
                } else {
                    Instr::Ds {
                        count: "1".to_string(),
                        fill: None,
                    }
                };
                (section, vec![content])
            })
            .collect();
        let printed = print(&sections);
        let text = text_of(&sections);
        assert_eq!(printed.is_ok(), valid, "{:?}\n{}", printed, text);
        if let Err(error) = printed {
            assert!(error.contains("is already opened"), "{}", error);
        }
        if let Some(accepts) = rgbds_accepts(&text) {
            assert_eq!(accepts, valid, "RGBDS on:\n{}", text);
        }
    }
}

#[test]
fn test_a_raw_line_or_include_may_change_the_section() {
    // A raw line that opens a section: what follows is not checked against the RAM
    // section before it (RGBDS accepts this program)
    let mut asm = Asm::new();
    asm.section(Section::wram0("Vars"))
        .raw("wA: db")
        .raw("SECTION \"Code\", ROM0")
        .nop();
    let text = asm.to_asm();
    assert_ne!(rgbds_accepts(&text), Some(false), "{}", text);

    // The same after an INCLUDE, a macro, or any other raw code
    for line in [
        "INCLUDE \"hardware.inc\"",
        "my_macro",
        "LOAD \"Ram\", WRAM0",
    ] {
        let mut asm = Asm::new();
        asm.section(Section::wram0("Vars")).raw(line).nop();
        assert_eq!(panic_of(|| asm.to_asm()), None, "{}", line);
    }
    let mut asm = Asm::new();
    asm.section(Section::wram0("Vars"))
        .include("hardware.inc")
        .nop();
    assert_eq!(panic_of(|| asm.to_asm()), None);

    // Raw labels and reservations keep the section: a `nop` after them is rejected
    for line in [
        "wA: db",
        "wB:: dw",
        ".local: ds 2",
        "wC:\n    ds 4 ; four",
        "",
    ] {
        let mut asm = Asm::new();
        asm.section(Section::wram0("Vars"))
            .label("Vars")
            .raw(line)
            .nop();
        let error = panic_of(|| asm.to_asm()).unwrap_or_else(|| panic!("{:?} accepted", line));
        assert!(
            error.contains("a RAM section holds no code or data"),
            "{}",
            error
        );
    }

    // A comment written with a line break prints code: it is read as a raw line
    let mut asm = Asm::new();
    asm.section(Section::wram0("Vars"))
        .comment("note\nSECTION \"Code\", ROM0")
        .nop();
    let text = asm.to_asm();
    assert_ne!(rgbds_accepts(&text), Some(false), "{}", text);
    let mut asm = Asm::new();
    asm.section(Section::wram0("Vars"))
        .comment("note\nwA: ds 1")
        .nop();
    assert!(panic_of(|| asm.to_asm()).is_some());

    // A typed section after an unknown one is checked again
    let mut asm = Asm::new();
    asm.raw("SECTION \"Code\", ROM0")
        .nop()
        .section(Section::hram("Fast"))
        .nop();
    let error = panic_of(|| asm.to_asm()).expect("a nop in HRAM");
    assert!(
        error.contains("`nop` in the HRAM section \"Fast\""),
        "{}",
        error
    );
}

#[test]
fn test_sections_print_in_the_order_they_are_opened() {
    let names = ["Zeta", "Alpha", "Mu", "Beta", "Omega", "Gamma"];
    let program = || {
        let mut asm = Asm::new();
        for (index, name) in names.iter().enumerate() {
            let memory = MemoryType::ALL[index % MemoryType::ALL.len()];
            asm.section(Section::new(name, memory));
            emit_content(&mut asm, memory);
        }
        asm.to_asm()
    };
    let text = program();
    let printed: Vec<&str> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("SECTION \""))
        .map(|rest| rest.split('"').next().unwrap())
        .collect();
    assert_eq!(printed, names);
    for _ in 0..10 {
        assert_eq!(program(), text);
    }
}

#[test]
fn test_memory_types_print_their_keyword() {
    let keywords: Vec<String> = MemoryType::ALL.iter().map(|m| m.to_string()).collect();
    assert_eq!(
        keywords,
        [
            "ROM0", "ROMX", "VRAM", "SRAM", "WRAM0", "WRAMX", "OAM", "HRAM"
        ]
    );
    // In address order, without overlaps
    for pair in MemoryType::ALL.windows(2) {
        assert!(pair[0].range().end() < pair[1].range().start());
    }
}

/// The text of the `Asm` that `write` writes, or the message it panics with (when the
/// instruction is emitted, or when the program is printed)
fn written(write: impl FnOnce(&mut Asm)) -> Result<String, String> {
    catch(|| {
        let mut asm = Asm::new();
        write(&mut asm);
        asm.to_asm()
    })
}

#[test]
fn test_conditional_assembly_stops_the_checks() {
    // The same section in both branches of an IF: RGBDS assembles one of them
    let both_branches = written(|asm| {
        asm.raw("IF DEF(DEBUG)")
            .section(Section::wram0("Buffers"))
            .label("wBuffer")
            .ds("16")
            .raw("ELSE")
            .section(Section::wram0("Buffers"))
            .label("wBuffer")
            .ds("8")
            .raw("ENDC");
    })
    .expect("the same section in both branches of an IF");
    assert_ne!(
        rgbds_accepts(&both_branches),
        Some(false),
        "{}",
        both_branches
    );

    // Code in a RAM section that RGBDS never assembles
    let skipped = written(|asm| {
        asm.section(Section::rom0("Code"))
            .label("Main")
            .ret()
            .raw("IF 0")
            .section(Section::wram0("Dead"))
            .nop()
            .raw("ENDC");
    })
    .expect("code in an IF 0");
    assert_ne!(rgbds_accepts(&skipped), Some(false), "{}", skipped);

    // Any block directive, in any case, after a label or in a multi-line raw text
    for line in [
        "if 1",
        "  ELIF X",
        "else",
        "EndC",
        "MyMacro: MACRO",
        "ENDM",
        "REPT 2",
        "FOR I, 3",
        "ENDR",
        "    nop\nIF 1",
    ] {
        let built = written(|asm| {
            asm.raw(line)
                .section(Section::wram0("Vars"))
                .nop()
                .section(Section::wram0("Vars"));
        });
        assert!(built.is_ok(), "{:?}: {:?}", line, built);
    }
    // A word that only starts like one is no directive: the checks go on after it
    for line in ["IFFY: db", "Format", "ENDCOUNT EQU 3"] {
        let built = written(|asm| {
            asm.raw(line).section(Section::wram0("Vars")).nop();
        });
        assert!(built.is_err(), "{:?}: {:?}", line, built);
    }
}

#[test]
fn test_a_multi_line_instruction_is_checked_as_itself() {
    // `db 1` with a line break after it is still data in RAM
    let error = written(|asm| {
        asm.section(Section::wram0("Vars")).db("1\n");
    })
    .expect_err("db 1 in WRAM0");
    assert!(
        error.contains("a RAM section holds no code or data"),
        "{}",
        error
    );
    let error = written(|asm| {
        asm.section(Section::wram0("Vars"))
            .ld(crate::gb_asm::R8::A, crate::gb_asm::Expr::raw("1\n"));
    })
    .expect_err("ld a in WRAM0");
    assert!(
        error.contains("a RAM section holds no code or data"),
        "{}",
        error
    );
    // A label or comment with a line break is fine there; the lines after it are read
    // as a raw line
    assert!(
        written(|asm| {
            asm.section(Section::wram0("Vars"))
                .comment("note\nwA: ds 1")
                .label("wB")
                .ds("1");
        })
        .is_ok()
    );
}
