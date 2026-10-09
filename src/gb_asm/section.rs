//! First-class RGBDS sections: [`Section`], its [`MemoryType`], and the rules a program
//! follows inside one.
//!
//! A `Section` is the typed form of the `SECTION` directive: a name, a memory type, and
//! optionally a fixed address, a bank, an alignment (`ALIGN[n, offset]`) and the
//! `UNION` / `FRAGMENT` modifiers. It is built with checked methods, so it always prints
//! a `SECTION` line that RGBDS 1.0 accepts:
//!
//! ```
//! use rust_boy::gb_asm::{MemoryType, Section};
//!
//! let header = Section::rom0("Header").at(0x100);
//! assert_eq!(header.to_string(), r#"SECTION "Header", ROM0[$100]"#);
//!
//! let data = Section::romx("Level 2").bank(2).align(8);
//! assert_eq!(data.to_string(), r#"SECTION "Level 2", ROMX, BANK[2], ALIGN[8]"#);
//!
//! let shared = Section::new("Scratch", MemoryType::Wram0).union();
//! assert_eq!(shared.to_string(), r#"SECTION UNION "Scratch", WRAM0"#);
//! ```
//!
//! What RGBDS rejects panics when the section is built, with a clear message: a bank on a
//! memory type without banks (`ROM0`, `WRAM0`, `OAM`, `HRAM`) or out of its range, an
//! address outside the memory type, an alignment that the address does not have or that
//! no address of the memory type has, `UNION` in ROM, a name that is no plain string.
//!
//! **RAM sections** (every type but `ROM0` and `ROMX`) hold no code and no data: they only
//! reserve space, with labels and `ds n` (no fill value). An [`Asm`](super::Asm) program
//! panics on an instruction, a `ds n, fill`, a `db` / `dw` with values or an `INCBIN`
//! emitted in one (see [`SectionTracker`]).
//!
//! The address ranges and banks are RGBDS's defaults (no `rgblink -t`, `-w` or `-d`): with
//! `-w`, for instance, `WRAM0` would go up to `$DFFF` and `WRAMX` would not exist.

use std::fmt;
use std::ops::RangeInclusive;

use super::instr::Instr;
use super::labels::{code_lines, is_identifier};

/// The memory type of a [`Section`]: where RGBDS may place it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemoryType {
    /// `ROM0`: the fixed ROM bank, `$0000`-`$3FFF`
    Rom0,
    /// `ROMX`: a switchable ROM bank, `$4000`-`$7FFF`, banks 1 and up
    Romx,
    /// `VRAM`: video RAM, `$8000`-`$9FFF`, banks 0 and 1 (the second on Game Boy Color)
    Vram,
    /// `SRAM`: cartridge RAM, `$A000`-`$BFFF`, banks 0 to 255
    Sram,
    /// `WRAM0`: the fixed work RAM bank, `$C000`-`$CFFF`
    Wram0,
    /// `WRAMX`: a switchable work RAM bank, `$D000`-`$DFFF`, banks 1 to 7 (Game Boy Color;
    /// bank 1 on the Game Boy)
    Wramx,
    /// `OAM`: object attribute memory, `$FE00`-`$FE9F`
    Oam,
    /// `HRAM`: high RAM, `$FF80`-`$FFFE`
    Hram,
}

impl MemoryType {
    /// Every memory type, in address order
    pub const ALL: [MemoryType; 8] = [
        MemoryType::Rom0,
        MemoryType::Romx,
        MemoryType::Vram,
        MemoryType::Sram,
        MemoryType::Wram0,
        MemoryType::Wramx,
        MemoryType::Oam,
        MemoryType::Hram,
    ];

    /// The addresses a section of this type can have
    pub fn range(self) -> RangeInclusive<u16> {
        match self {
            MemoryType::Rom0 => 0x0000..=0x3FFF,
            MemoryType::Romx => 0x4000..=0x7FFF,
            MemoryType::Vram => 0x8000..=0x9FFF,
            MemoryType::Sram => 0xA000..=0xBFFF,
            MemoryType::Wram0 => 0xC000..=0xCFFF,
            MemoryType::Wramx => 0xD000..=0xDFFF,
            MemoryType::Oam => 0xFE00..=0xFE9F,
            MemoryType::Hram => 0xFF80..=0xFFFE,
        }
    }

    /// The banks a section of this type can be given (`BANK[n]`), as rgbasm checks them;
    /// `None` for a type without banks (`ROM0`, `WRAM0`, `OAM`, `HRAM`)
    pub fn banks(self) -> Option<RangeInclusive<u32>> {
        match self {
            MemoryType::Romx => Some(1..=0xFFFF),
            MemoryType::Vram => Some(0..=1),
            MemoryType::Sram => Some(0..=0xFF),
            MemoryType::Wramx => Some(1..=7),
            MemoryType::Rom0 | MemoryType::Wram0 | MemoryType::Oam | MemoryType::Hram => None,
        }
    }

    /// Whether a section of this type is ROM (`ROM0`, `ROMX`): the only ones that hold code
    /// or data. The others are RAM: they only reserve space.
    pub fn is_rom(self) -> bool {
        matches!(self, MemoryType::Rom0 | MemoryType::Romx)
    }

    /// The RGBDS keyword: `ROM0`, `ROMX`, ...
    pub fn keyword(self) -> &'static str {
        match self {
            MemoryType::Rom0 => "ROM0",
            MemoryType::Romx => "ROMX",
            MemoryType::Vram => "VRAM",
            MemoryType::Sram => "SRAM",
            MemoryType::Wram0 => "WRAM0",
            MemoryType::Wramx => "WRAMX",
            MemoryType::Oam => "OAM",
            MemoryType::Hram => "HRAM",
        }
    }
}

impl fmt::Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.keyword())
    }
}

/// Whether a [`Section`] is a plain one, a `UNION` or a `FRAGMENT`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SectionKind {
    /// `SECTION "name", ...`: a name used once
    #[default]
    Plain,
    /// `SECTION UNION "name", ...`: every section of that name starts at the same address
    /// (RAM only)
    Union,
    /// `SECTION FRAGMENT "name", ...`: the sections of that name are placed one after the
    /// other, in an order only rgblink knows
    Fragment,
}

/// `ALIGN[bits, offset]`: the section starts at an address whose low `bits` bits are
/// `offset`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Align {
    /// 0 to 16
    pub bits: u8,
    /// Below `2^bits`
    pub offset: u16,
}

impl Align {
    /// `2^bits - 1`: the bits of an address the alignment fixes
    fn mask(self) -> u32 {
        (1u32 << self.bits) - 1
    }
}

/// A typed `SECTION` directive; see the [module documentation](self)
///
/// Built with [`Section::new`] (or [`Section::rom0`], [`Section::wram0`], ...) and the
/// methods that add a fixed address ([`at`](Section::at)), a bank ([`bank`](Section::bank)),
/// an alignment ([`align`](Section::align), [`align_offset`](Section::align_offset)) or a
/// modifier ([`union`](Section::union), [`fragment`](Section::fragment)). Each one panics
/// if the section it makes is one RGBDS rejects ([`Section::check`]), so a `Section` always
/// prints a valid `SECTION` line.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Section {
    name: String,
    memory: MemoryType,
    kind: SectionKind,
    address: Option<u16>,
    bank: Option<u32>,
    align: Option<Align>,
}

impl Section {
    /// `SECTION "name", memory`
    ///
    /// # Panics
    /// If `name` has a character that a plain RGBDS string cannot hold as it is: `"`, `\`,
    /// `{` (interpolation) or a control character (a line break).
    #[track_caller]
    pub fn new(name: &str, memory: MemoryType) -> Self {
        Section {
            name: name.to_string(),
            memory,
            kind: SectionKind::Plain,
            address: None,
            bank: None,
            align: None,
        }
        .checked()
    }

    /// `SECTION "name", ROM0`
    #[track_caller]
    pub fn rom0(name: &str) -> Self {
        Section::new(name, MemoryType::Rom0)
    }

    /// `SECTION "name", ROMX`
    #[track_caller]
    pub fn romx(name: &str) -> Self {
        Section::new(name, MemoryType::Romx)
    }

    /// `SECTION "name", VRAM`
    #[track_caller]
    pub fn vram(name: &str) -> Self {
        Section::new(name, MemoryType::Vram)
    }

    /// `SECTION "name", SRAM`
    #[track_caller]
    pub fn sram(name: &str) -> Self {
        Section::new(name, MemoryType::Sram)
    }

    /// `SECTION "name", WRAM0`
    #[track_caller]
    pub fn wram0(name: &str) -> Self {
        Section::new(name, MemoryType::Wram0)
    }

    /// `SECTION "name", WRAMX`
    #[track_caller]
    pub fn wramx(name: &str) -> Self {
        Section::new(name, MemoryType::Wramx)
    }

    /// `SECTION "name", OAM`
    #[track_caller]
    pub fn oam(name: &str) -> Self {
        Section::new(name, MemoryType::Oam)
    }

    /// `SECTION "name", HRAM`
    #[track_caller]
    pub fn hram(name: &str) -> Self {
        Section::new(name, MemoryType::Hram)
    }

    /// At the fixed address `address`: `ROM0[$100]`
    ///
    /// # Panics
    /// If `address` is outside the memory type ([`MemoryType::range`]), or does not have
    /// the section's alignment.
    #[track_caller]
    pub fn at(mut self, address: u16) -> Self {
        self.address = Some(address);
        self.checked()
    }

    /// In the bank `bank`: `BANK[2]`
    ///
    /// # Panics
    /// If the memory type has no banks (`ROM0`, `WRAM0`, `OAM`, `HRAM`), or not this one
    /// ([`MemoryType::banks`]).
    #[track_caller]
    pub fn bank(mut self, bank: u32) -> Self {
        self.bank = Some(bank);
        self.checked()
    }

    /// At an address that is a multiple of `2^bits`: `ALIGN[8]` (256 bytes)
    ///
    /// # Panics
    /// If `bits` is above 16, the fixed address is not aligned, or no address of the
    /// memory type is (`HRAM` has no multiple of 256).
    #[track_caller]
    pub fn align(self, bits: u8) -> Self {
        self.align_offset(bits, 0)
    }

    /// At an address whose low `bits` bits are `offset`: `ALIGN[8, 4]`
    ///
    /// # Panics
    /// If `bits` is above 16, `offset` is not below `2^bits`, the fixed address does not
    /// have this alignment, or no address of the memory type has it.
    #[track_caller]
    pub fn align_offset(mut self, bits: u8, offset: u16) -> Self {
        self.align = Some(Align { bits, offset });
        self.checked()
    }

    /// `SECTION UNION`: every section of this name starts at the same address
    ///
    /// # Panics
    /// In ROM (`ROM0`, `ROMX`): a union only reserves space.
    #[track_caller]
    pub fn union(mut self) -> Self {
        self.kind = SectionKind::Union;
        self.checked()
    }

    /// `SECTION FRAGMENT`: the sections of this name are placed one after the other
    #[track_caller]
    pub fn fragment(mut self) -> Self {
        self.kind = SectionKind::Fragment;
        self.checked()
    }

    /// The name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The memory type
    pub fn memory(&self) -> MemoryType {
        self.memory
    }

    /// Plain, `UNION` or `FRAGMENT`
    pub fn kind(&self) -> SectionKind {
        self.kind
    }

    /// The fixed address, if any
    pub fn address(&self) -> Option<u16> {
        self.address
    }

    /// The bank, if any
    pub fn bank_number(&self) -> Option<u32> {
        self.bank
    }

    /// The alignment, if any
    pub fn alignment(&self) -> Option<Align> {
        self.align
    }

    /// `Ok` if RGBDS accepts this section, else what is wrong (see the
    /// [module documentation](self)). Always `Ok` for a section built with the methods
    /// of `Section`, which panic otherwise.
    pub fn check(&self) -> Result<(), String> {
        let error = |why: String| Err(format!("{}: {}", self, why));
        if let Some(c) = self
            .name
            .chars()
            .find(|c| matches!(c, '"' | '\\' | '{') || c.is_control())
        {
            return Err(format!(
                "section name {:?}: {:?} cannot be written in an RGBDS string as it is",
                self.name, c
            ));
        }
        let range = self.memory.range();
        if let Some(bank) = self.bank {
            match self.memory.banks() {
                None => {
                    return error(format!(
                        "{} has no banks: BANK is only for ROMX, VRAM, SRAM and WRAMX",
                        self.memory
                    ));
                }
                Some(banks) if !banks.contains(&bank) => {
                    return error(format!(
                        "{} banks are {} to {}, not {}",
                        self.memory,
                        banks.start(),
                        banks.end(),
                        bank
                    ));
                }
                Some(_) => {}
            }
        }
        if let Some(address) = self.address.filter(|address| !range.contains(address)) {
            return error(format!(
                "${:04X} is outside {} (${:04X}-${:04X})",
                address,
                self.memory,
                range.start(),
                range.end()
            ));
        }
        if let Some(align) = self.align {
            if align.bits > 16 {
                return error("the alignment must be 0 to 16 bits".to_string());
            }
            if u32::from(align.offset) > align.mask() {
                return error(format!(
                    "the alignment offset must be below 2^{} = {}",
                    align.bits,
                    align.mask() + 1
                ));
            }
            match self.address {
                Some(address) if u32::from(address) & align.mask() != u32::from(align.offset) => {
                    return error("the fixed address does not have this alignment".to_string());
                }
                Some(_) => {}
                None => {
                    // The first address of the range with these low bits
                    let start = u32::from(*range.start());
                    let mut first = (start & !align.mask()) | u32::from(align.offset);
                    if first < start {
                        first += align.mask() + 1;
                    }
                    if first > u32::from(*range.end()) {
                        return error(format!(
                            "no address of {} (${:04X}-${:04X}) has this alignment",
                            self.memory,
                            range.start(),
                            range.end()
                        ));
                    }
                }
            }
        }
        if self.kind == SectionKind::Union && self.memory.is_rom() {
            return error("a UNION cannot be in ROM: it only reserves space".to_string());
        }
        Ok(())
    }

    /// `self`, or a panic with what [`Section::check`] finds wrong
    #[track_caller]
    fn checked(self) -> Self {
        if let Err(error) = self.check() {
            panic!("invalid section: {}", error);
        }
        self
    }
}

/// `SECTION [UNION|FRAGMENT] "name", TYPE[$address], BANK[n], ALIGN[bits, offset]`, the
/// parts that are set (an offset of 0 is left out: `ALIGN[8]`)
impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifier = match self.kind {
            SectionKind::Plain => "",
            SectionKind::Union => "UNION ",
            SectionKind::Fragment => "FRAGMENT ",
        };
        write!(f, "SECTION {}\"{}\", {}", modifier, self.name, self.memory)?;
        if let Some(address) = self.address {
            write!(f, "[${:X}]", address)?;
        }
        if let Some(bank) = self.bank {
            write!(f, ", BANK[{}]", bank)?;
        }
        match self.align {
            Some(Align { bits, offset: 0 }) => write!(f, ", ALIGN[{}]", bits),
            Some(Align { bits, offset }) => write!(f, ", ALIGN[{}, {}]", bits, offset),
            None => Ok(()),
        }
    }
}

/// Where a program is, for [`SectionTracker`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// Before any `SECTION`
    Outside,
    /// In the section at this index of `opened`
    In(usize),
    /// After a line that may have changed the section (an `INCLUDE`, a raw line with a
    /// directive or a macro): nothing is checked until the next typed `SECTION`
    Unknown,
}

/// Follows the sections of a program, instruction by instruction, and rejects what does
/// not belong where it is emitted (used by [`Asm`](super::Asm)):
/// - code or data in a RAM section: an instruction, `ds n, fill`, `db` / `dw` with values,
///   `INCBIN`. A RAM section takes labels, `ds n`, `db` / `dw` without values (one byte, two
///   bytes), comments, `DEF`s;
/// - a section name used twice, unless both are `UNION`s, or both `FRAGMENT`s, of the same
///   memory type and bank.
///
/// Raw lines are never rejected (the escape hatch). A raw line whose code is only labels
/// and `db` / `dw` / `ds` keeps the current section; any other raw line with code (it may
/// be a `SECTION`, a `LOAD`, a macro that opens one), and an `INCLUDE`, make the section
/// unknown, and nothing is checked until the next typed `SECTION`.
#[derive(Clone, Debug)]
pub(crate) struct SectionTracker {
    opened: Vec<Section>,
    place: Place,
}

impl Default for SectionTracker {
    fn default() -> Self {
        SectionTracker {
            opened: Vec::new(),
            place: Place::Outside,
        }
    }
}

impl SectionTracker {
    /// Follow `instr`, emitted after everything given so far: `Err` with what is wrong if
    /// it does not belong there (and then nothing changes)
    pub(crate) fn add(&mut self, instr: &Instr) -> Result<(), String> {
        match instr {
            Instr::Section(section) => {
                if let Some(first) = self.opened.iter().find(|s| s.name == section.name) {
                    let reopens = first.kind == section.kind
                        && first.kind != SectionKind::Plain
                        && first.memory == section.memory
                        && match (first.bank, section.bank) {
                            (Some(a), Some(b)) => a == b,
                            _ => true,
                        };
                    if !reopens {
                        return Err(format!(
                            "{}: the section \"{}\" is already opened ({}); only UNIONs, or \
                             FRAGMENTs, of one memory type and bank share a name",
                            section, section.name, first
                        ));
                    }
                }
                self.opened.push(section.clone());
                self.place = Place::In(self.opened.len() - 1);
                Ok(())
            }
            Instr::Include { .. } => {
                self.place = Place::Unknown;
                Ok(())
            }
            Instr::Raw { line } => {
                if !only_reserves(line) {
                    self.place = Place::Unknown;
                }
                Ok(())
            }
            _ => match self.place {
                Place::In(index) if !self.opened[index].memory.is_rom() => {
                    if reserves_space(instr) {
                        Ok(())
                    } else {
                        let section = &self.opened[index];
                        Err(format!(
                            "`{}` in the {} section \"{}\": a RAM section holds no code or \
                             data, it only reserves space (labels, `ds n` without a fill \
                             value)",
                            instr, section.memory, section.name
                        ))
                    }
                }
                _ => Ok(()),
            },
        }
    }
}

/// Whether `instr` may be in a RAM section: it takes no room (a label, a comment, a `DEF`)
/// or only reserves it (`ds n`, a `db` / `dw` without values)
fn reserves_space(instr: &Instr) -> bool {
    match instr {
        Instr::Label { .. } | Instr::Comment { .. } | Instr::Def { .. } => true,
        Instr::Ds { fill, .. } => fill.is_none(),
        Instr::Db { values } => values.trim().is_empty(),
        Instr::Dw { value } => value.trim().is_empty(),
        _ => false,
    }
}

/// Whether the code of a raw line is only labels and `db` / `dw` / `ds` (it cannot open a
/// section): `wScore: db`, `.end:`, `ds 4`
fn only_reserves(line: &str) -> bool {
    code_lines(line).iter().all(|code| {
        let mut rest = code.trim();
        // Labels first: `Name:`, `Name::`, `.local:`, `Scope.local:`
        while let Some((label, after)) = rest.split_once(':') {
            let label = label.trim();
            let name = label.strip_prefix('.').unwrap_or(label);
            let is_label = name.split('.').all(is_identifier) && !name.is_empty();
            if !is_label {
                break;
            }
            rest = after.trim_start_matches(':').trim();
        }
        let word = rest.split_whitespace().next().unwrap_or("");
        word.is_empty() || ["db", "dw", "dl", "ds"].contains(&word.to_ascii_lowercase().as_str())
    })
}

#[cfg(test)]
mod tests;
