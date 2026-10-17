//! Button input crossing the core/frontend boundary; the frontend drains it at frame
//! boundaries (decision A_04).

/// Current state of the DMG buttons.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Input {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
}

impl Input {
    /// Direction buttons as a nibble, 1 = pressed: bit0 right, bit1 left, bit2 up, bit3 down.
    pub fn dpad_bits(&self) -> u8 {
        u8::from(self.right)
            | (u8::from(self.left) << 1)
            | (u8::from(self.up) << 2)
            | (u8::from(self.down) << 3)
    }

    /// Action buttons as a nibble, 1 = pressed: bit0 a, bit1 b, bit2 select, bit3 start.
    pub fn action_bits(&self) -> u8 {
        u8::from(self.a)
            | (u8::from(self.b) << 1)
            | (u8::from(self.select) << 2)
            | (u8::from(self.start) << 3)
    }
}

/// Value read from P1 ($FF00), DMG (note 06). `select` is the stored value: bit 4 = 0 selects the
/// direction pad, bit 5 = 0 selects the action buttons, a pressed button reads 0 (active low),
/// both families selected are ANDed, none selected reads $F. Bits 7-6 read 1 (as in the power-up
/// value $CF; UNKNOWN - to confirm for other cases).
pub fn p1_read(select: u8, input: &Input) -> u8 {
    let mut low = 0x0F;
    if select & 0x10 == 0 {
        low &= !input.dpad_bits();
    }
    if select & 0x20 == 0 {
        low &= !input.action_bits();
    }
    0xC0 | (select & 0x30) | (low & 0x0F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e05_01_dpad_and_action_bits_follow_the_documented_order() {
        let d = |right, left, up, down| Input {
            right,
            left,
            up,
            down,
            ..Input::default()
        };
        assert_eq!(d(true, false, false, false).dpad_bits(), 1);
        assert_eq!(d(false, true, false, false).dpad_bits(), 2);
        assert_eq!(d(false, false, true, false).dpad_bits(), 4);
        assert_eq!(d(false, false, false, true).dpad_bits(), 8);
        let a = |a, b, select, start| Input {
            a,
            b,
            select,
            start,
            ..Input::default()
        };
        assert_eq!(a(true, false, false, false).action_bits(), 1);
        assert_eq!(a(false, true, false, false).action_bits(), 2);
        assert_eq!(a(false, false, true, false).action_bits(), 4);
        assert_eq!(a(false, false, false, true).action_bits(), 8);
    }

    #[test]
    fn e05_01_p1_with_no_family_selected_reads_f() {
        let all = Input {
            up: true,
            down: true,
            left: true,
            right: true,
            a: true,
            b: true,
            select: true,
            start: true,
        };
        assert_eq!(p1_read(0x30, &all), 0xFF);
    }

    #[test]
    fn e05_01_p1_with_nothing_pressed_reads_f_in_the_low_nibble() {
        assert_eq!(p1_read(0x00, &Input::default()), 0xCF);
        assert_eq!(p1_read(0x10, &Input::default()), 0xDF);
        assert_eq!(p1_read(0x20, &Input::default()), 0xEF);
    }

    #[test]
    fn e05_01_p1_dpad_selected_is_active_low_and_ignores_actions() {
        let i = Input {
            right: true,
            up: true,
            a: true,
            ..Input::default()
        };
        assert_eq!(p1_read(0x20, &i), 0xEA);
    }

    #[test]
    fn e05_01_p1_actions_selected_is_active_low_and_ignores_the_pad() {
        let i = Input {
            a: true,
            start: true,
            right: true,
            ..Input::default()
        };
        assert_eq!(p1_read(0x10, &i), 0xD6);
    }

    #[test]
    fn e05_01_p1_with_both_families_selected_ands_them() {
        let i = Input {
            right: true,
            b: true,
            ..Input::default()
        };
        assert_eq!(p1_read(0x00, &i), 0xCC);
    }
}
