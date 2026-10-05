//! Save states (decision A_05): versioned binary format written by hand, little-endian,
//! header = 4-byte magic + u32 version (starting at 1) + cartridge type 0147 (note 07a).
//! Serialization of the full state arrives with later tasks.

/// Error returned when a save-state buffer is invalid or too small (never panic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    /// The destination buffer is smaller than the serialized state.
    BufferTooSmall,
    /// The buffer does not hold a valid state: bad magic/version or truncated data.
    InvalidState,
}
