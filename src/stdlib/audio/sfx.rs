//! Hardware one-shot effects. Lengths use 256 Hz ticks (phase dependent by one tick),
//! envelopes 64 Hz ticks and sweep 128 Hz ticks. These are register settings, not Hz.
//! Serialize all audio writers: an interrupt must not replace a channel mid-snippet.
use crate::asm::{Block, Condition, Expr, Instr, LabelAllocator, Mem, R8};
use crate::hw;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Pulse1,
    Pulse2,
    Wave,
    Noise,
}
impl Channel {
    fn control(self) -> crate::hw::Symbol<u16> {
        match self {
            Self::Pulse1 => hw::NR14,
            Self::Pulse2 => hw::NR24,
            Self::Wave => hw::NR34,
            Self::Noise => hw::NR44,
        }
    }
    fn dac(self) -> crate::hw::Symbol<u16> {
        match self {
            Self::Pulse1 => hw::NR12,
            Self::Pulse2 => hw::NR22,
            Self::Wave => hw::NR30,
            Self::Noise => hw::NR42,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duty {
    Eighth,
    Quarter,
    Half,
    ThreeQuarters,
}
impl Duty {
    fn bits(self) -> u8 {
        match self {
            Self::Eighth => hw::AUDLEN_DUTY_12_5.value,
            Self::Quarter => hw::AUDLEN_DUTY_25.value,
            Self::Half => hw::AUDLEN_DUTY_50.value,
            Self::ThreeQuarters => hw::AUDLEN_DUTY_75.value,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeDirection {
    Increase,
    Decrease,
}
/// Volume 0..=15, pace 0..=7 (zero disables envelope). Zero/decrease disables the DAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envelope(u8);
impl Envelope {
    pub fn new(volume: u8, direction: EnvelopeDirection, pace: u8) -> Self {
        assert!(
            volume < 16 && pace < 8,
            "envelope volume 0..=15 and pace 0..=7"
        );
        Self(
            volume << 4
                | pace
                | match direction {
                    EnvelopeDirection::Increase => hw::AUDENV_UP.value,
                    EnvelopeDirection::Decrease => hw::AUDENV_DOWN.value,
                },
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepDirection {
    Add,
    Subtract,
}
/// Pace and shift 0..=7. Addition overflow silences CH1, even with pace zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sweep(u8);
impl Sweep {
    pub fn new(pace: u8, direction: SweepDirection, shift: u8) -> Self {
        assert!(pace < 8 && shift < 8, "sweep pace and shift must be 0..=7");
        Self(
            pace << 4
                | shift
                | match direction {
                    SweepDirection::Add => hw::AUD1SWEEP_UP.value,
                    SweepDirection::Subtract => hw::AUD1SWEEP_DOWN.value,
                },
        )
    }
}
/// CH1/CH2 settings: period 0..=2047, length 1..=64 ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseEffect {
    period: u16,
    length: u8,
    duty: Duty,
    envelope: Envelope,
}
impl PulseEffect {
    pub fn new(period: u16, length: u8, duty: Duty, envelope: Envelope) -> Self {
        assert!(
            period < 2048 && (1..=64).contains(&length),
            "pulse period 0..=2047, length 1..=64"
        );
        Self {
            period,
            length,
            duty,
            envelope,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveLevel {
    Mute,
    Full,
    Half,
    Quarter,
}
impl WaveLevel {
    fn bits(self) -> u8 {
        match self {
            Self::Mute => hw::WAVE_LEVEL_MUTE,
            Self::Full => hw::WAVE_LEVEL_FULL,
            Self::Half => hw::WAVE_LEVEL_HALF,
            Self::Quarter => hw::WAVE_LEVEL_QUARTER,
        }
    }
}
/// 32 four-bit samples, stored upper nibble first. Hardware starts playback at sample 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavePattern([u8; hw::WAVE_RAM_BYTES]);
impl WavePattern {
    pub fn from_bytes(bytes: [u8; hw::WAVE_RAM_BYTES]) -> Self {
        Self(bytes)
    }
    pub fn from_samples(samples: [u8; hw::WAVE_RAM_BYTES * 2]) -> Self {
        assert!(
            samples.iter().all(|&v| v < 16),
            "wave samples must be 0..=15"
        );
        let mut bytes = [0; hw::WAVE_RAM_BYTES];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = samples[i * 2] << 4 | samples[i * 2 + 1];
        }
        Self(bytes)
    }
    pub fn bytes(&self) -> &[u8; hw::WAVE_RAM_BYTES] {
        &self.0
    }
}
/// CH3 settings: period 0..=2047, length 1..=256 ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveEffect {
    period: u16,
    length: u16,
    level: WaveLevel,
    pattern: WavePattern,
}
impl WaveEffect {
    pub fn new(period: u16, length: u16, level: WaveLevel, pattern: WavePattern) -> Self {
        assert!(
            period < 2048 && (1..=256).contains(&length),
            "wave period 0..=2047, length 1..=256"
        );
        Self {
            period,
            length,
            level,
            pattern,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseWidth {
    Fifteen,
    Seven,
}
/// CH4: length 1..=64 ticks, shift 0..=13, divider 0..=7 (zero means 0.5).
/// Shifts 14/15 are rejected because hardware stops clocking the noise generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseEffect {
    length: u8,
    envelope: Envelope,
    polynomial: u8,
}
impl NoiseEffect {
    pub fn new(length: u8, envelope: Envelope, shift: u8, width: NoiseWidth, divider: u8) -> Self {
        assert!(
            (1..=64).contains(&length) && shift < 14 && divider < 8,
            "noise length 1..=64, shift 0..=13, divider 0..=7"
        );
        Self {
            length,
            envelope,
            polynomial: shift << 4
                | divider
                | match width {
                    NoiseWidth::Fifteen => 0,
                    NoiseWidth::Seven => hw::NOISE_WIDTH_SEVEN,
                },
        }
    }
}
/// One trigger, with hardware length/envelope/sweep. No software sequencer or mixer changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    Pulse1(PulseEffect, Sweep),
    Pulse2(PulseEffect),
    Wave(WaveEffect),
    Noise(NoiseEffect),
}
impl Sfx {
    pub fn channel(self) -> Channel {
        match self {
            Self::Pulse1(..) => Channel::Pulse1,
            Self::Pulse2(_) => Channel::Pulse2,
            Self::Wave(_) => Channel::Wave,
            Self::Noise(_) => Channel::Noise,
        }
    }
}
fn write(b: &mut Block, address: impl Into<Expr>, value: u8) {
    b.ld_a(value).ldh(Mem::addr(address), R8::A);
}
/// Stop only the selected channel by disabling its DAC. AF only; IME unchanged.
pub fn stop_channel(channel: Channel) -> Vec<Instr> {
    let mut b = Block::new();
    write(&mut b, channel.dac(), 0);
    b.into_instrs()
}
/// Replace one channel's sound. Carry clear means programmed; carry set means APU off
/// (no writes). AF only; IME unchanged. Configure routing/power separately. Wave uploads
/// stop CH3 first, avoiding active-wave access/retrigger corruption on DMG.
pub fn play_sfx(labels: &LabelAllocator, effect: &Sfx) -> Vec<Instr> {
    let off = labels.local("sfx_off");
    let done = labels.local("sfx_done");
    let mut b = Block::new();
    b.ldh(R8::A, Mem::addr(hw::NR52))
        .and(hw::AUDENA_ON)
        .jp_cond(Condition::Z, &off)
        .emit_all(stop_channel(effect.channel()));
    // Enable length before loading it. Start with the maximum length so neither
    // an extra length clock nor a regular clock during setup can expire it before
    // trigger. Load the requested duration immediately after triggering.
    write(
        &mut b,
        effect.channel().control(),
        hw::AUDHIGH_LENGTH_ON.value,
    );
    let trigger = hw::AUDHIGH_RESTART.value | hw::AUDHIGH_LENGTH_ON.value;
    match *effect {
        Sfx::Pulse1(p, sweep) => {
            write(&mut b, hw::NR10, sweep.0);
            pulse(&mut b, p, [hw::NR11, hw::NR12, hw::NR13, hw::NR14], trigger);
        }
        Sfx::Pulse2(p) => pulse(&mut b, p, [hw::NR21, hw::NR22, hw::NR23, hw::NR24], trigger),
        Sfx::Wave(w) => {
            for (i, byte) in w.pattern.0.into_iter().enumerate() {
                write(&mut b, Expr::from(hw::AUD3WAVERAM) + i as i32, byte);
            }
            write(&mut b, hw::NR31, 0);
            write(&mut b, hw::NR32, w.level.bits());
            write(&mut b, hw::NR33, w.period as u8);
            write(&mut b, hw::NR30, hw::WAVE_DAC_ON);
            write(&mut b, hw::NR34, trigger | (w.period >> 8) as u8);
            write(&mut b, hw::NR31, (256 - w.length) as u8);
        }
        Sfx::Noise(n) => {
            write(&mut b, hw::NR41, 0);
            write(&mut b, hw::NR42, n.envelope.0);
            write(&mut b, hw::NR43, n.polynomial);
            write(&mut b, hw::NR44, trigger);
            write(&mut b, hw::NR41, 64 - n.length);
        }
    }
    b.and(R8::A).jr(&done).label(&off).scf().label(&done);
    b.into_instrs()
}
fn pulse(b: &mut Block, p: PulseEffect, regs: [crate::hw::Symbol<u16>; 4], trigger: u8) {
    write(b, regs[0], p.duty.bits());
    write(b, regs[1], p.envelope.0);
    write(b, regs[2], p.period as u8);
    write(b, regs[3], trigger | (p.period >> 8) as u8);
    write(b, regs[0], p.duty.bits() | (64 - p.length));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::{Event, TestCpu};
    fn effects() -> [Sfx; 4] {
        let pulse = PulseEffect::new(
            0x567,
            23,
            Duty::Half,
            Envelope::new(12, EnvelopeDirection::Decrease, 3),
        );
        [
            Sfx::Pulse1(pulse, Sweep::new(2, SweepDirection::Subtract, 4)),
            Sfx::Pulse2(pulse),
            Sfx::Wave(WaveEffect::new(
                0x678,
                193,
                WaveLevel::Half,
                WavePattern::from_samples(std::array::from_fn(|i| (i % 16) as u8)),
            )),
            Sfx::Noise(NoiseEffect::new(
                17,
                Envelope::new(4, EnvelopeDirection::Increase, 2),
                11,
                NoiseWidth::Seven,
                5,
            )),
        ]
    }
    fn cpu(power: u8) -> TestCpu {
        let mut cpu = TestCpu::default();
        cpu.mem.insert("rNR52".into(), power);
        cpu.consts.insert("AUDENA_ON".into(), 0x80);
        cpu.b = 12;
        cpu.c = 34;
        cpu.d = 56;
        cpu.e = 78;
        cpu.h = 90;
        cpu.l = 123;
        cpu
    }
    #[test]
    fn powered_off_rejects_every_effect_without_writes_or_register_clobbers() {
        for effect in effects() {
            let mut cpu = cpu(0x0f); // Status bits alone do not mean APU enabled.
            cpu.run(&play_sfx(&LabelAllocator::new(), &effect));
            assert!(cpu.carry);
            assert!(cpu.trace.is_empty());
            assert_eq!(
                (cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l),
                (12, 34, 56, 78, 90, 123)
            );
        }
    }
    #[test]
    fn effects_write_only_their_channel_and_load_length_after_trigger() {
        let expected = [
            vec![
                ("rNR12", 0),
                ("rNR14", 0x40),
                ("rNR10", 0x2c),
                ("rNR11", 0x80),
                ("rNR12", 0xc3),
                ("rNR13", 0x67),
                ("rNR14", 0xc5),
                ("rNR11", 0xa9),
            ],
            vec![
                ("rNR22", 0),
                ("rNR24", 0x40),
                ("rNR21", 0x80),
                ("rNR22", 0xc3),
                ("rNR23", 0x67),
                ("rNR24", 0xc5),
                ("rNR21", 0xa9),
            ],
            vec![],
            vec![
                ("rNR42", 0),
                ("rNR44", 0x40),
                ("rNR41", 0),
                ("rNR42", 0x4a),
                ("rNR43", 0xbd),
                ("rNR44", 0xc0),
                ("rNR41", 47),
            ],
        ];
        for (index, effect) in effects().iter().enumerate() {
            let mut cpu = cpu(0x80);
            cpu.run(&play_sfx(&LabelAllocator::new(), effect));
            assert!(!cpu.carry);
            assert_eq!(
                (cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l),
                (12, 34, 56, 78, 90, 123)
            );
            if index != 2 {
                let writes: Vec<_> = expected[index]
                    .iter()
                    .map(|&(s, v)| Event::Write(s.into(), v))
                    .collect();
                assert_eq!(cpu.trace, writes);
            } else {
                // DAC disabled before every byte, then enabled only after loading settings.
                let mut writes = vec![
                    Event::Write("rNR30".into(), 0),
                    Event::Write("rNR34".into(), 0x40),
                ];
                for i in 0..16 {
                    writes.push(Event::Write(
                        if i == 0 {
                            "_AUD3WAVERAM".into()
                        } else {
                            format!("_AUD3WAVERAM+{i}")
                        },
                        ((i * 2 % 16) << 4 | ((i * 2 + 1) % 16)) as u8,
                    ));
                }
                for (s, v) in [
                    ("rNR31", 0),
                    ("rNR32", 0x40),
                    ("rNR33", 0x78),
                    ("rNR30", 0x80),
                    ("rNR34", 0xc6),
                    ("rNR31", 63),
                ] {
                    writes.push(Event::Write(s.into(), v));
                }
                assert_eq!(cpu.trace, writes);
            }
            cpu.trace.clear();
            cpu.run(&stop_channel(effect.channel()));
            assert_eq!(
                cpu.trace,
                [Event::Write(effect.channel().dac().name.into(), 0)]
            );
        }
    }
    #[test]
    fn boundaries_pack_correctly_and_invalid_settings_are_rejected() {
        let envelope = Envelope::new(15, EnvelopeDirection::Increase, 7);
        assert_eq!(envelope.0, 0xff);
        assert_eq!(Sweep::new(7, SweepDirection::Subtract, 7).0, 0x7f);
        let pattern = WavePattern::from_samples([15; 32]);
        assert_eq!(pattern.bytes(), &[255; 16]);
        for length in [1, 64] {
            let p = PulseEffect::new(2047, length, Duty::ThreeQuarters, envelope);
            let mut cpu = cpu(128);
            cpu.run(&play_sfx(&LabelAllocator::new(), &Sfx::Pulse2(p)));
            assert_eq!(cpu.mem["rNR21"], 0xc0 | (64 - length));
            assert_eq!(cpu.mem["rNR24"], 0xc7);
        }
        for length in [1, 256] {
            let w = WaveEffect::new(0, length, WaveLevel::Quarter, pattern);
            let mut cpu = cpu(128);
            cpu.run(&play_sfx(&LabelAllocator::new(), &Sfx::Wave(w)));
            assert_eq!(cpu.mem["rNR31"], (256 - length) as u8);
            assert_eq!(cpu.mem["rNR32"], 0x60);
        }
        for length in [0, 65, 255] {
            assert!(
                std::panic::catch_unwind(|| PulseEffect::new(0, length, Duty::Half, envelope))
                    .is_err()
            );
            assert!(
                std::panic::catch_unwind(|| NoiseEffect::new(
                    length,
                    envelope,
                    0,
                    NoiseWidth::Fifteen,
                    0
                ))
                .is_err()
            );
        }
        for period in [2048, u16::MAX] {
            assert!(
                std::panic::catch_unwind(|| PulseEffect::new(period, 1, Duty::Half, envelope))
                    .is_err()
            );
            assert!(
                std::panic::catch_unwind(|| WaveEffect::new(period, 1, WaveLevel::Full, pattern))
                    .is_err()
            );
        }
        for length in [0, 257, u16::MAX] {
            assert!(
                std::panic::catch_unwind(|| WaveEffect::new(0, length, WaveLevel::Full, pattern))
                    .is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(|| Envelope::new(16, EnvelopeDirection::Decrease, 0)).is_err()
        );
        assert!(
            std::panic::catch_unwind(|| Envelope::new(0, EnvelopeDirection::Decrease, 8)).is_err()
        );
        assert!(std::panic::catch_unwind(|| Sweep::new(8, SweepDirection::Add, 0)).is_err());
        assert!(std::panic::catch_unwind(|| Sweep::new(0, SweepDirection::Add, 8)).is_err());
        for shift in [14, 15, 255] {
            assert!(
                std::panic::catch_unwind(|| NoiseEffect::new(
                    1,
                    envelope,
                    shift,
                    NoiseWidth::Seven,
                    0
                ))
                .is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(|| NoiseEffect::new(1, envelope, 0, NoiseWidth::Seven, 8))
                .is_err()
        );
        assert!(std::panic::catch_unwind(|| WavePattern::from_samples([16; 32])).is_err());
    }
    #[test]
    fn one_tick_lengths_do_not_reload_to_maximum_on_extra_length_clock() {
        // Model the documented NRx4 extra-length clock and trigger reload, both
        // DIV-APU phases and both prior length-enable states. No waveform model.
        let envelope = Envelope::new(1, EnvelopeDirection::Decrease, 0);
        let pulse = PulseEffect::new(0, 1, Duty::Half, envelope);
        let cases = [
            (Sfx::Pulse1(pulse, Sweep::default()), hw::NR11, 64u16),
            (Sfx::Pulse2(pulse), hw::NR21, 64),
            (
                Sfx::Wave(WaveEffect::new(
                    0,
                    1,
                    WaveLevel::Full,
                    WavePattern::from_bytes([0; 16]),
                )),
                hw::NR31,
                256,
            ),
            (
                Sfx::Noise(NoiseEffect::new(1, envelope, 0, NoiseWidth::Fifteen, 0)),
                hw::NR41,
                64,
            ),
        ];
        for (effect, length_register, max) in cases {
            let mut cpu = cpu(128);
            cpu.run(&play_sfx(&LabelAllocator::new(), &effect));
            for extra_clock in [false, true] {
                for was_enabled in [false, true] {
                    for tick_at in 0..=cpu.trace.len() {
                        let mut enabled = was_enabled;
                        let mut length = 3;
                        for (boundary, event) in cpu.trace.iter().enumerate() {
                            if boundary == tick_at && enabled && length > 0 {
                                length -= 1;
                            }
                            if let Event::Write(register, value) = event {
                                if register == length_register.name {
                                    length = max - (*value as u16 & (max - 1));
                                }
                                if register == effect.channel().control().name {
                                    let now_enabled = value & 0x40 != 0;
                                    if extra_clock && !enabled && now_enabled && length != 0 {
                                        length -= 1;
                                    }
                                    if value & 0x80 != 0 && length == 0 {
                                        length = max - u16::from(extra_clock && now_enabled);
                                    }
                                    enabled = now_enabled;
                                }
                            }
                        }
                        assert_eq!(
                            length, 1,
                            "{effect:?}, extra={extra_clock}, enabled={was_enabled}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_snippets_use_unique_local_labels_and_link() {
        use crate::engine::{RustBoy, RustBoyConfig};
        let mut game = RustBoy::with_config(
            RustBoyConfig::default()
                .audio(super::super::AudioConfig::On(super::super::Mixer::default())),
        );
        let mut b = Block::new();
        for effect in effects().iter().cycle().take(8) {
            b.emit_all(game.play_sfx(effect));
        }
        game.init(crate::stdlib::flow::IfA::eq(0, b.into_instrs()));
        let asm = game.build().unwrap();
        assert_eq!(asm, game.build().unwrap());
        crate::asm::label_check::assert_links(&asm);
    }
}
