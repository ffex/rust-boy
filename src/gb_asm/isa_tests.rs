//! Every instruction family, with all the operands of the regular families (8-bit loads,
//! 8-bit ALU, `inc`/`dec`, 16-bit loads and arithmetic, the `$CB` instructions, `push`/
//! `pop`, the conditional jumps, calls and returns, `rst`), checked three ways: its
//! RGBDS text, its size ([`instr_size`], which `jr_range_errors` uses) and, with
//! `RGBDS_LINK_CHECK` set, the bytes rgbasm 1.0.4 assembles it to, compared with the SM83
//! opcode table (<https://gbdev.io/gb-opcodes/optables/>, gbz80(7)). Forms that take a
//! symbol or an expression are sampled (`Operand::Label`, `Operand::AddrDef`).

use super::label_check::{instr_size, rgbds_rom};
use super::{Asm, Condition, Instr, JumpTarget, Operand, R8, R16, R16Stack, Register};

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

fn reg(r: Register) -> Operand {
    Operand::Reg(r)
}

/// A sample of every family, each encoding copied from the opcode table by hand
fn sample() -> Vec<Case> {
    use Instr::*;
    let hl = || Operand::AddrReg(Register::HL);
    let addr = |a| JumpTarget::Addr(a);
    vec![
        // 8-bit ALU: register, [hl], immediate, expression
        case(
            Add {
                src: reg(Register::B),
            },
            "add a, b",
            &[0x80],
        ),
        case(Adc { src: hl() }, "adc a, [hl]", &[0x8E]),
        case(
            Sub {
                src: Operand::Imm(8),
            },
            "sub a, 8",
            &[0xD6, 0x08],
        ),
        case(
            Sbc {
                src: reg(Register::A),
            },
            "sbc a, a",
            &[0x9F],
        ),
        case(
            Sbc {
                src: Operand::Imm(1),
            },
            "sbc a, 1",
            &[0xDE, 0x01],
        ),
        case(
            And {
                src: Operand::Label("%11110000".to_string()),
            },
            "and a, %11110000",
            &[0xE6, 0xF0],
        ),
        case(
            Xor {
                src: reg(Register::A),
            },
            "xor a, a",
            &[0xAF],
        ),
        case(
            Or {
                src: reg(Register::C),
            },
            "or a, c",
            &[0xB1],
        ),
        case(
            Cp {
                src: Operand::Imm(144),
            },
            "cp a, 144",
            &[0xFE, 0x90],
        ),
        case(Cp { src: hl() }, "cp a, [hl]", &[0xBE]),
        case(
            Inc {
                operand: reg(Register::A),
            },
            "inc a",
            &[0x3C],
        ),
        case(
            Dec {
                operand: reg(Register::BC),
            },
            "dec bc",
            &[0x0B],
        ),
        // 16-bit arithmetic
        case(AddHl { src: R16::BC }, "add hl, bc", &[0x09]),
        case(AddHl { src: R16::DE }, "add hl, de", &[0x19]),
        case(AddHl { src: R16::HL }, "add hl, hl", &[0x29]),
        case(AddHl { src: R16::SP }, "add hl, sp", &[0x39]),
        case(AddSp { offset: -3 }, "add sp, -3", &[0xE8, 0xFD]),
        case(AddSp { offset: 127 }, "add sp, 127", &[0xE8, 0x7F]),
        case(AddSp { offset: -128 }, "add sp, -128", &[0xE8, 0x80]),
        // Loads
        case(LdHlSp { offset: 5 }, "ld hl, sp + 5", &[0xF8, 0x05]),
        case(LdHlSp { offset: 0 }, "ld hl, sp + 0", &[0xF8, 0x00]),
        case(LdHlSp { offset: -3 }, "ld hl, sp - 3", &[0xF8, 0xFD]),
        case(LdHlSp { offset: -128 }, "ld hl, sp - 128", &[0xF8, 0x80]),
        case(
            Ld {
                dst: Operand::AddrRegDec(Register::HL),
                src: reg(Register::A),
            },
            "ld [hld], a",
            &[0x32],
        ),
        case(
            Ld {
                dst: reg(Register::A),
                src: Operand::AddrRegDec(Register::HL),
            },
            "ld a, [hld]",
            &[0x3A],
        ),
        case(
            Ld {
                dst: Operand::AddrRegInc(Register::HL),
                src: reg(Register::A),
            },
            "ld [hli], a",
            &[0x22],
        ),
        case(
            Ld {
                dst: reg(Register::A),
                src: Operand::AddrRegInc(Register::HL),
            },
            "ld a, [hli]",
            &[0x2A],
        ),
        case(
            Ldh {
                dst: Operand::AddrReg(Register::C),
                src: reg(Register::A),
            },
            "ldh [c], a",
            &[0xE2],
        ),
        case(
            Ldh {
                dst: reg(Register::A),
                src: Operand::AddrReg(Register::C),
            },
            "ldh a, [c]",
            &[0xF2],
        ),
        case(
            Ldh {
                dst: Operand::Addr(0xFF80),
                src: reg(Register::A),
            },
            "ldh [$ff80], a",
            &[0xE0, 0x80],
        ),
        case(
            Ldh {
                dst: reg(Register::A),
                src: Operand::AddrDef("rLY".to_string()),
            },
            "ldh a, [rLY]",
            &[0xF0, 0x44],
        ),
        case(Push { pair: R16Stack::BC }, "push bc", &[0xC5]),
        case(Pop { pair: R16Stack::AF }, "pop af", &[0xF1]),
        // Rotates, shifts and swap
        case(Rlca, "rlca", &[0x07]),
        case(Rrca, "rrca", &[0x0F]),
        case(Rla, "rla", &[0x17]),
        case(Rra, "rra", &[0x1F]),
        case(Rlc { operand: R8::B }, "rlc b", &[0xCB, 0x00]),
        case(Rrc { operand: R8::AtHl }, "rrc [hl]", &[0xCB, 0x0E]),
        case(Rl { operand: R8::A }, "rl a", &[0xCB, 0x17]),
        case(Rr { operand: R8::C }, "rr c", &[0xCB, 0x19]),
        case(Sla { operand: R8::D }, "sla d", &[0xCB, 0x22]),
        case(Sra { operand: R8::E }, "sra e", &[0xCB, 0x2B]),
        case(Swap { operand: R8::A }, "swap a", &[0xCB, 0x37]),
        case(Srl { operand: R8::AtHl }, "srl [hl]", &[0xCB, 0x3E]),
        // Bit instructions
        case(
            Bit {
                bit: 0,
                operand: R8::B,
            },
            "bit 0, b",
            &[0xCB, 0x40],
        ),
        case(
            Bit {
                bit: 7,
                operand: R8::AtHl,
            },
            "bit 7, [hl]",
            &[0xCB, 0x7E],
        ),
        case(
            Set {
                bit: 3,
                operand: R8::A,
            },
            "set 3, a",
            &[0xCB, 0xDF],
        ),
        case(
            Res {
                bit: 5,
                operand: R8::H,
            },
            "res 5, h",
            &[0xCB, 0xAC],
        ),
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
        case(
            CallCond {
                condition: Condition::NZ,
                target: addr(0x1234),
            },
            "call nz, $1234",
            &[0xC4, 0x34, 0x12],
        ),
        case(Ret, "ret", &[0xC9]),
        case(
            RetCond {
                condition: Condition::C,
            },
            "ret c",
            &[0xD8],
        ),
        case(Reti, "reti", &[0xD9]),
        case(Rst { vector: 0x00 }, "rst $00", &[0xC7]),
        case(Rst { vector: 0x38 }, "rst $38", &[0xFF]),
    ]
}

/// Every operand of the regular families, the encoding computed from the layout of the
/// opcode table: `r8` is numbered b, c, d, e, h, l, [hl], a (0 to 7, [`R8::ALL`])
fn sweep() -> Vec<Case> {
    let mut cases = Vec::new();
    // 8-bit ALU: $80 + 8 × operation + r8, and $C6 + 8 × operation for an immediate
    type Alu = fn(Operand) -> Instr;
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
                make(Operand::from(r8)),
                &format!("{} a, {}", mnemonic, r8),
                &[0x80 + 8 * op + index],
            ));
        }
        cases.push(case(
            make(Operand::Imm(0x42)),
            &format!("{} a, 66", mnemonic),
            &[0xC6 + 8 * op, 0x42],
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
    // call cc: $C4 + 8 × condition (nz, z, nc, c)
    let conditions = [Condition::NZ, Condition::Z, Condition::NC, Condition::C];
    for (index, condition) in (0u8..).zip(conditions) {
        cases.push(case(
            Instr::CallCond {
                condition: condition.clone(),
                target: JumpTarget::Label("Target".to_string()),
            },
            &format!("call {}, Target", condition),
            &[0xC4 + 8 * index, 0x00, 0x00],
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

/// The loads, `inc` / `dec` and the conditional `jp` / `ret`: every operand of the regular
/// encodings, and each of the other forms
fn loads_inc_dec_and_conditions() -> Vec<Case> {
    let mut cases = Vec::new();
    // ld r8, r8': $40 + 8 × destination + source ($76, `ld [hl], [hl]`, is `halt`)
    for (dst_index, dst) in (0u8..).zip(R8::ALL) {
        for (src_index, src) in (0u8..).zip(R8::ALL) {
            if dst == R8::AtHl && src == R8::AtHl {
                continue;
            }
            cases.push(case(
                Instr::Ld {
                    dst: Operand::from(dst),
                    src: Operand::from(src),
                },
                &format!("ld {}, {}", dst, src),
                &[0x40 + 8 * dst_index + src_index],
            ));
        }
    }
    // ld r8, n8: $06 + 8 × r8; inc r8: $04 + 8 × r8; dec r8: $05 + 8 × r8
    for (index, r8) in (0u8..).zip(R8::ALL) {
        cases.push(case(
            Instr::Ld {
                dst: Operand::from(r8),
                src: Operand::Imm(0x42),
            },
            &format!("ld {}, 66", r8),
            &[0x06 + 8 * index, 0x42],
        ));
        cases.push(case(
            Instr::Inc {
                operand: Operand::from(r8),
            },
            &format!("inc {}", r8),
            &[0x04 + 8 * index],
        ));
        cases.push(case(
            Instr::Dec {
                operand: Operand::from(r8),
            },
            &format!("dec {}", r8),
            &[0x05 + 8 * index],
        ));
    }
    // ld r16, n16: $01 + $10 × r16; inc r16: $03 + $10 × r16; dec r16: $0B + $10 × r16
    let pairs = [Register::BC, Register::DE, Register::HL, Register::SP];
    for (index, pair) in (0u8..).zip(pairs) {
        cases.push(case(
            Instr::Ld {
                dst: reg(pair.clone()),
                src: Operand::Imm16(0x1234),
            },
            &format!("ld {}, 4660", pair),
            &[0x01 + 0x10 * index, 0x34, 0x12],
        ));
        // An 8-bit value loaded into a pair is still a 16-bit operand: 3 bytes
        cases.push(case(
            Instr::Ld {
                dst: reg(pair.clone()),
                src: Operand::Imm(5),
            },
            &format!("ld {}, 5", pair),
            &[0x01 + 0x10 * index, 0x05, 0x00],
        ));
        cases.push(case(
            Instr::Inc {
                operand: reg(pair.clone()),
            },
            &format!("inc {}", pair),
            &[0x03 + 0x10 * index],
        ));
        cases.push(case(
            Instr::Dec {
                operand: reg(pair.clone()),
            },
            &format!("dec {}", pair),
            &[0x0B + 0x10 * index],
        ));
    }
    // ld through bc / de, with an address, sp
    let ld = |dst, src, text: &str, bytes: &[u8]| case(Instr::Ld { dst, src }, text, bytes);
    let a = || reg(Register::A);
    let at = |r| Operand::AddrReg(r);
    cases.extend([
        ld(at(Register::BC), a(), "ld [bc], a", &[0x02]),
        ld(at(Register::DE), a(), "ld [de], a", &[0x12]),
        ld(a(), at(Register::BC), "ld a, [bc]", &[0x0A]),
        ld(a(), at(Register::DE), "ld a, [de]", &[0x1A]),
        ld(
            a(),
            Operand::Addr(0xC000),
            "ld a, [$c000]",
            &[0xFA, 0x00, 0xC0],
        ),
        ld(
            Operand::Addr(0xC000),
            a(),
            "ld [$c000], a",
            &[0xEA, 0x00, 0xC0],
        ),
        ld(
            a(),
            Operand::AddrDef("rLCDC".to_string()),
            "ld a, [rLCDC]",
            &[0xFA, 0x40, 0xFF],
        ),
        ld(
            Operand::Addr(0xC000),
            reg(Register::SP),
            "ld [$c000], sp",
            &[0x08, 0x00, 0xC0],
        ),
        ld(reg(Register::SP), reg(Register::HL), "ld sp, hl", &[0xF9]),
        ld(
            reg(Register::HL),
            Operand::Label("Target".to_string()),
            "ld hl, Target",
            &[0x21, 0x00, 0x00],
        ),
    ]);
    // jp cc: $C2 + 8 × condition; ret cc: $C0 + 8 × condition (nz, z, nc, c)
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
            Instr::RetCond {
                condition: condition.clone(),
            },
            &format!("ret {}", condition),
            &[0xC0 + 8 * index],
        ));
    }
    cases
}

#[test]
fn test_every_instruction_form() {
    let cases: Vec<Case> = relative_jumps()
        .into_iter()
        .chain(sample())
        .chain(sweep())
        .chain(loads_inc_dec_and_conditions())
        .collect();
    for case in &cases {
        assert_eq!(case.instr.to_string(), case.text, "{:?}", case.instr);
        assert_eq!(
            instr_size(&case.instr),
            case.bytes.len(),
            "size of {}",
            case.text
        );
    }

    // The calls in the sweep go to `Target`, at $0000
    let mut asm = Asm::new();
    asm.include_hardware()
        .section("Isa", "ROM0[$0000]")
        .label("Target")
        .emit_all(cases.iter().map(|case| case.instr.clone()).collect());
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
    let rejected: [(&str, Emit); 11] = [
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
        ("and a, hl: the source of an 8-bit ALU", |asm| {
            asm.and(Operand::Reg(Register::HL));
        }),
        ("cp a, [wCount]: the source", |asm| {
            asm.cp(Operand::AddrDef("wCount".to_string()));
        }),
        (
            "ld [bcd], a: only hl can be incremented or decremented",
            |asm| {
                asm.ld(Operand::AddrRegDec(Register::BC), Operand::Reg(Register::A));
            },
        ),
        ("the destination of sub must be a", |asm| {
            asm.sub_label("hl", "bc");
        }),
        ("the destination of add must be a, hl or sp", |asm| {
            asm.add_label("b", "c");
        }),
        (
            "the offset of add sp must be a number from -128 to 127",
            |asm| {
                asm.add_label("sp", "Offset");
            },
        ),
        ("\"bc\" is not an 8-bit register or [hl]", |asm| {
            asm.swap_label("bc");
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
    assert_eq!(
        panic_message(|| {
            R16::from_name("af");
        }),
        "\"af\" is not a 16-bit register (bc, de, hl, sp)"
    );
}

#[test]
fn test_text_helpers_build_the_typed_instructions() {
    let mut asm = Asm::new();
    asm.add_label("hl", "bc")
        .add_label("sp", "-2")
        .add_label("A", "5")
        .sub_label("a", "8 + 1")
        .or_label("a", "c")
        .xor_label("a", "a")
        .adc_label("[hl]")
        .srl_label("a")
        .swap_label("[HL]");
    let label = |text: &str| Operand::Label(text.to_string());
    assert_eq!(
        asm.get_main_instrs(),
        [
            Instr::AddHl { src: R16::BC },
            Instr::AddSp { offset: -2 },
            Instr::Add { src: label("5") },
            Instr::Sub {
                src: label("8 + 1")
            },
            Instr::Or { src: label("c") },
            Instr::Xor { src: label("a") },
            Instr::Adc { src: label("[hl]") },
            Instr::Srl { operand: R8::A },
            Instr::Swap { operand: R8::AtHl },
        ]
    );
}

#[test]
fn test_builders_emit_their_instruction() {
    let mut asm = Asm::new();
    asm.push(R16Stack::HL)
        .pop(R16Stack::DE)
        .ld_hl_sp(-2)
        .add_sp(4)
        .add_hl(R16::SP)
        .sbc(Operand::Reg(Register::B))
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
