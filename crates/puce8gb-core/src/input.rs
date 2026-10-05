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
