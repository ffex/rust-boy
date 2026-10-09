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
