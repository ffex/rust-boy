//! Every instruction family, with all the operands of the regular families (8-bit loads,
//! 8-bit ALU, `inc`/`dec`, 16-bit loads and arithmetic, the `$CB` instructions, `push`/
//! `pop`, the conditional jumps, calls and returns, `rst`) and every form of `ld` and
//! `ldh`, checked three ways: its RGBDS text, its size ([`instr_size`], which
//! `jr_range_errors` uses) and, with `RGBDS_LINK_CHECK` set, the bytes rgbasm 1.0.4
//! assembles it to, compared with the SM83 opcode table
//! (<https://gbdev.io/gb-opcodes/optables/>, gbz80(7)). Operands that hold an [`Expr`]
//! are checked with numbers, symbols (`hardware.inc` names, whose values are written
//! here by hand) and expressions whose bytes differ when a parenthesis is missing.
//!
//! [`test_ld_check_matches_the_opcode_table`] also checks [`Instr::check`] against the
//! table: every pair of `ld` / `ldh` operands it accepts is one of the encodings below,
//! and it rejects every other pair.

use super::label_check::{instr_size, rgbds_rom};
use super::{
    AluOperand, Asm, Condition, Dst, Expr, Instr, JumpTarget, Mem, Operand, R8, R16, R16Stack,
};

/// An instruction, the text it prints and the bytes of its encoding
struct Case {
    instr: Instr,
    text: String,
    bytes: Vec<u8>,
}

fn case(instr: Instr, text: &str, bytes: &[u8]) -> Case {
    Case {
        instr,
        text: text.to_string(),
        bytes: bytes.to_vec(),
    }
}

/// `ld dst, src`
fn ld(dst: impl Into<Dst>, src: impl Into<Operand>) -> Instr {
    Instr::Ld {
        dst: dst.into(),
        src: src.into(),
    }
}

/// `ldh dst, src`
fn ldh(dst: impl Into<Dst>, src: impl Into<Operand>) -> Instr {
    Instr::Ldh {
        dst: dst.into(),
        src: src.into(),
    }
}

/// A sample of the families the sweeps below do not cover, each encoding copied from
/// the opcode table by hand
fn sample() -> Vec<Case> {
    use Instr::*;
    let addr = |a| JumpTarget::Addr(a);
    vec![
        // 16-bit arithmetic
        case(AddHl { src: R16::BC }, "add hl, bc", &[0x09]),
        case(AddHl { src: R16::DE }, "add hl, de", &[0x19]),
        case(AddHl { src: R16::HL }, "add hl, hl", &[0x29]),
        case(AddHl { src: R16::SP }, "add hl, sp", &[0x39]),
        case(AddSp { offset: -3 }, "add sp, -3", &[0xE8, 0xFD]),
        case(AddSp { offset: 127 }, "add sp, 127", &[0xE8, 0x7F]),
        case(AddSp { offset: -128 }, "add sp, -128", &[0xE8, 0x80]),
        // ld hl, sp + e8
        case(LdHlSp { offset: 5 }, "ld hl, sp + 5", &[0xF8, 0x05]),
        case(LdHlSp { offset: 0 }, "ld hl, sp + 0", &[0xF8, 0x00]),
        case(LdHlSp { offset: -3 }, "ld hl, sp - 3", &[0xF8, 0xFD]),
        case(LdHlSp { offset: -128 }, "ld hl, sp - 128", &[0xF8, 0x80]),
        // Rotates on a
        case(Rlca, "rlca", &[0x07]),
        case(Rrca, "rrca", &[0x0F]),
        case(Rla, "rla", &[0x17]),
        case(Rra, "rra", &[0x1F]),
        // Flags, accumulator and CPU control
        case(Daa, "daa", &[0x27]),
        case(Cpl, "cpl", &[0x2F]),
        case(Scf, "scf", &[0x37]),
        case(Ccf, "ccf", &[0x3F]),
        case(Nop, "nop", &[0x00]),
        case(Halt, "halt", &[0x76]),
        case(Stop, "stop", &[0x10, 0x00]),
        case(Di, "di", &[0xF3]),
        case(Ei, "ei", &[0xFB]),
        // Jumps, calls and returns
        case(
            Jp {
                target: addr(0x1234),
            },
            "jp $1234",
            &[0xC3, 0x34, 0x12],
        ),
        case(JpHl, "jp hl", &[0xE9]),
        case(
            Call {
                target: addr(0x1234),
            },
            "call $1234",
            &[0xCD, 0x34, 0x12],
        ),
        case(Ret, "ret", &[0xC9]),
        case(Reti, "reti", &[0xD9]),
    ]
}

/// Every operand of the regular families, the encoding computed from the layout of the
/// opcode table: `r8` is numbered b, c, d, e, h, l, [hl], a (0 to 7, [`R8::ALL`])
fn sweep() -> Vec<Case> {
    let mut cases = Vec::new();
    // 8-bit ALU: $80 + 8 × operation + r8, and $C6 + 8 × operation for a value
    type Alu = fn(AluOperand) -> Instr;
    let alu: [(&str, Alu); 8] = [
        ("add", |src| Instr::Add { src }),
        ("adc", |src| Instr::Adc { src }),
        ("sub", |src| Instr::Sub { src }),
        ("sbc", |src| Instr::Sbc { src }),
        ("and", |src| Instr::And { src }),
        ("xor", |src| Instr::Xor { src }),
        ("or", |src| Instr::Or { src }),
        ("cp", |src| Instr::Cp { src }),
    ];
    for (op, (mnemonic, make)) in (0u8..).zip(alu) {
        for (index, r8) in (0u8..).zip(R8::ALL) {
            cases.push(case(
                make(r8.into()),
                &format!("{} a, {}", mnemonic, r8),
                &[0x80 + 8 * op + index],
            ));
        }
        cases.push(case(
            make(0x42.into()),
            &format!("{} a, 66", mnemonic),
            &[0xC6 + 8 * op, 0x42],
        ));
        // A symbol, and a negative number (-1 is $FF)
        cases.push(case(
            make("PADF_LEFT".into()),
            &format!("{} a, PADF_LEFT", mnemonic),
            &[0xC6 + 8 * op, 0x20],
        ));
        cases.push(case(
            make((-1).into()),
            &format!("{} a, -1", mnemonic),
            &[0xC6 + 8 * op, 0xFF],
        ));
    }
    // Rotates, shifts and swap: $CB, 8 × operation + r8
    type Shift = fn(R8) -> Instr;
    let shifts: [(&str, Shift); 8] = [
        ("rlc", |operand| Instr::Rlc { operand }),
        ("rrc", |operand| Instr::Rrc { operand }),
        ("rl", |operand| Instr::Rl { operand }),
        ("rr", |operand| Instr::Rr { operand }),
        ("sla", |operand| Instr::Sla { operand }),
        ("sra", |operand| Instr::Sra { operand }),
        ("swap", |operand| Instr::Swap { operand }),
        ("srl", |operand| Instr::Srl { operand }),
    ];
    for (op, (mnemonic, make)) in (0u8..).zip(shifts) {
        for (index, r8) in (0u8..).zip(R8::ALL) {
            cases.push(case(
                make(r8),
                &format!("{} {}", mnemonic, r8),
                &[0xCB, 8 * op + index],
            ));
        }
    }
    // bit / res / set: $CB, base + 8 × bit + r8
    type BitOp = fn(u8, R8) -> Instr;
    let bit_ops: [(&str, u8, BitOp); 3] = [
        ("bit", 0x40, |bit, operand| Instr::Bit { bit, operand }),
        ("res", 0x80, |bit, operand| Instr::Res { bit, operand }),
        ("set", 0xC0, |bit, operand| Instr::Set { bit, operand }),
    ];
    for (mnemonic, base, make) in bit_ops {
        for bit in 0..8 {
            for (index, r8) in (0u8..).zip(R8::ALL) {
                cases.push(case(
                    make(bit, r8),
                    &format!("{} {}, {}", mnemonic, bit, r8),
                    &[0xCB, base + 8 * bit + index],
                ));
            }
        }
    }
    // rst: $C7 + vector
    for vector in (0..=0x38).step_by(8) {
        cases.push(case(
            Instr::Rst { vector },
            &format!("rst ${:02x}", vector),
            &[0xC7 + vector],
        ));
    }
    // push / pop: $C5 / $C1 + $10 × pair (bc, de, hl, af)
    let pairs = [R16Stack::BC, R16Stack::DE, R16Stack::HL, R16Stack::AF];
    for (index, pair) in (0u8..).zip(pairs) {
        cases.push(case(
            Instr::Push { pair },
            &format!("push {}", pair),
            &[0xC5 + 0x10 * index],
        ));
        cases.push(case(
            Instr::Pop { pair },
            &format!("pop {}", pair),
            &[0xC1 + 0x10 * index],
        ));
    }
    // jp cc: $C2 + 8 × condition; call cc: $C4 + 8 × condition; ret cc: $C0 + 8 ×
    // condition (nz, z, nc, c); the calls go to `Target`, at $0000
    let conditions = [Condition::NZ, Condition::Z, Condition::NC, Condition::C];
    for (index, condition) in (0u8..).zip(conditions) {
        cases.push(case(
            Instr::JpCond {
                condition: condition.clone(),
                target: JumpTarget::Addr(0x1234),
            },
            &format!("jp {}, $1234", condition),
            &[0xC2 + 8 * index, 0x34, 0x12],
        ));
        cases.push(case(
            Instr::CallCond {
                condition: condition.clone(),
                target: JumpTarget::Label("Target".to_string()),
            },
            &format!("call {}, Target", condition),
            &[0xC4 + 8 * index, 0x00, 0x00],
        ));
        cases.push(case(
            Instr::RetCond {
                condition: condition.clone(),
            },
            &format!("ret {}", condition),
            &[0xC0 + 8 * index],
        ));
    }
    cases
}

/// `jr` and `jr cc` to `Target`, at $0000: the first instructions of the program, so the
/// offset of each (from the end of the `jr`, 2 bytes after its address) is known
fn relative_jumps() -> Vec<Case> {
    let target = || JumpTarget::Label("Target".to_string());
    let mut cases = vec![case(
        Instr::Jr { target: target() },
        "jr Target",
        &[0x18, 0xFE],
    )];
    // jr cc: $20 + 8 × condition (nz, z, nc, c)
    let conditions = [Condition::NZ, Condition::Z, Condition::NC, Condition::C];
    for (index, condition) in (0u8..).zip(conditions) {
        let offset = -(2 * (i16::from(index) + 2)); // at address 2 × (index + 1)
        cases.push(case(
            Instr::JrCond {
                condition: condition.clone(),
                target: target(),
            },
            &format!("jr {}, Target", condition),
            &[0x20 + 8 * index, offset as u8],
        ));
    }
    cases
}

/// Every form of `ld` and `ldh` (each SM83 load opcode but `ld hl, sp + e8`, which is
/// [`Instr::LdHlSp`]), and of `inc` and `dec`
fn loads_inc_dec() -> Vec<Case> {
    let mut cases = Vec::new();
    // ld r8, r8': $40 + 8 × destination + source ($76, `ld [hl], [hl]`, is `halt`)
    for (dst_index, dst) in (0u8..).zip(R8::ALL) {
        for (src_index, src) in (0u8..).zip(R8::ALL) {
            if dst == R8::AtHl && src == R8::AtHl {
                continue;
            }
            cases.push(case(
                ld(dst, src),
                &format!("ld {}, {}", dst, src),
                &[0x40 + 8 * dst_index + src_index],
            ));
        }
    }
    // ld r8, n8: $06 + 8 × r8; inc r8: $04 + 8 × r8; dec r8: $05 + 8 × r8
    for (index, r8) in (0u8..).zip(R8::ALL) {
        cases.push(case(
            ld(r8, 0x42),
            &format!("ld {}, 66", r8),
            &[0x06 + 8 * index, 0x42],
        ));
        cases.push(case(
            Instr::Inc { operand: r8.into() },
            &format!("inc {}", r8),
            &[0x04 + 8 * index],
        ));
        cases.push(case(
            Instr::Dec { operand: r8.into() },
            &format!("dec {}", r8),
            &[0x05 + 8 * index],
        ));
    }
    // ld r16, n16: $01 + $10 × r16; inc r16: $03 + $10 × r16; dec r16: $0B + $10 × r16
    for (index, r16) in (0u8..).zip(R16::ALL) {
        cases.push(case(
            ld(r16, 0x1234),
            &format!("ld {}, 4660", r16),
            &[0x01 + 0x10 * index, 0x34, 0x12],
        ));
        // An 8-bit value loaded into a pair is still a 16-bit operand: 3 bytes
        cases.push(case(
            ld(r16, 5),
            &format!("ld {}, 5", r16),
            &[0x01 + 0x10 * index, 0x05, 0x00],
        ));
        cases.push(case(
            Instr::Inc {
                operand: r16.into(),
            },
            &format!("inc {}", r16),
            &[0x03 + 0x10 * index],
        ));
        cases.push(case(
            Instr::Dec {
                operand: r16.into(),
            },
            &format!("dec {}", r16),
            &[0x0B + 0x10 * index],
        ));
    }
    // ld [r16], a: $02 + $10 × ([bc], [de], [hli], [hld]); ld a, [r16]: $0A + $10 × …
    let through = [Mem::Bc, Mem::De, Mem::Hli, Mem::Hld];
    for (index, mem) in (0u8..).zip(through) {
        cases.push(case(
            ld(mem.clone(), R8::A),
            &format!("ld {}, a", mem),
            &[0x02 + 0x10 * index],
        ));
        cases.push(case(
            ld(R8::A, mem.clone()),
            &format!("ld a, {}", mem),
            &[0x0A + 0x10 * index],
        ));
    }
    let a = || R8::A;
    cases.extend([
        // ld [n16], a / ld a, [n16]: a number and a symbol
        case(
            ld(Mem::addr(Expr::hex(0xC000)), a()),
            "ld [$C000], a",
            &[0xEA, 0x00, 0xC0],
        ),
        case(
            ld(a(), Mem::addr(Expr::hex(0xC000))),
            "ld a, [$C000]",
            &[0xFA, 0x00, 0xC0],
        ),
        case(
            ld(a(), Mem::addr("rLCDC")),
            "ld a, [rLCDC]",
            &[0xFA, 0x40, 0xFF],
        ),
        case(
            ld(Mem::addr("rLCDC"), a()),
            "ld [rLCDC], a",
            &[0xEA, 0x40, 0xFF],
        ),
        // ld [n16], sp and ld sp, hl
        case(
            ld(Mem::addr(Expr::hex(0xC000)), R16::SP),
            "ld [$C000], sp",
            &[0x08, 0x00, 0xC0],
        ),
        case(ld(R16::SP, R16::HL), "ld sp, hl", &[0xF9]),
        // ldh: [c] and an address from $FF00, a number and a symbol
        case(ldh(Mem::C, a()), "ldh [c], a", &[0xE2]),
        case(ldh(a(), Mem::C), "ldh a, [c]", &[0xF2]),
        case(
            ldh(Mem::addr(Expr::hex(0xFF80)), a()),
            "ldh [$FF80], a",
            &[0xE0, 0x80],
        ),
        case(ldh(a(), Mem::addr("rLY")), "ldh a, [rLY]", &[0xF0, 0x44]),
        case(ldh(Mem::addr("rP1"), a()), "ldh [rP1], a", &[0xE0, 0x00]),
        case(
            ldh(a(), Mem::addr(Expr::sym("_HRAM") + 2)),
            "ldh a, [_HRAM+2]",
            &[0xF0, 0x82],
        ),
    ]);
    cases
}

/// Operands that hold an [`Expr`]: numbers in every form, symbols, and expressions,
/// with the bytes RGBDS makes of them (`hardware.inc`: `_OAMRAM` $FE00, `_SCRN0` $9800,
/// `PADF_LEFT` $20, `LCDCF_ON` $80, `LCDCF_OBJON` $02, `LCDCF_BGON` $01)
fn expressions() -> Vec<Case> {
    let sym = Expr::sym;
    let ld_a = |value: Expr, text: &str, byte: u8| case(ld(R8::A, value), text, &[0x3E, byte]);
    let ld_hl = |value: Expr, text: &str, word: u16| {
        let [low, high] = word.to_le_bytes();
        case(ld(R16::HL, value), text, &[0x21, low, high])
    };
    vec![
        // Numbers
        ld_a(Expr::num(-1), "ld a, -1", 0xFF),
        ld_a(Expr::num(-128), "ld a, -128", 0x80),
        ld_a(Expr::hex(0x0F), "ld a, $0F", 0x0F),
        ld_a(Expr::bin(0b1110_0100), "ld a, %11100100", 0xE4),
        ld_a(Expr::from("0x1A"), "ld a, $1A", 0x1A),
        ld_hl(Expr::num(-2), "ld hl, -2", 0xFFFE),
        ld_hl(Expr::hex(0x9800), "ld hl, $9800", 0x9800),
        // Symbols and address arithmetic
        ld_hl(sym("_OAMRAM"), "ld hl, _OAMRAM", 0xFE00),
        ld_hl(sym("_OAMRAM") + 4 + 1, "ld hl, _OAMRAM+4+1", 0xFE05),
        ld_hl(sym("_SCRN0") + 32 * 3, "ld hl, _SCRN0+96", 0x9860),
        ld_hl(sym("_OAMRAM") - 1, "ld hl, _OAMRAM-1", 0xFDFF),
        ld_hl(sym("_OAMRAM") - "_SCRN0", "ld hl, _OAMRAM - _SCRN0", 0x6600),
        case(
            ld(Mem::addr(sym("_OAMRAM") + 5), R8::A),
            "ld [_OAMRAM+5], a",
            &[0xEA, 0x05, 0xFE],
        ),
        // LOW / HIGH
        ld_a(Expr::low(sym("_OAMRAM") + 5), "ld a, LOW(_OAMRAM+5)", 0x05),
        ld_a(Expr::high(sym("_SCRN0")), "ld a, HIGH(_SCRN0)", 0x98),
        // Flags
        ld_a(
            sym("LCDCF_ON") | "LCDCF_BGON" | "LCDCF_OBJON",
            "ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON",
            0x83,
        ),
        case(
            Instr::And {
                src: (!sym("PADF_LEFT")).into(),
            },
            "and a, ~PADF_LEFT",
            &[0xE6, 0xDF],
        ),
        // Precedence: `|` binds tighter than `+` in RGBDS, so these two differ only by
        // where the parentheses are: (3 + 1) | 4 = 4, 3 + (1 | 4) = 8
        ld_a((Expr::num(3) + 1) | 4, "ld a, (3 + 1) | 4", 4),
        ld_a(Expr::num(3) + (Expr::num(1) | 4), "ld a, 3 + 1 | 4", 8),
        // `*` binds tighter than `<<`: (1 << 2) * 3 = 12, 1 << (2 * 3) = 64
        ld_a((Expr::num(1) << 2) * 3, "ld a, (1 << 2) * 3", 12),
        ld_a(Expr::num(1) << (Expr::num(2) * 3), "ld a, 1 << 2 * 3", 64),
        // `-` on the right: 10 - (2 - 1) = 9, (10 - 2) - 1 = 7
        ld_a(Expr::num(10) - (Expr::num(2) - 1), "ld a, 10 - (2 - 1)", 9),
        ld_a(Expr::num(10) - 2 - 1, "ld a, 10 - 2 - 1", 7),
        ld_a(-(Expr::num(2) + 1), "ld a, -(2 + 1)", 0xFD),
        // Raw text, as it is
        ld_a(Expr::raw("BANK(\"Isa\")"), "ld a, BANK(\"Isa\")", 0x00),
    ]
}

/// Every case, in the order they are assembled (the relative jumps first: their
/// offsets assume it)
fn all_cases() -> Vec<Case> {
    relative_jumps()
        .into_iter()
        .chain(sample())
        .chain(sweep())
        .chain(loads_inc_dec())
        .chain(expressions())
        .collect()
}

#[test]
fn test_every_instruction_form() {
    let cases = all_cases();
    for case in &cases {
        assert_eq!(case.instr.check(), Ok(()), "{}", case.text);
        assert_eq!(case.instr.to_string(), case.text, "{:?}", case.instr);
        assert_eq!(
            instr_size(&case.instr),
            case.bytes.len(),
            "size of {}",
            case.text
        );
        // A constant value: its byte is the value `Expr::value` computes
        if let Instr::Ld {
            dst: Dst::R8(_),
            src: Operand::Imm(value),
        } = &case.instr
        {
            if let Some(v) = value.value() {
                assert_eq!(case.bytes[1], v as u8, "value of {}", case.text);
            }
        }
    }

    // The calls in the sweep go to `Target`, at $0000
    let mut asm = Asm::new();
    asm.include_hardware()
        .section("Isa", "ROM0[$0000]")
        .label("Target")
        .emit_all(cases.iter().map(|case| case.instr.clone()));
    let Some(rom) = rgbds_rom(&asm.to_asm()) else {
        return; // RGBDS_LINK_CHECK not set: the text and sizes only
    };
    let mut address = 0;
    let mut errors = Vec::new();
    for case in &cases {
        let end = address + case.bytes.len();
        let assembled = &rom[address..end];
        if assembled != case.bytes.as_slice() {
            errors.push(format!(
                "{} at ${:04x}: rgbasm {:02X?}, opcode table {:02X?}",
                case.text, address, assembled, case.bytes
            ));
        }
        address = end;
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

/// The shape of a load operand, for [`test_ld_check_matches_the_opcode_table`]: a value
/// is any value, an address any address
fn shape(operand: &Operand) -> String {
    match operand {
        Operand::Imm(_) => "n".to_string(),
        Operand::Mem(Mem::Addr(_)) => "[n]".to_string(),
        other => other.to_string(),
    }
}

#[test]
fn test_ld_check_matches_the_opcode_table() {
    // `loads_inc_dec` holds every `ld` and `ldh` opcode of the table: the 91 opcodes
    // of `ld` and `ldh` (the 92nd load, $F8, is `ld hl, sp + e8`, `Instr::LdHlSp`)
    let mut encodings = std::collections::BTreeSet::new();
    let mut opcodes = std::collections::BTreeSet::new();
    for case in loads_inc_dec() {
        let (mnemonic, dst, src) = match &case.instr {
            Instr::Ld { dst, src } => ("ld", dst, src),
            Instr::Ldh { dst, src } => ("ldh", dst, src),
            _ => continue,
        };
        encodings.insert((mnemonic, shape(&dst.clone().into()), shape(src)));
        opcodes.insert(case.bytes[0]);
    }
    let mut table: std::collections::BTreeSet<u8> =
        (0x40..=0x7F).filter(|&op| op != 0x76).collect();
    table.extend((0..8).map(|r| 0x06 + 8 * r)); // ld r8, n8
    table.extend((0..4).map(|r| 0x01 + 0x10 * r)); // ld r16, n16
    table.extend((0..4).map(|r| 0x02 + 0x10 * r)); // ld [r16], a
    table.extend((0..4).map(|r| 0x0A + 0x10 * r)); // ld a, [r16]
    table.extend([0xEA, 0xFA, 0x08, 0xF9, 0xE0, 0xF0, 0xE2, 0xF2]);
    assert_eq!(opcodes, table);
    assert_eq!(table.len(), 91);

    // Every pair of operands `Instr::check` accepts for `ld` / `ldh` is one of those
    // encodings, and every other pair is rejected: `ld [hl], [hl]`, `ld b, [de]`,
    // `ld bc, de`, `ld [hli], 5`, `ldh b, [c]`, … (`ld 1, 2` cannot even be written: a
    // value is not a `Dst`)
    let mut dsts: Vec<Dst> = R8::ALL.iter().map(|&r| Dst::from(r)).collect();
    dsts.extend(R16::ALL.iter().map(|&r| Dst::from(r)));
    for mem in [
        Mem::Bc,
        Mem::De,
        Mem::Hli,
        Mem::Hld,
        Mem::C,
        Mem::addr("wVar"),
    ] {
        dsts.push(mem.into());
    }
    let mut srcs: Vec<Operand> = dsts.iter().cloned().map(Operand::from).collect();
    srcs.push(Operand::from(5));
    let mut accepted = 0;
    for dst in &dsts {
        for src in &srcs {
            let forms = [
                ("ld", ld(dst.clone(), src.clone())),
                ("ldh", ldh(dst.clone(), src.clone())),
            ];
            for (mnemonic, instr) in forms {
                let valid = encodings.contains(&(mnemonic, shape(&dst.clone().into()), shape(src)));
                assert_eq!(
                    instr.check().is_ok(),
                    valid,
                    "{} {}, {}",
                    mnemonic,
                    dst,
                    src
                );
                accepted += usize::from(valid);
            }
        }
    }
    assert_eq!(accepted, encodings.len());
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
fn test_invalid_operands_are_rejected() {
    type Emit = fn(&mut Asm);
    let rejected: [(&str, Emit); 17] = [
        ("bit 8: the bit number must be 0 to 7", |asm| {
            asm.bit(8, R8::A);
        }),
        ("set 9: the bit number must be 0 to 7", |asm| {
            asm.set(9, R8::AtHl);
        }),
        ("rst $09: the vector must be one of", |asm| {
            asm.rst(0x09);
        }),
        ("rst $40: the vector must be one of", |asm| {
            asm.rst(0x40);
        }),
        (
            "ld [hl], [hl]: [hl] cannot be both the destination and the source",
            |asm| {
                asm.ld(R8::AtHl, R8::AtHl);
            },
        ),
        ("ld b, [de]: only a moves between a register and", |asm| {
            asm.ld(R8::B, Mem::De);
        }),
        ("ld bc, de: the only copy between 16-bit registers", |asm| {
            asm.ld(R16::BC, R16::DE);
        }),
        ("ld [hli], 5: a value can only be stored to [hl]", |asm| {
            asm.ld(Mem::Hli, 5);
        }),
        ("ld [c], a: [c] is an ldh operand", |asm| {
            asm.ld(Mem::C, R8::A);
        }),
        ("ld a, 300: the value 300 does not fit in 8 bits", |asm| {
            asm.ld(R8::A, 300);
        }),
        (
            "ld bc, 70000: the value 70000 does not fit in 16 bits",
            |asm| {
                asm.ld(R16::BC, Expr::num(70000));
            },
        ),
        ("cp a, -129: the value -129 does not fit in 8 bits", |asm| {
            asm.cp(-129);
        }),
        (
            "ldh [$80], a: the address $80 is not in $FF00 to $FFFF",
            |asm| {
                asm.ldh(Mem::addr(Expr::hex(0x80)), R8::A);
            },
        ),
        ("ldh b, [c]: ldh moves a", |asm| {
            asm.ldh(R8::B, Mem::C);
        }),
        ("ldh [hli], a: ldh takes [c] or an address", |asm| {
            asm.ldh(Mem::Hli, R8::A);
        }),
        // A register as text is never a value: use the typed operand
        ("\"c\" is a register or condition, not a symbol", |asm| {
            asm.cp("c");
        }),
        ("\"[wScore]\" is not a symbol name", |asm| {
            asm.ld(R8::A, "[wScore]");
        }),
    ];
    for (expected, emit) in rejected {
        let message = panic_message(|| emit(&mut Asm::new()));
        assert!(
            message.contains(expected),
            "{:?} does not contain {:?}",
            message,
            expected
        );
    }

    // A list of instructions is checked too, and so is an instruction built by hand when
    // it is printed
    let bad = Instr::Res {
        bit: 8,
        operand: R8::B,
    };
    let message = panic_message(|| {
        Asm::new().emit_all(vec![
            Instr::Nop,
            Instr::Res {
                bit: 8,
                operand: R8::B,
            },
        ]);
    });
    assert!(message.contains("res 8: the bit number"), "{}", message);
    let message = panic_message(move || {
        let _ = bad.to_string();
    });
    assert!(message.contains("res 8: the bit number"), "{}", message);
}

#[test]
fn test_registers_are_never_expressions() {
    // The string helpers put registers into expressions (`or_label("a", "c")`,
    // `adc_label("[hl]")`, `cp_label("b")` in the `unbricked` tutorial): `Instr::check`
    // took them for values, and `instr_size` counted 2 bytes for a 1-byte instruction.
    // Typed, a register is a register and a constant is an `Expr`.
    let mut asm = Asm::new();
    asm.or(R8::C)
        .adc(R8::AtHl)
        .cp(R8::B)
        .cp("BRICK_LEFT")
        .add(Expr::sym("DIGIT_OFFSET") + 1);
    let code = asm.get_main_instrs();
    assert_eq!(
        code,
        [
            Instr::Or {
                src: AluOperand::R8(R8::C)
            },
            Instr::Adc {
                src: AluOperand::R8(R8::AtHl)
            },
            Instr::Cp {
                src: AluOperand::R8(R8::B)
            },
            Instr::Cp {
                src: AluOperand::Imm(Expr::Sym("BRICK_LEFT".to_string()))
            },
            Instr::Add {
                src: AluOperand::Imm(Expr::sym("DIGIT_OFFSET") + 1)
            },
        ]
    );
    let sizes: Vec<usize> = code.iter().map(instr_size).collect();
    assert_eq!(sizes, [1, 1, 1, 2, 2]);
    let text: Vec<String> = code.iter().map(Instr::to_string).collect();
    assert_eq!(
        text,
        [
            "or a, c",
            "adc a, [hl]",
            "cp a, b",
            "cp a, BRICK_LEFT",
            "add a, DIGIT_OFFSET+1"
        ]
    );
}

#[test]
fn test_builders_emit_their_instruction() {
    let mut asm = Asm::new();
    asm.ld(R8::B, R8::C)
        .ld(R16::HL, "_OAMRAM")
        .ld(Mem::Hli, R8::A)
        .ld_a_addr_def("wScore")
        .ld_addr_def_a(Expr::sym("wScore") + 1)
        .ldh(Mem::addr("rP1"), R8::A)
        .inc(R8::AtHl)
        .dec(R16::SP)
        .push(R16Stack::HL)
        .pop(R16Stack::DE)
        .ld_hl_sp(-2)
        .add_sp(4)
        .add_hl(R16::SP)
        .sbc(R8::B)
        .rlca()
        .rrca()
        .rla()
        .rra()
        .rlc(R8::C)
        .rrc(R8::D)
        .rl(R8::E)
        .rr(R8::H)
        .sla(R8::L)
        .sra(R8::A)
        .bit(1, R8::B)
        .set(2, R8::C)
        .res(3, R8::D)
        .cpl()
        .scf()
        .ccf()
        .nop()
        .halt()
        .stop()
        .di()
        .ei()
        .jp_hl()
        .call_cond(Condition::Z, "Routine")
        .reti()
        .rst(0x28);
    let text: Vec<String> = asm
        .get_main_instrs()
        .iter()
        .map(|instr| instr.to_string())
        .collect();
    assert_eq!(
        text,
        [
            "ld b, c",
            "ld hl, _OAMRAM",
            "ld [hli], a",
            "ld a, [wScore]",
            "ld [wScore+1], a",
            "ldh [rP1], a",
            "inc [hl]",
            "dec sp",
            "push hl",
            "pop de",
            "ld hl, sp - 2",
            "add sp, 4",
            "add hl, sp",
            "sbc a, b",
            "rlca",
            "rrca",
            "rla",
            "rra",
            "rlc c",
            "rrc d",
            "rl e",
            "rr h",
            "sla l",
            "sra a",
            "bit 1, b",
            "set 2, c",
            "res 3, d",
            "cpl",
            "scf",
            "ccf",
            "nop",
            "halt",
            "stop",
            "di",
            "ei",
            "jp hl",
            "call z, Routine",
            "reti",
            "rst $28",
        ]
    );
}
