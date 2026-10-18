//! Gamepad polling with gilrs; button state is tracked from the events, not from gilrs cached state (E07_05).
use crate::input_map::{button_for_pad, merge, set_button, stick_state};
use gilrs::{Axis, EventType, GamepadId, Gilrs};
use puce8gb_core::Input;
use std::collections::HashMap;

/// Analog button values above this count as pressed.
const PRESS_LEVEL: f32 = 0.5;

/// Held buttons and left stick position of one gamepad.
#[derive(Debug, Default, Clone, Copy)]
struct PadState {
    buttons: Input,
    stick_x: f32,
    stick_y: f32,
}

fn set_pad_button(
    states: &mut HashMap<GamepadId, PadState>,
    id: GamepadId,
    button: gilrs::Button,
    down: bool,
) {
    if let Some(mapped) = button_for_pad(button) {
        set_button(&mut states.entry(id).or_default().buttons, mapped, down);
    }
}

/// All connected gamepads, polled once per UI frame.
pub struct Pads {
    gilrs: Option<Gilrs>,
    states: HashMap<GamepadId, PadState>,
}

impl Default for Pads {
    fn default() -> Self {
        Pads {
            gilrs: Gilrs::new().ok(),
            states: HashMap::new(),
        }
    }
}

impl Pads {
    /// Merged state of every connected gamepad (empty when none or when gilrs is unavailable).
    pub fn poll(&mut self) -> Input {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return Input::default();
        };
        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::ButtonPressed(button, _) => {
                    set_pad_button(&mut self.states, event.id, button, true);
                }
                EventType::ButtonReleased(button, _) => {
                    set_pad_button(&mut self.states, event.id, button, false);
                }
                EventType::ButtonChanged(button, value, _) => {
                    set_pad_button(&mut self.states, event.id, button, value > PRESS_LEVEL);
                }
                EventType::AxisChanged(Axis::LeftStickX, value, _) => {
                    self.states.entry(event.id).or_default().stick_x = value;
                }
                EventType::AxisChanged(Axis::LeftStickY, value, _) => {
                    self.states.entry(event.id).or_default().stick_y = value;
                }
                EventType::Disconnected => {
                    self.states.remove(&event.id);
                }
                _ => {}
            }
        }
        let mut state = Input::default();
        for pad in self.states.values() {
            state = merge(
                state,
                merge(pad.buttons, stick_state(pad.stick_x, pad.stick_y)),
            );
        }
        state
    }
}
