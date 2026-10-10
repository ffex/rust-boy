//! Tests of [`Regs`] and [`Routine`], and of the calling convention of every `stdlib`
//! routine, run on the test CPU

use super::*;
use crate::asm::test_cpu::TestCpu;
use crate::asm::{Block, Expr};
use crate::hw;
use crate::stdlib::graphics::utility::{
    get_tile_by_pixel, is_specific_tile, memcopy, wait_not_vblank, wait_vblank,
};
use crate::stdlib::inputs::update_keys;
use crate::stdlib::utility::delay;

/// What the test CPU knows of each register: its value or the address its pair holds
/// (`a` to `l`), and the Z and C flags (`f`; N and H are not modelled)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Snapshot(Vec<(Regs, String)>);

pub(crate) fn snapshot(cpu: &TestCpu) -> Snapshot {
    let pairs = [
        (R8::A, None),
        (R8::B, cpu.bc.as_ref()),
        (R8::C, cpu.bc.as_ref()),
        (R8::D, cpu.de.as_ref()),
        (R8::E, cpu.de.as_ref()),
        (R8::H, cpu.hl.as_ref()),
        (R8::L, cpu.hl.as_ref()),
    ];
    let mut state: Vec<(Regs, String)> = pairs
        .into_iter()
        .map(|(reg, pointer)| (Regs::r8(reg), format!("{:?} {:?}", cpu.known(reg), pointer)))
        .collect();
    state.push((
        Regs::F,
        format!("Z {:?} C {:?}", cpu.known_zero(), cpu.known_carry()),
    ));
    Snapshot(state)
}

/// The registers whose state differs between two snapshots (a register the model no
/// longer knows has changed)
pub(crate) fn changed(before: &Snapshot, after: &Snapshot) -> Regs {
    before
        .0
        .iter()
        .zip(&after.0)
        .filter(|(before, after)| before != after)
        .fold(Regs::NONE, |regs, ((reg, _), _)| regs | *reg)
}

/// Test CPUs with every register and flag known, set to different values
pub(crate) fn seeded_cpus() -> Vec<TestCpu> {
    [(0x11u8, false, true), (0xA4, true, false)]
        .into_iter()
        .map(|(seed, zero, carry)| {
            let mut cpu = TestCpu::default();
            cpu.a = seed;
            cpu.b = seed.wrapping_add(0x11);
            cpu.c = seed.wrapping_add(0x22);
            cpu.d = seed.wrapping_add(0x33);
            cpu.e = seed.wrapping_add(0x44);
            cpu.h = seed.wrapping_add(0x55);
            cpu.l = seed.wrapping_add(0x66);
            cpu.zero = zero;
            cpu.carry = carry;
            cpu
        })
        .collect()
}

/// Run `code` on `cpu`, and return the registers it changed
pub(crate) fn run_and_compare(cpu: &mut TestCpu, code: &[Instr]) -> Regs {
    let before = snapshot(cpu);
    cpu.run(code);
    changed(&before, &snapshot(cpu))
}

/// Check the calling convention of `routine` on the test CPU: for each case, a seeded CPU
/// (every register known) is prepared by `setup` (memory, constants, and the code that
/// loads the routine's inputs), then the routine runs (with its dependencies after it).
/// Every register it changes must be listed (returned or clobbered): the others are
/// preserved. And each register it lists must change in at least one case, so the list
/// is exact.
fn check_convention(routine: &Routine, cases: &[&dyn Fn(&mut TestCpu)]) {
    let code = routine.code_with_deps();
    let mut seen = Regs::NONE;
    for (index, setup) in cases.iter().enumerate() {
        for mut cpu in seeded_cpus() {
            setup(&mut cpu);
            let changes = run_and_compare(&mut cpu, &code);
            assert!(
                routine.changes().contains(changes),
                "{} (case {}) changes {}, but lists only {} (returns {}, clobbers {})",
                routine.name(),
                index,
                changes,
                routine.changes(),
                routine.returns(),
                routine.clobbers()
            );
            seen |= changes;
        }
    }
    assert_eq!(
        seen,
        routine.changes(),
        "{}: lists {} but changed only {} in the tests",
        routine.name(),
        routine.changes(),
        seen
    );
}

/// Run `code` (the loads of a routine's inputs) on `cpu`
fn load(cpu: &mut TestCpu, code: Block) {
    cpu.run(&code.into_instrs());
}

#[test]
fn test_memcopy_convention() {
    // Copies to WRAM, the longest one over a page boundary (so `b` and `h` change too)
    let copy = |destination: u16, length: u16| {
        move |cpu: &mut TestCpu| {
            for i in 0..length {
                cpu.mem.insert(format!("Src+{}", i), i as u8);
            }
            let mut inputs = Block::new();
            inputs
                .ld(R16::DE, "Src")
                .ld(R16::HL, Expr::hex(destination))
                .ld(R16::BC, length);
            load(cpu, inputs);
        }
    };
    let ram = hw::RAM.value;
    let (empty, three, long) = (copy(ram, 0), copy(ram, 3), copy(ram + 0xFF, 0x102));
    check_convention(&memcopy(), &[&empty, &three, &long]);
}

#[test]
fn test_vblank_waits_convention() {
    // In VBlank (LY 144 to 153) WaitVBlank returns at once; out of it, WaitNotVBlank does
    let ly = |value: u8| {
        move |cpu: &mut TestCpu| {
            cpu.mem.insert(hw::LY.name.to_string(), value);
        }
    };
    let (first, last, top, bottom) = (ly(144), ly(153), ly(0), ly(143));
    check_convention(&wait_vblank(), &[&first, &last]);
    check_convention(&wait_not_vblank(), &[&top, &bottom]);
}

#[test]
fn test_update_keys_convention() {
    let keys = |held: u8| {
        move |cpu: &mut TestCpu| {
            for flag in [hw::P1F_GET_BTN, hw::P1F_GET_DPAD, hw::P1F_GET_NONE] {
                cpu.consts.insert(flag.name.to_string(), flag.value);
            }
            cpu.mem.insert("wCurKeys".to_string(), held);
        }
    };
    let (none, some) = (keys(0x00), keys(0x21));
    check_convention(&update_keys(), &[&none, &some]);
}

#[test]
fn test_get_tile_by_pixel_convention() {
    let pixel = |x: u8, y: u8| {
        move |cpu: &mut TestCpu| {
            for addr in hw::SCRN0.value..hw::SCRN1.value {
                cpu.mem.insert(format!("${:04X}", addr), (addr % 251) as u8);
            }
            (cpu.b, cpu.c) = (x, y);
        }
    };
    let (origin, far) = (pixel(0, 0), pixel(255, 143));
    check_convention(&get_tile_by_pixel(), &[&origin, &far]);
}

#[test]
fn test_delay_convention() {
    let count = |n: u16| {
        move |cpu: &mut TestCpu| {
            let mut inputs = Block::new();
            inputs.ld(R16::BC, n);
            load(cpu, inputs);
        }
    };
    let (zero, five) = (count(0), count(5));
    check_convention(&delay(), &[&zero, &five]);
}

#[test]
fn test_is_specific_tile_convention() {
    let tile = |value: u8| {
        move |cpu: &mut TestCpu| {
            cpu.a = value;
        }
    };
    let (wall, other) = (tile(5), tile(7));
    check_convention(
        &is_specific_tile("IsWall", &["$00", "$05"]),
        &[&wall, &other],
    );
}

#[test]
fn test_a_routine_that_changes_an_unlisted_register_fails_the_check() {
    // The check itself: a routine that says it preserves `de` but changes `e`
    let mut body = Block::new();
    body.label("Liar").ld(R8::E, 0).ret();
    let liar = Routine::new("Liar", body).with_clobbers(Regs::NONE);
    let message = crate::engine::panic_message(|| check_convention(&liar, &[&|_| {}]));
    assert!(
        message.contains("Liar (case 0) changes e, but lists only none"),
        "{}",
        message
    );
}

#[test]
fn test_regs_print_pairs_by_name() {
    assert_eq!(Regs::NONE.to_string(), "none");
    assert_eq!((Regs::A | Regs::F).to_string(), "a, f");
    assert_eq!((Regs::BC | Regs::DE | Regs::HL).to_string(), "bc, de, hl");
    assert_eq!(
        (Regs::A | Regs::B | Regs::L | Regs::F).to_string(),
        "a, b, l, f"
    );
    assert_eq!(Regs::ALL.to_string(), "a, bc, de, hl, f");
    assert_eq!(format!("{:?}", Regs::HL), "Regs(hl)");
    assert_eq!(Regs::ALL - Regs::AF, Regs::BC | Regs::DE | Regs::HL);
    assert_eq!(Regs::r16(R16::DE), Regs::D | Regs::E);
    assert!(Regs::ALL.contains(Regs::HL) && !Regs::HL.contains(Regs::ALL));
    assert!(Regs::BC.intersects(Regs::C) && !Regs::BC.intersects(Regs::DE));
    assert_eq!(Regs::ALL.iter().count(), 8, "seven registers and the flags");
}

/// A routine `name` that calls each of `deps`, which it depends on
fn calling(name: &str, deps: Vec<Routine>) -> Routine {
    let mut body = Block::new();
    body.label(name);
    for dep in &deps {
        body.call(dep.name());
    }
    body.ret();
    deps.into_iter()
        .fold(Routine::new(name, body), Routine::with_dep)
}

#[test]
fn test_with_deps_lists_each_routine_once_dependencies_first() {
    let leaf = calling("Leaf", vec![delay()]);
    let middle = calling("Middle", vec![leaf.clone(), memcopy()]);
    let top = calling("Top", vec![memcopy(), middle, leaf]);
    let names: Vec<&str> = top.with_deps().iter().map(|r| r.name()).collect();
    assert_eq!(names, ["Memcopy", "Delay", "Leaf", "Middle", "Top"]);

    // The code: the routine first, then the others in that order, each once
    let code = top.code_with_deps();
    let labels: Vec<String> = code
        .iter()
        .map(|instr| instr.to_string())
        .filter(|line| line.ends_with(':') && !line.starts_with('.'))
        .collect();
    assert_eq!(labels, ["Top:", "Memcopy:", "Delay:", "Leaf:", "Middle:"]);
}

#[test]
fn test_a_routine_is_its_body() {
    let routine = delay();
    let body = routine.body().to_vec();
    let mut asm = Block::new();
    asm.emit_all(routine.clone());
    assert_eq!(asm.into_instrs(), body);
    assert_eq!(Vec::<Instr>::from(routine.clone()), body);
    let mut call = routine.call();
    let text: Vec<String> =
        crate::asm::Emittable::emit(&mut call, &crate::asm::LabelAllocator::new())
            .iter()
            .map(|instr| instr.to_string())
            .collect();
    assert_eq!(text, ["call Delay"]);
    // Unless it says otherwise, a routine may change every register
    let mut body = Block::new();
    body.label("Unknown").ret();
    assert_eq!(Routine::new("Unknown", body).changes(), Regs::ALL);
}

#[test]
fn test_routine_errors() {
    let message = crate::engine::panic_message(|| Routine::new("my routine", Vec::new()));
    assert!(
        message.contains("invalid routine name \"my routine\""),
        "{}",
        message
    );

    let mut other = Block::new();
    other.label("Other").ret();
    let message = crate::engine::panic_message(|| Routine::new("Missing", other));
    assert!(
        message.contains("routine \"Missing\": its body does not define the label `Missing:`"),
        "{}",
        message
    );

    let message = crate::engine::panic_message(|| delay().with_dep(delay()));
    assert!(message.contains("cannot depend on itself"), "{}", message);

    let first = calling("Helper", vec![]);
    let second = calling("Helper", vec![delay()]);
    let message = crate::engine::panic_message(|| {
        calling("Top", vec![first.clone()]).with_dep(second.clone())
    });
    assert!(
        message.contains("two different dependencies are named \"Helper\""),
        "{}",
        message
    );

    // Two different routines with one name further down the dependencies
    let top = calling(
        "Top",
        vec![calling("Left", vec![first]), calling("Right", vec![second])],
    );
    let message = crate::engine::panic_message(|| top.with_deps().len());
    assert!(
        message.contains("two different routines are named \"Helper\""),
        "{}",
        message
    );
}

#[test]
fn test_written_by_covers_what_each_instruction_changes() {
    // Regs::written_by is an upper bound: run each instruction on the test CPU (hl and de
    // point at memory), and every register it changed must be in what written_by says
    use crate::asm::{Condition, Mem, R16Stack};

    let cases: Vec<Block> = {
        let mut cases = Vec::new();
        let mut add = |f: &dyn Fn(&mut Block)| {
            let mut code = Block::new();
            f(&mut code);
            cases.push(code);
        };
        add(&|c| {
            c.ld(R8::B, R8::A);
        });
        add(&|c| {
            c.ld(R8::L, 7);
        });
        add(&|c| {
            c.ld(R16::DE, 0x1234);
        });
        add(&|c| {
            c.ld(R8::A, Mem::Hli);
        });
        add(&|c| {
            c.ld(Mem::Hld, R8::A);
        });
        add(&|c| {
            c.ld(R8::A, Mem::De);
        });
        add(&|c| {
            c.ld(R8::AtHl, R8::C);
        });
        add(&|c| {
            c.ldh(R8::A, Mem::addr(hw::P1));
        });
        add(&|c| {
            c.push(R16Stack::BC).pop(R16Stack::DE);
        });
        add(&|c| {
            c.push(R16Stack::AF).pop(R16Stack::HL);
        });
        add(&|c| {
            c.add(R8::B)
                .adc(3)
                .sub(R8::C)
                .sbc(1)
                .and(R8::D)
                .xor(R8::E)
                .or(9);
        });
        add(&|c| {
            c.cp(R8::H);
        });
        add(&|c| {
            c.inc(R8::C).dec(R8::B).inc(R8::AtHl);
        });
        add(&|c| {
            c.inc(R16::BC).dec(R16::DE);
        });
        add(&|c| {
            c.add_hl(R16::BC);
        });
        add(&|c| {
            c.rlca().rrca().rla().rra();
        });
        add(&|c| {
            c.rlc(R8::B)
                .rrc(R8::C)
                .rl(R8::D)
                .rr(R8::E)
                .sla(R8::A)
                .sra(R8::B)
                .swap(R8::C)
                .srl(R8::D);
        });
        add(&|c| {
            c.bit(3, R8::E).set(1, R8::B).res(7, R8::AtHl);
        });
        add(&|c| {
            c.cpl().scf().ccf().nop();
        });
        add(&|c| {
            c.jr_cond(Condition::Z, ".skip").ld(R8::C, 1).label(".skip");
        });
        cases
    };
    for code in cases {
        let written = Regs::written_by(&code).expect("plain code");
        for mut cpu in seeded_cpus() {
            cpu.mem.insert(hw::P1.name.to_string(), 0x0F);
            // hl and de point at WRAM, as numbers (their registers stay known)
            let ram = hw::RAM.value;
            for i in 0..4 {
                cpu.mem.insert(format!("${:04X}", ram + i), i as u8);
            }
            let mut pointers = Block::new();
            pointers
                .ld(R16::HL, Expr::hex(ram + 1))
                .ld(R16::DE, Expr::hex(ram));
            cpu.run(&pointers.into_instrs());
            let mut all = Block::new();
            all.label("Code").emit_all(code.iter().cloned());
            let changes = run_and_compare(&mut cpu, &all.into_instrs());
            assert!(
                written.contains(changes),
                "{:?}: changes {}, written_by says {}",
                code.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
                changes,
                written
            );
        }
    }

    // What it cannot see
    let mut call = Block::new();
    call.call("Elsewhere");
    assert_eq!(Regs::written_by(&call), None);
    let mut raw = Block::new();
    raw.raw("ld b, 1");
    assert_eq!(Regs::written_by(&raw), None);
    let mut comment = Block::new();
    comment.raw("; only a comment").comment("and another");
    assert_eq!(Regs::written_by(&comment), Some(Regs::NONE));
    let mut sp = Block::new();
    sp.inc(R16::SP);
    assert_eq!(Regs::written_by(&sp), None);
}

#[test]
fn test_written_by_does_not_guess_about_data_or_far_jumps() {
    // Review of #29: data, sections and jumps out of the code counted as writing nothing
    use crate::asm::{Condition, Section};

    type Case<'a> = (&'a str, &'a dyn Fn(&mut Block));
    let unknown: [Case; 8] = [
        ("db", &|c| {
            c.db("$3E, 1");
        }),
        ("dw", &|c| {
            c.dw("$1234");
        }),
        ("ds", &|c| {
            c.ds_fill("2", "0");
        }),
        ("INCBIN", &|c| {
            c.incbin("code.bin");
        }),
        ("SECTION", &|c| {
            c.section(Section::rom0("Elsewhere"));
        }),
        ("jp to a label outside", &|c| {
            c.jp("Outside");
        }),
        ("jr cc to a label outside", &|c| {
            c.jr_cond(Condition::NZ, ".outside");
        }),
        ("jr to @+n", &|c| {
            c.jr("@+4");
        }),
    ];
    for (what, code) in unknown {
        let mut block = Block::new();
        block.ld_a(1);
        code(&mut block);
        assert_eq!(Regs::written_by(&block), None, "{}", what);
    }

    // A jump to a label of the code stays in it; labels, comments and DEFs write nothing
    let mut inside = Block::new();
    inside
        .label(".top")
        .def("COUNT", "3")
        .comment("loop")
        .ld(R8::C, 1)
        .jr_cond(Condition::NZ, ".top")
        .jp(".top");
    assert_eq!(Regs::written_by(&inside), Some(Regs::C));
}
