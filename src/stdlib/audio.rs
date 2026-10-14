//! Audio power, mixer configuration and one-shot effects. Snippets preserve IME and all registers
//! except AF. Initialization resets the APU, silences its channels and sets the mixer;
//! it does not erase wave RAM or generate sound. Runtime power-on also resets channels.
pub mod sfx;
pub use sfx::*;

use crate::asm::{Block, Instr, Mem, R8};
use crate::hw;

/// NR50 volumes (0..=7 each, VIN disconnected) and the NR51 routing mask.
/// Zero volume is quiet, not mute; use routing zero or power off to mute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mixer {
    volume: u8,
    routing: u8,
}
impl Mixer {
    /// Left/right volume and routing bits (`hw::AUDTERM_*`). Panics above seven.
    pub fn new(left: u8, right: u8, routing: u8) -> Self {
        assert!(left < 8 && right < 8, "audio volumes must be 0..=7");
        Self {
            volume: left << 4 | right,
            routing,
        }
    }
    pub fn volume(self) -> u8 {
        self.volume
    }
    pub fn routing(self) -> u8 {
        self.routing
    }
}
impl Default for Mixer {
    fn default() -> Self {
        let routing = hw::AUDTERM_1_LEFT.value
            | hw::AUDTERM_2_LEFT.value
            | hw::AUDTERM_3_LEFT.value
            | hw::AUDTERM_4_LEFT.value
            | hw::AUDTERM_1_RIGHT.value
            | hw::AUDTERM_2_RIGHT.value
            | hw::AUDTERM_3_RIGHT.value
            | hw::AUDTERM_4_RIGHT.value;
        Self::new(7, 7, routing)
    }
}
/// Startup policy. Inherit preserves legacy output; Off/On explicitly reset audio on
/// each startup, including soft resets. On enables a silent APU ready for triggering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AudioConfig {
    #[default]
    Inherit,
    Off,
    On(Mixer),
}
/// Power off, clearing channel/mixer registers on hardware. Wave RAM is unchanged.
pub fn power_off() -> Vec<Instr> {
    let mut b = Block::new();
    b.ld(R8::A, hw::AUDENA_OFF).ldh(Mem::addr(hw::NR52), R8::A);
    b.into_instrs()
}
/// Change the mixer while powered on. Does not retrigger or reset channels.
pub fn set_mixer(mixer: Mixer) -> Vec<Instr> {
    let mut b = Block::new();
    b.ld_a(mixer.volume)
        .ldh(Mem::addr(hw::NR50), R8::A)
        .ld_a(mixer.routing)
        .ldh(Mem::addr(hw::NR51), R8::A);
    b.into_instrs()
}
/// Reset channel state and power on with a mixer. Does not start any channel.
pub fn power_on(mixer: Mixer) -> Vec<Instr> {
    let mut b = Block::new();
    b.emit_all(power_off())
        .ld(R8::A, hw::AUDENA_ON)
        .ldh(Mem::addr(hw::NR52), R8::A)
        .emit_all(set_mixer(mixer));
    b.into_instrs()
}
/// Initialize according to the startup policy, with no instructions for Inherit.
pub fn initialize(config: AudioConfig) -> Vec<Instr> {
    match config {
        AudioConfig::Inherit => vec![],
        AudioConfig::Off => power_off(),
        AudioConfig::On(mixer) => power_on(mixer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_precedes_enable_and_mixer_writes() {
        let asm: Vec<_> = power_on(Mixer::new(3, 5, hw::AUDTERM_1_LEFT.value))
            .iter()
            .map(Instr::to_string)
            .collect();
        assert_eq!(
            asm,
            [
                "ld a, AUDENA_OFF",
                "ldh [rNR52], a",
                "ld a, AUDENA_ON",
                "ldh [rNR52], a",
                "ld a, 53",
                "ldh [rNR50], a",
                "ld a, 16",
                "ldh [rNR51], a"
            ]
        );
        assert!(initialize(AudioConfig::Inherit).is_empty());
    }
    #[test]
    fn mixer_fields_are_bounded_and_do_not_enable_vin() {
        for left in 0..8 {
            for right in 0..8 {
                let mixer = Mixer::new(left, right, 0);
                assert_eq!(mixer.volume(), left << 4 | right);
                assert_eq!(
                    mixer.volume() & (hw::AUDVOL_VIN_LEFT.value | hw::AUDVOL_VIN_RIGHT.value),
                    0
                );
            }
        }
        assert!(std::panic::catch_unwind(|| Mixer::new(8, 0, 0)).is_err());
        assert!(std::panic::catch_unwind(|| Mixer::new(0, 8, 0)).is_err());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::engine::{RustBoy, RustBoyConfig};
    #[test]
    fn inherited_output_is_unchanged_and_setter_matches_builder() {
        let legacy = RustBoy::new().build().unwrap();
        assert!(!legacy.contains("rNR52"));
        let config = AudioConfig::On(Mixer::default());
        let mut setter = RustBoy::new();
        setter.set_audio(config);
        let built = setter.build().unwrap();
        assert_eq!(
            built,
            RustBoy::with_config(RustBoyConfig::default().audio(config))
                .build()
                .unwrap()
        );
        assert_eq!(built, setter.build().unwrap());
        crate::asm::label_check::assert_links(&built);
        setter.set_audio(AudioConfig::Inherit);
        assert_eq!(legacy, setter.build().unwrap());
    }
}
