//! Mixer math (E04_05): DAC conversion, NR51 panning, NR50 master volume, high-pass filter. Integer only, so the output is bit-exact deterministic (decisions A_04/A_05). Not wired to the APU yet.

/// Gain from the mixer sum to i16: full scale (4 channels, volume 8) is 60 * 8 * 68 = 32640.
/// The value 68 is an implementation choice (console volume knob is not modelled).
pub const GAIN: i32 = 68;

/// High-pass filter charge factor at 32768 Hz in 16.16 fixed point: 0.999958^128 * 65536 (DMG, note 05b).
pub const HPF_FACTOR: i64 = 65185;

const HPF_SHIFT: u32 = 16;

/// Analog value of a channel scaled by 15: digital 0 -> +15, digital 15 -> -15, DAC off -> 0.
/// The fade of a switched-off DAC towards 0 is not modelled: UNKNOWN - to confirm.
pub fn analog15(digital: u8, dac_on: bool) -> i32 {
    if dac_on {
        15 - 2 * i32::from(digital)
    } else {
        0
    }
}

/// One output side. `routing` holds the NR51 bits of this side (bit 0 = channel 1 ... bit 3 =
/// channel 4), `volume` the 3-bit NR50 field (0..=7 counts as 1..=8). VIN is not mixed.
pub fn mix_side(analog: [i32; 4], routing: u8, volume: u8) -> i32 {
    let mut sum = 0;
    for (i, a) in analog.iter().enumerate() {
        if routing & (1 << i) != 0 {
            sum += a;
        }
    }
    sum * (i32::from(volume & 0x07) + 1) * GAIN
}

/// High-pass filter of one output side: out = in - capacitor; capacitor = in - out * factor.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hpf {
    cap: i64,
}

impl Hpf {
    pub fn reset(&mut self) {
        self.cap = 0;
    }

    pub fn apply(&mut self, input: i32) -> i32 {
        let input = i64::from(input);
        let out = input - self.cap;
        self.cap = input - ((out * HPF_FACTOR) >> HPF_SHIFT);
        out as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e04_05_dac_conversion_is_linear_and_inverted() {
        assert_eq!(analog15(0, true), 15);
        assert_eq!(analog15(15, true), -15);
        assert_eq!(analog15(5, true), 5);
        assert_eq!(analog15(7, true), 1);
        assert_eq!(analog15(8, true), -1);
        assert_eq!(analog15(9, false), 0);
    }

    #[test]
    fn e04_05_mix_side_applies_routing_and_volume() {
        assert_eq!(mix_side([15, 15, 15, 15], 0b1111, 7), 32640);
        assert_eq!(mix_side([15, 15, 15, 15], 0b0001, 0), 1020);
        assert_eq!(mix_side([15, 15, 15, 15], 0b0000, 7), 0);
        assert_eq!(mix_side([-15, 15, 0, 0], 0b0011, 5), 0);
        assert_eq!(mix_side([0, 0, 10, 0], 0b0100, 3), 2720);
    }

    #[test]
    fn e04_05_hpf_first_samples_of_a_constant_input() {
        let mut hpf = Hpf::default();
        assert_eq!(hpf.apply(1000), 1000);
        assert_eq!(hpf.apply(1000), 994);
        assert_eq!(hpf.apply(1000), 988);
    }

    #[test]
    fn e04_05_hpf_removes_a_constant_offset() {
        let mut hpf = Hpf::default();
        let mut last = 1;
        for _ in 0..2000 {
            last = hpf.apply(1000);
        }
        assert_eq!(last, 0);
    }

    #[test]
    fn e04_05_hpf_reset_clears_the_capacitor() {
        let mut hpf = Hpf::default();
        hpf.apply(500);
        hpf.reset();
        assert_eq!(hpf.apply(500), 500);
    }
}
