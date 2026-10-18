//! Input mapping: keyboard and gamepad sources merged into the core Input (E07_05).
//! Opposite directions are passed through unchanged: any special rule is UNKNOWN - to confirm.
use eframe::egui::Key;
use puce8gb_core::Input;

/// The eight Game Boy buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

pub const BUTTONS: [Button; 8] = [
    Button::Up,
    Button::Down,
    Button::Left,
    Button::Right,
    Button::A,
    Button::B,
    Button::Start,
    Button::Select,
];

/// Left stick deflection (0..1) above which a direction counts as pressed.
pub const STICK_THRESHOLD: f32 = 0.5;

/// Keyboard layout: one key per button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyMap {
    pub up: Key,
    pub down: Key,
    pub left: Key,
    pub right: Key,
    pub a: Key,
    pub b: Key,
    pub start: Key,
    pub select: Key,
}

impl Default for KeyMap {
    fn default() -> Self {
        KeyMap {
            up: Key::ArrowUp,
            down: Key::ArrowDown,
            left: Key::ArrowLeft,
            right: Key::ArrowRight,
            a: Key::W,
            b: Key::X,
            start: Key::Enter,
            select: Key::Backspace,
        }
    }
}

impl KeyMap {
    /// Key bound to a button.
    pub fn key_for(&self, button: Button) -> Key {
        match button {
            Button::Up => self.up,
            Button::Down => self.down,
            Button::Left => self.left,
            Button::Right => self.right,
            Button::A => self.a,
            Button::B => self.b,
            Button::Start => self.start,
            Button::Select => self.select,
        }
    }

    /// Copy of the map with `button` bound to `key`.
    pub fn with_key(mut self, button: Button, key: Key) -> KeyMap {
        match button {
            Button::Up => self.up = key,
            Button::Down => self.down = key,
            Button::Left => self.left = key,
            Button::Right => self.right = key,
            Button::A => self.a = key,
            Button::B => self.b = key,
            Button::Start => self.start = key,
            Button::Select => self.select = key,
        }
        self
    }

    /// Binds `key` to `button`; the button that used `key` gets the old key of `button`.
    pub fn rebind(self, button: Button, key: Key) -> KeyMap {
        let old = self.key_for(button);
        let mut map = self;
        for other in BUTTONS {
            if other != button && map.key_for(other) == key {
                map = map.with_key(other, old);
            }
        }
        map.with_key(button, key)
    }
}

/// Game Boy button for a gamepad button (A on the right, B below-left, like the real layout).
pub fn button_for_pad(pad: gilrs::Button) -> Option<Button> {
    match pad {
        gilrs::Button::DPadUp => Some(Button::Up),
        gilrs::Button::DPadDown => Some(Button::Down),
        gilrs::Button::DPadLeft => Some(Button::Left),
        gilrs::Button::DPadRight => Some(Button::Right),
        gilrs::Button::East | gilrs::Button::North => Some(Button::A),
        gilrs::Button::South | gilrs::Button::West => Some(Button::B),
        gilrs::Button::Start => Some(Button::Start),
        gilrs::Button::Select => Some(Button::Select),
        _ => None,
    }
}

/// Sets a button to pressed or released.
pub fn set_button(input: &mut Input, button: Button, down: bool) {
    match button {
        Button::Up => input.up = down,
        Button::Down => input.down = down,
        Button::Left => input.left = down,
        Button::Right => input.right = down,
        Button::A => input.a = down,
        Button::B => input.b = down,
        Button::Start => input.start = down,
        Button::Select => input.select = down,
    }
}

/// Builds an Input from a "is this button down" predicate.
pub fn state_from(is_down: impl Fn(Button) -> bool) -> Input {
    let mut input = Input::default();
    for button in BUTTONS {
        set_button(&mut input, button, is_down(button));
    }
    input
}

/// Left stick as directions; y is positive when pushed up.
pub fn stick_state(x: f32, y: f32) -> Input {
    Input {
        left: x < -STICK_THRESHOLD,
        right: x > STICK_THRESHOLD,
        up: y > STICK_THRESHOLD,
        down: y < -STICK_THRESHOLD,
        ..Input::default()
    }
}

/// A button is pressed when it is pressed on either source.
pub fn merge(a: Input, b: Input) -> Input {
    Input {
        up: a.up || b.up,
        down: a.down || b.down,
        left: a.left || b.left,
        right: a.right || b.right,
        a: a.a || b.a,
        b: a.b || b.b,
        start: a.start || b.start,
        select: a.select || b.select,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_05_key_for_default_layout() {
        let map = KeyMap::default();
        assert_eq!(map.key_for(Button::Up), Key::ArrowUp);
        assert_eq!(map.key_for(Button::Down), Key::ArrowDown);
        assert_eq!(map.key_for(Button::Left), Key::ArrowLeft);
        assert_eq!(map.key_for(Button::Right), Key::ArrowRight);
        assert_eq!(map.key_for(Button::A), Key::W);
        assert_eq!(map.key_for(Button::B), Key::X);
        assert_eq!(map.key_for(Button::Start), Key::Enter);
        assert_eq!(map.key_for(Button::Select), Key::Backspace);
    }

    #[test]
    fn e07_05_button_for_pad_mapping() {
        assert_eq!(button_for_pad(gilrs::Button::DPadUp), Some(Button::Up));
        assert_eq!(button_for_pad(gilrs::Button::East), Some(Button::A));
        assert_eq!(button_for_pad(gilrs::Button::North), Some(Button::A));
        assert_eq!(button_for_pad(gilrs::Button::South), Some(Button::B));
        assert_eq!(button_for_pad(gilrs::Button::West), Some(Button::B));
        assert_eq!(button_for_pad(gilrs::Button::Start), Some(Button::Start));
        assert_eq!(button_for_pad(gilrs::Button::Select), Some(Button::Select));
        assert_eq!(button_for_pad(gilrs::Button::LeftThumb), None);
    }

    #[test]
    fn e07_05_merge_two_sources() {
        let keyboard = Input {
            up: true,
            a: true,
            ..Input::default()
        };
        let pad = Input {
            down: true,
            b: true,
            ..Input::default()
        };
        let merged = merge(keyboard, pad);
        assert!(merged.up && merged.down && merged.a && merged.b);
        assert!(!merged.left && !merged.right && !merged.start && !merged.select);
    }

    #[test]
    fn e07_05_stick_state_threshold() {
        let input = stick_state(0.6, 0.2);
        assert!(input.right);
        assert!(!input.left && !input.up && !input.down);
        let input = stick_state(-0.9, -0.7);
        assert!(input.left && input.down);
        assert!(!input.right && !input.up);
    }

    #[test]
    fn e07_05_state_from_predicate() {
        let input = state_from(|button| matches!(button, Button::A | Button::Start));
        assert!(input.a && input.start);
        assert!(!input.up && !input.b && !input.select);
    }

    #[test]
    fn e07_05_set_button_press_and_release() {
        let mut input = Input::default();
        set_button(&mut input, Button::Select, true);
        assert!(input.select);
        set_button(&mut input, Button::Select, false);
        assert!(!input.select);
    }
}
