//! Offline hUGEDriver 6.1.1 integration (hUGETracker 1.0.1 RGBDS exports).
//! Song data must stay in ROM0 or the currently mapped ROMX bank throughout a tick.
use super::{Channel, Sfx, stop_channel};
use crate::asm::{Block, Condition, Instr, LabelAllocator, Mem, R8, R16, is_identifier};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const DRIVER_VERSION: &str = "6.1.1";
pub const DRIVER_COMMIT: &str = "13a26513498637baee9997bd3cad9476224db723";
pub const BUFFER: &str = "wRustBoyMusicDriver";
pub const RAM_BASE: &str = "RustBoyMusicRamBase";
pub const BUFFER_BYTES: u16 = 100;
pub const NO_WAVE: u8 = 100;
pub const PLAYING: &str = "wRustBoyMusicPlaying";
pub const TICK: &str = "RustBoyMusicTick";
pub const START: &str = "RustBoyMusicStart";
pub const PAUSE: &str = "RustBoyMusicPause";
pub const READY: &str = "wRustBoyMusicReady";
pub const STOP: &str = "RustBoyMusicStop";
pub const RESUME: &str = "RustBoyMusicResume";
pub const RESERVE: &str = "RustBoyMusicReserve";
pub const RELEASE: &str = "RustBoyMusicRelease";
const INIT: &str = "RustBoyHuge_hUGE_init";
const DOSOUND: &str = "RustBoyHuge_hUGE_dosound";
const MUTE: &str = "RustBoyHuge_hUGE_mute_channel";
const MASK: &str = "RustBoyHuge_mute_channels";
const WAVE: &str = "RustBoyHuge_current_wave";
const DRIVER: &str = include_str!("../../../vendor/hugedriver/hUGEDriver.asm");
const NOTES: &str = include_str!("../../../vendor/hugedriver/include/hUGE.inc");
const TABLE: &str = include_str!("../../../vendor/hugedriver/include/hUGE_note_table.inc");

/// A self-contained RGBDS export. Internal globals are prefixed per descriptor,
/// so multiple exports can use the same generated pattern/instrument names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    descriptor: String,
    asm: String,
}
impl Song {
    /// Read an export now; generated assembly has no file dependency. IO errors returned.
    pub fn from_rgbds_file(path: impl AsRef<Path>, descriptor: &str) -> std::io::Result<Self> {
        Ok(Self::from_rgbds(
            &std::fs::read_to_string(path)?,
            descriptor,
        ))
    }
    /// Import a hUGETracker RGBDS export. Panics for invalid/missing descriptors or
    /// extra includes. Only the standard hUGE.inc include is expanded automatically.
    /// ROM0 and ROMX sections are retained; keep ROMX songs in the mapped bank.
    pub fn from_rgbds(asm: &str, descriptor: &str) -> Self {
        assert!(
            is_identifier(descriptor) && crate::asm::expr::check_symbol(descriptor).is_ok(),
            "invalid music descriptor"
        );
        assert!(
            !descriptor.starts_with("RustBoy"),
            "music descriptor uses reserved RustBoy prefix"
        );
        let mut cleaned = String::new();
        for line in asm.lines() {
            let code = line.split(';').next().unwrap().trim();
            let (opcode, args) = code.split_once(char::is_whitespace).unwrap_or((code, ""));
            if opcode.eq_ignore_ascii_case("include") {
                assert!(
                    matches!(
                        args.trim().to_ascii_lowercase().as_str(),
                        "\"include/huge.inc\"" | "\"huge.inc\""
                    ),
                    "music exports must be self-contained except hUGE.inc"
                );
            } else {
                cleaned.push_str(line);
                cleaned.push('\n');
            }
        }
        let globals = definitions(&cleaned);
        assert!(
            cleaned.lines().any(|line| line
                .split_once(':')
                .is_some_and(|(name, _)| name.trim() == descriptor)),
            "music descriptor missing from export"
        );
        let prefix = format!("RustBoySong_{descriptor}_");
        let names: BTreeMap<_, _> = globals
            .into_iter()
            .filter(|name| name != descriptor)
            .map(|name| {
                let renamed = format!("{prefix}{name}");
                (name, renamed)
            })
            .collect();
        let mut asm = modernize(&rename(&cleaned, &names));
        // RGBDS section names share an object-wide namespace too.
        asm = asm
            .lines()
            .map(|line| {
                if line
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("section ")
                {
                    if let Some(start) = line.find('"') {
                        return format!("{}{}{}", &line[..start + 1], prefix, &line[start + 1..]);
                    }
                }
                line.to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            descriptor: descriptor.into(),
            asm,
        }
    }
    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }
    pub fn assembly(&self) -> &str {
        &self.asm
    }
}

// RGBDS 0.9+ requires DEF before old-style EQU/EQUS/SET declarations.
fn modernize(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let words: Vec<_> = line.split_whitespace().collect();
            if words.len() > 1
                && matches!(
                    words[1].to_ascii_uppercase().as_str(),
                    "EQU" | "EQUS" | "SET"
                )
            {
                format!("DEF {}", line.trim_start())
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Identify global labels/constants/macros. Local labels are kept in their scopes.
fn definitions(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in source.lines() {
        let line = line.split(';').next().unwrap().trim();
        let words: Vec<_> = line.split_whitespace().collect();
        if let Some(first) = words.first() {
            let name = first.trim_end_matches(':');
            if first.ends_with(':') && !name.starts_with('.') {
                names.insert(name.into());
            }
            if first.eq_ignore_ascii_case("DEF") && words.len() > 2 {
                names.insert(words[1].into());
            }
            if words.len() > 1
                && matches!(
                    words[1].to_ascii_uppercase().as_str(),
                    "EQU" | "EQUS" | "SET"
                )
            {
                names.insert(name.into());
            }
            if first.eq_ignore_ascii_case("MACRO") && words.len() > 1 {
                names.insert(words[1].into());
            }
        }
    }
    names
}
// Token replacement leaves quoted text, comments and local-label components alone.
fn rename(source: &str, names: &BTreeMap<String, String>) -> String {
    let chars: Vec<_> = source.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' || chars[i] == ';' {
            let quote = chars[i] == '"';
            out.push(chars[i]);
            i += 1;
            while i < chars.len() {
                let c = chars[i];
                out.push(c);
                i += 1;
                if quote && c == '\\' && i < chars.len() {
                    out.push(chars[i]);
                    i += 1;
                    continue;
                }
                if (quote && c == '"') || (!quote && c == '\n') {
                    break;
                }
            }
        } else if chars[i].is_ascii_alphabetic() || chars[i] == '_' || chars[i] == '.' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || matches!(chars[i], '_' | '#' | '$' | '@' | '.'))
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if let Some((global, local)) = word.split_once('.') {
                if !global.is_empty() {
                    out.push_str(names.get(global).map_or(global, String::as_str));
                    out.push('.');
                    out.push_str(local);
                } else {
                    out.push_str(&word);
                }
            } else {
                out.push_str(names.get(&word).unwrap_or(&word));
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}
/// The vendored driver, expanded and namespaced for a single generated object.
/// Reserve BUFFER_BYTES at BUFFER and zero-initialized bytes PLAYING/READY first.
/// Define RAM_BASE to the numeric address of BUFFER before emitting this routine.
pub fn driver() -> Routine {
    let expanded = DRIVER
        .replace("include \"include/hardware.inc\"", "")
        .replace("include \"include/hUGE.inc\"", "")
        .replace("include \"include/hUGE_note_table.inc\"", TABLE);
    let ram_start = expanded
        .find("SECTION \"Playback variables\", WRAM0")
        .unwrap();
    let rom_start = expanded.find("SECTION \"Sound Driver\", ROM0").unwrap();
    let ram = &expanded[ram_start..rom_start];
    let mut offset = 0u16;
    let mut defs = String::new();
    let mut preview = false;
    for line in ram.lines().skip(1) {
        let code = line.split(';').next().unwrap().trim();
        if code == "IF DEF(PREVIEW_MODE)" {
            preview = true;
            continue;
        }
        if code == "ENDC" {
            preview = false;
            continue;
        }
        if preview || code.is_empty() {
            continue;
        }
        if code.contains(" equ ") || code.starts_with("EXPORT ") {
            defs.push_str(line);
            defs.push('\n');
            continue;
        }
        let tail = if let Some((label, tail)) = code.split_once(':') {
            defs.push_str(&format!("DEF {} EQU {RAM_BASE} + {offset}\n", label));
            tail.trim_start_matches(':').trim()
        } else {
            code
        };
        offset += match tail {
            "" => 0,
            "db" => 1,
            "dw" => 2,
            "ds 5" => 5,
            "ds 4" => 4,
            _ => panic!("unsupported pinned driver WRAM declaration {tail:?}"),
        };
    }
    assert_eq!(offset, BUFFER_BYTES, "pinned driver WRAM layout changed");
    let body = format!(
        "{}\n{}\n{}",
        &expanded[..ram_start],
        defs,
        &expanded[rom_start..]
    );
    let names: BTreeMap<_, _> = definitions(&expanded)
        .into_iter()
        .map(|name| {
            let renamed = format!("RustBoyHuge_{name}");
            (name, renamed)
        })
        .collect();
    let body = modernize(&rename(&body, &names))
        .replace("IF !DEF(PREVIEW_MODE)", "IF 1")
        .replace("IF DEF(PREVIEW_MODE)", "IF 0")
        .replace("IF DEF(GBDK)", "IF 0")
        .replace(
            "SECTION \"Sound Driver\", ROM0",
            "SECTION \"RustBoy music driver\", ROM0",
        );
    let mut b = Block::new();
    b.raw(&modernize(NOTES)).raw(&body);
    Routine::new(INIT, b.into_instrs())
        .with_reads(Regs::HL)
        .with_clobbers(Regs::ALL)
}
fn index(channel: Channel) -> u8 {
    match channel {
        Channel::Pulse1 => 0,
        Channel::Pulse2 => 1,
        Channel::Wave => 2,
        Channel::Noise => 3,
    }
}
/// Main-only controls. All may change AF/BC/DE/HL; IME is preserved. PLAYING=0
/// gates ticks while hUGE_init runs, so the optional VBlank clock never reenters it.
pub fn controls() -> Vec<Routine> {
    let driver = driver();
    let mut start = Block::new();
    start.label(START).xor(R8::A).ld(Mem::addr(PLAYING), R8::A);
    // A new song takes all channels; old sustained notes must not leak into blank rows.
    for channel in [
        Channel::Pulse1,
        Channel::Pulse2,
        Channel::Wave,
        Channel::Noise,
    ] {
        start.emit_all(stop_channel(channel));
    }
    start
        .call(INIT)
        .ld_a(1)
        .ld(Mem::addr(READY), R8::A)
        .ld(Mem::addr(PLAYING), R8::A)
        .ret();
    let start = Routine::new(START, start.into_instrs())
        .with_reads(Regs::HL)
        .with_dep(driver.clone())
        .with_clobbers(Regs::ALL);
    let mut tick = Block::new();
    tick.label(TICK)
        .ld(R8::A, Mem::addr(PLAYING))
        .and(R8::A)
        .ret_cond(Condition::Z)
        .ldh(R8::A, Mem::addr(hw::NR52))
        .and(hw::AUDENA_ON)
        .ret_cond(Condition::Z)
        .jp(DOSOUND);
    let tick = Routine::new(TICK, tick.into_instrs())
        .with_dep(driver.clone())
        .with_clobbers(Regs::ALL);
    let mut pause = Block::new();
    pause.label(PAUSE).xor(R8::A).ld(Mem::addr(PLAYING), R8::A);
    for channel in [
        Channel::Pulse1,
        Channel::Pulse2,
        Channel::Wave,
        Channel::Noise,
    ] {
        let skip = format!(".skip_{}", index(channel));
        pause
            .ld(R8::A, Mem::addr(MASK))
            .and(1u8 << index(channel))
            .jr_cond(Condition::NZ, &skip)
            .emit_all(stop_channel(channel))
            .label(&skip);
    }
    pause.ret();
    let pause = Routine::new(PAUSE, pause.into_instrs())
        .with_dep(driver.clone())
        .with_clobbers(Regs::AF);
    let mut resume = Block::new();
    resume
        .label(RESUME)
        .ld(R8::A, Mem::addr(READY))
        .and(R8::A)
        .jr_cond(Condition::Z, ".stopped")
        .ld_a(1)
        .ld(Mem::addr(PLAYING), R8::A)
        .and(R8::A)
        .ret()
        .label(".stopped")
        .scf()
        .ret();
    let resume = Routine::new(RESUME, resume.into_instrs()).with_clobbers(Regs::AF);
    let mut stop = Block::new();
    stop.label(STOP)
        .call(PAUSE)
        .xor(R8::A)
        .ld(Mem::addr(READY), R8::A)
        .ret();
    let stop = Routine::new(STOP, stop.into_instrs())
        .with_dep(pause.clone())
        .with_clobbers(Regs::AF);
    let mut reserve = Block::new();
    reserve
        .label(RESERVE)
        .ld(R8::B, R8::A)
        .ld(R8::C, 1)
        .jp(MUTE);
    let reserve = Routine::new(RESERVE, reserve.into_instrs())
        .with_reads(Regs::A)
        .with_dep(driver.clone())
        .with_clobbers(Regs::ALL);
    let mut release = Block::new();
    release
        .label(RELEASE)
        .ld(R8::B, R8::A)
        .cp_imm(index(Channel::Wave))
        .jr_cond(Condition::NZ, ".unmute")
        .ld_a(NO_WAVE)
        .ld(Mem::addr(WAVE), R8::A)
        .label(".unmute")
        .ld(R8::C, 0)
        .jp(MUTE);
    let release = Routine::new(RELEASE, release.into_instrs())
        .with_reads(Regs::A)
        .with_dep(driver)
        .with_clobbers(Regs::ALL);
    vec![start, tick, pause, resume, stop, reserve, release]
}
/// Load a descriptor in HL and start it; engine validates the song belongs to it.
pub fn start(song: &Song) -> Vec<Instr> {
    let mut b = Block::new();
    b.ld(R16::HL, song.descriptor()).call(START);
    b.into_instrs()
}
pub fn reserve(channel: Channel) -> Vec<Instr> {
    channel_call(RESERVE, channel)
}
pub fn release(channel: Channel) -> Vec<Instr> {
    channel_call(RELEASE, channel)
}
fn channel_call(name: &str, channel: Channel) -> Vec<Instr> {
    let mut b = Block::new();
    b.ld_a(index(channel)).call(name);
    b.into_instrs()
}
/// Effects may play only on channels reserved from the music driver. Carry set means
/// channel not reserved or APU off. AF only, IME unchanged. Release CH3 invalidates its wave cache.
pub fn play_sfx(labels: &LabelAllocator, effect: &Sfx) -> Vec<Instr> {
    let rejected = labels.local("sfx_unreserved");
    let done = labels.local("music_sfx_done");
    let mut b = Block::new();
    b.ld(R8::A, Mem::addr(MASK))
        .and(1u8 << index(effect.channel()))
        .jr_cond(Condition::Z, &rejected)
        .emit_all(super::play_sfx(labels, effect))
        .jr(&done)
        .label(&rejected)
        .scf()
        .label(&done);
    b.into_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{MusicClock, RustBoy};
    const DEMO: &str = include_str!("../../../examples/music/song.asm");
    fn song(name: &str) -> Song {
        Song::from_rgbds(&DEMO.replace("DemoSong", name), name)
    }
    #[test]
    fn import_namespaces_globals_sections_and_preserves_notes_and_comments() {
        let song = Song::from_rgbds(
            "INCLUDE \"hUGE.inc\"\nSECTION \"Song\", ROM0\nTune::\n dw P0, Tune.local\n.local: db C#3\nP0: db 1 ; P0 untouched\nTICKS EQU 2\n",
            "Tune",
        );
        assert!(
            song.assembly()
                .contains("SECTION \"RustBoySong_Tune_Song\"")
        );
        assert!(
            song.assembly()
                .contains("dw RustBoySong_Tune_P0, Tune.local")
        );
        assert!(song.assembly().contains("db C#3"));
        assert!(song.assembly().contains("; P0 untouched"));
        assert!(song.assembly().contains("DEF RustBoySong_Tune_TICKS EQU 2"));
        assert!(!song.assembly().contains("INCLUDE"));
        assert!(Song::from_rgbds_file("examples/music/song.asm", "DemoSong").is_ok());
        assert_eq!(
            Song::from_rgbds("INCLUDE\t\"hUGE.inc\"\nTune: db 0", "Tune").assembly(),
            "Tune: db 0"
        );
        assert!(Song::from_rgbds_file("examples/music/missing.asm", "Missing").is_err());
        for name in ["", "a", "bad name", "RustBoyTune"] {
            assert!(std::panic::catch_unwind(|| Song::from_rgbds("Tune: db 0", name)).is_err());
        }
        assert!(
            std::panic::catch_unwind(|| Song::from_rgbds(
                "Tune: db 0\nINCLUDE \"other.inc\"",
                "Tune"
            ))
            .is_err()
        );
        assert!(std::panic::catch_unwind(|| Song::from_rgbds("DEF Tune EQU 5", "Tune")).is_err());
    }
    #[test]
    fn driver_is_single_offline_copy_with_explicit_100_byte_layout() {
        let text = driver()
            .body()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains("include \"include/"));
        assert!(text.contains("DEF RustBoyHuge_end_zero EQU RustBoyMusicRamBase + 100"));
        assert!(text.contains("DEF RustBoyHuge_mute_channels EQU RustBoyMusicRamBase + 29"));
        assert!(text.contains("DEF RustBoyHuge_row EQU RustBoyMusicRamBase + 34"));
        assert!(!text.contains("IF DEF(GBDK)"));
        assert!(!text.contains("IF DEF(PREVIEW_MODE)"));
        assert_eq!(driver(), driver());
    }
    #[test]
    fn multiple_songs_link_deterministically_without_moving_variables() {
        let mut game = RustBoy::new();
        let value = game.vars.create_u8("wUser", 17);
        let before = game.vars.get_address(value.id());
        game.enable_music(song("First"), MusicClock::MainFrame)
            .add_music_song(song("Second"));
        game.init(game.start_music("Second"));
        let asm = game.build().unwrap();
        assert_eq!(game.vars.get_address(value.id()), before);
        assert_eq!(asm, game.build().unwrap());
        assert_eq!(asm.matches("RustBoyHuge_hUGE_init::").count(), 1);
        assert!(asm.contains("RustBoySong_First_order1:"));
        assert!(asm.contains("RustBoySong_Second_order1:"));
        // RGBDS expands upstream macros; the lightweight label checker cannot.
        crate::asm::label_check::rgbds_rom(&asm);
    }
    #[test]
    fn controller_in_handlers_is_rejected_transitively_for_both_clocks() {
        for clock in [MusicClock::MainFrame, MusicClock::VBlank] {
            for target in [
                START, PAUSE, RESUME, STOP, RESERVE, RELEASE, TICK, INIT, DOSOUND, MUTE,
            ] {
                let mut game = RustBoy::new();
                game.enable_music(song("DemoSong"), clock);
                let mut function = Block::new();
                function.label("UserControl").call(target).ret();
                game.define_function("UserControl", function.into_instrs());
                let mut handler = Block::new();
                handler.call("UserControl");
                game.on_vblank(handler.into_instrs());
                assert!(
                    matches!(game.build(), Err(crate::engine::Error::MainLoopOnly { .. })),
                    "{target}"
                );
            }
        }
    }
    #[test]
    fn driver_memory_is_accounted_for_and_reserved_names_cannot_be_taken() {
        let mut game = RustBoy::new();
        game.enable_music(song("DemoSong"), MusicClock::MainFrame);
        for i in 0..2048 {
            game.vars.create_u16(&format!("wV{i}"), 0);
        }
        assert!(matches!(
            game.build(),
            Err(crate::engine::Error::MemoryFull { .. })
        ));
        let mut game = RustBoy::new();
        game.vars.create_u8(BUFFER, 0);
        game.enable_music(song("DemoSong"), MusicClock::MainFrame);
        assert!(matches!(
            game.build(),
            Err(crate::engine::Error::NameConflict { .. })
        ));
        for place in 0..3 {
            let mut game = RustBoy::new();
            game.enable_music(song("DemoSong"), MusicClock::MainFrame);
            match place {
                0 => {
                    game.define_const(RAM_BASE, 0);
                }
                1 => {
                    game.vars.create_u8(RAM_BASE, 0);
                }
                _ => {
                    game.external_symbol(RAM_BASE);
                }
            }
            assert!(matches!(
                game.build(),
                Err(crate::engine::Error::NameConflict { .. })
            ));
        }
        assert!(
            std::panic::catch_unwind(|| {
                let mut g = RustBoy::new();
                g.enable_music(song("DemoSong"), MusicClock::MainFrame);
                g.add_music_song(song("DemoSong"));
            })
            .is_err()
        );
    }
}
