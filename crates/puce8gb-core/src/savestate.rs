//! Save states (decision A_05): versioned binary format written by hand, little-endian,
//! header = 4-byte magic + u32 version (starting at 1) + cartridge type 0147 (note 07a).
//! This module only holds the writer/reader helpers and the header; the serialization of
//! the machine components arrives with later tasks. No I/O, no dependency, no panic.
//!
//! A_05 fixes the third header field as the cartridge type; a ROM checksum is not part of
//! A_05, so none is stored.

/// Magic bytes at the start of every state.
/// UNKNOWN - to confirm: A_05 only fixes the length (4 bytes); this value is a placeholder choice.
pub const MAGIC: [u8; 4] = *b"P8GB";
/// Format version written in the header (A_05: starts at 1; 2 since E14_04, which adds the extension block of A_11).
pub const VERSION: u32 = 2;
/// Header length in bytes: magic (4) + version (4) + cartridge type (1).
pub const HEADER_LEN: usize = 9;

/// Error returned when a save-state buffer is invalid or too small (never panic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    /// The destination buffer is smaller than the serialized state.
    BufferTooSmall,
    /// The buffer does not hold a valid state (for example a corrupted flag byte).
    InvalidState,
    /// The first four bytes are not the state magic.
    BadMagic,
    /// The version field is not the supported version.
    BadVersion,
    /// The data is truncated: fewer bytes than the format needs.
    BadSize,
    /// The state was saved for another cartridge type than the loaded ROM.
    RomMismatch,
}

/// Writes little-endian values into a caller-provided buffer.
#[derive(Debug)]
pub struct StateWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> StateWriter<'a> {
    /// Starts writing at the beginning of `buf`.
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Number of bytes written so far.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Appends raw bytes; nothing is written when they do not fit.
    pub fn put_bytes(&mut self, data: &[u8]) -> Result<(), StateError> {
        let end = self
            .pos
            .checked_add(data.len())
            .ok_or(StateError::BufferTooSmall)?;
        let dst = self
            .buf
            .get_mut(self.pos..end)
            .ok_or(StateError::BufferTooSmall)?;
        dst.copy_from_slice(data);
        self.pos = end;
        Ok(())
    }

    pub fn put_u8(&mut self, value: u8) -> Result<(), StateError> {
        self.put_bytes(&[value])
    }

    pub fn put_u16(&mut self, value: u16) -> Result<(), StateError> {
        self.put_bytes(&value.to_le_bytes())
    }

    pub fn put_u32(&mut self, value: u32) -> Result<(), StateError> {
        self.put_bytes(&value.to_le_bytes())
    }

    pub fn put_u64(&mut self, value: u64) -> Result<(), StateError> {
        self.put_bytes(&value.to_le_bytes())
    }

    pub fn put_bool(&mut self, value: bool) -> Result<(), StateError> {
        self.put_u8(u8::from(value))
    }

    /// Writes the header: magic, version, cartridge type (all or nothing).
    pub fn put_header(&mut self, cartridge_type: u8) -> Result<(), StateError> {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(&MAGIC);
        header[4..8].copy_from_slice(&VERSION.to_le_bytes());
        header[8] = cartridge_type;
        self.put_bytes(&header)
    }
}

/// Reads little-endian values from a buffer; truncated data returns `BadSize`.
#[derive(Debug)]
pub struct StateReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> StateReader<'a> {
    /// Starts reading at the beginning of `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Number of bytes read so far.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Number of bytes not read yet.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Reads `len` raw bytes; nothing is consumed when they are not all there.
    pub fn get_bytes(&mut self, len: usize) -> Result<&'a [u8], StateError> {
        let end = self.pos.checked_add(len).ok_or(StateError::BadSize)?;
        let src = self.buf.get(self.pos..end).ok_or(StateError::BadSize)?;
        self.pos = end;
        Ok(src)
    }

    fn get_array<const N: usize>(&mut self) -> Result<[u8; N], StateError> {
        let src = self.get_bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(src);
        Ok(out)
    }

    pub fn get_u8(&mut self) -> Result<u8, StateError> {
        Ok(self.get_array::<1>()?[0])
    }

    pub fn get_u16(&mut self) -> Result<u16, StateError> {
        Ok(u16::from_le_bytes(self.get_array()?))
    }

    pub fn get_u32(&mut self) -> Result<u32, StateError> {
        Ok(u32::from_le_bytes(self.get_array()?))
    }

    pub fn get_u64(&mut self) -> Result<u64, StateError> {
        Ok(u64::from_le_bytes(self.get_array()?))
    }

    /// Reads a flag byte: 0 = false, 1 = true, anything else is `InvalidState`.
    pub fn get_bool(&mut self) -> Result<bool, StateError> {
        match self.get_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StateError::InvalidState),
        }
    }

    /// Reads and checks the header against the cartridge type of the loaded ROM.
    pub fn get_header(&mut self, cartridge_type: u8) -> Result<(), StateError> {
        let magic: [u8; 4] = self.get_array()?;
        if magic != MAGIC {
            return Err(StateError::BadMagic);
        }
        if self.get_u32()? != VERSION {
            return Err(StateError::BadVersion);
        }
        if self.get_u8()? != cartridge_type {
            return Err(StateError::RomMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_header() -> [u8; HEADER_LEN] {
        let mut buf = [0u8; HEADER_LEN];
        StateWriter::new(&mut buf)
            .put_header(0x13)
            .expect("header fits");
        buf
    }

    #[test]
    fn e09_01_primitives_round_trip_little_endian() {
        let mut buf = [0u8; 32];
        let mut w = StateWriter::new(&mut buf);
        w.put_u8(0xAB).expect("room");
        w.put_u16(0x1234).expect("room");
        w.put_u32(0xDEAD_BEEF).expect("room");
        w.put_u64(0x0102_0304_0506_0708).expect("room");
        w.put_bool(true).expect("room");
        w.put_bytes(&[1, 2, 3]).expect("room");
        assert_eq!(w.position(), 19);
        assert_eq!(
            &buf[..19],
            &[
                0xAB, 0x34, 0x12, 0xEF, 0xBE, 0xAD, 0xDE, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02,
                0x01, 0x01, 0x01, 0x02, 0x03
            ]
        );
        let mut r = StateReader::new(&buf[..19]);
        assert_eq!(r.get_u8(), Ok(0xAB));
        assert_eq!(r.get_u16(), Ok(0x1234));
        assert_eq!(r.get_u32(), Ok(0xDEAD_BEEF));
        assert_eq!(r.get_u64(), Ok(0x0102_0304_0506_0708));
        assert_eq!(r.get_bool(), Ok(true));
        assert_eq!(r.get_bytes(3), Ok(&[1u8, 2, 3][..]));
        assert_eq!(r.remaining(), 0);
        assert_eq!(r.position(), 19);
    }

    #[test]
    fn e09_01_header_layout_and_round_trip() {
        assert_eq!(HEADER_LEN, 9);
        assert_eq!(VERSION, 2);
        let mut buf = [0xEEu8; 16];
        let mut w = StateWriter::new(&mut buf);
        w.put_header(0x13).expect("room");
        assert_eq!(w.position(), HEADER_LEN);
        assert_eq!(
            &buf[..HEADER_LEN],
            &[0x50, 0x38, 0x47, 0x42, 0x02, 0x00, 0x00, 0x00, 0x13]
        );
        assert_eq!(buf[HEADER_LEN], 0xEE);
        let mut r = StateReader::new(&buf);
        assert_eq!(r.get_header(0x13), Ok(()));
        assert_eq!(r.position(), HEADER_LEN);
    }

    #[test]
    fn e09_01_header_rejects_bad_magic_version_and_cartridge() {
        let good = valid_header();
        let mut bad = good;
        bad[0] = 0x58;
        assert_eq!(
            StateReader::new(&bad).get_header(0x13),
            Err(StateError::BadMagic)
        );
        let mut bad = good;
        bad[4] = 1;
        assert_eq!(
            StateReader::new(&bad).get_header(0x13),
            Err(StateError::BadVersion)
        );
        assert_eq!(
            StateReader::new(&good).get_header(0x01),
            Err(StateError::RomMismatch)
        );
    }

    #[test]
    fn e09_01_truncated_header_is_bad_size() {
        let good = valid_header();
        for len in 0..HEADER_LEN {
            assert_eq!(
                StateReader::new(&good[..len]).get_header(0x13),
                Err(StateError::BadSize),
                "prefix of {len} bytes"
            );
        }
    }

    #[test]
    fn e09_01_truncated_primitives_never_panic() {
        let data = [0x01u8, 0x02, 0x03];
        let mut r = StateReader::new(&data);
        assert_eq!(r.get_u32(), Err(StateError::BadSize));
        assert_eq!(r.position(), 0);
        assert_eq!(r.get_u16(), Ok(0x0201));
        assert_eq!(r.get_u16(), Err(StateError::BadSize));
        assert_eq!(r.get_u8(), Ok(0x03));
        assert_eq!(r.get_u8(), Err(StateError::BadSize));
        assert_eq!(r.get_u64(), Err(StateError::BadSize));
        assert_eq!(r.get_bytes(usize::MAX), Err(StateError::BadSize));
        let empty: [u8; 0] = [];
        assert_eq!(
            StateReader::new(&empty).get_bool(),
            Err(StateError::BadSize)
        );
    }

    #[test]
    fn e09_01_writer_reports_buffer_too_small() {
        let mut buf = [0u8; 3];
        {
            let mut w = StateWriter::new(&mut buf);
            assert_eq!(w.put_u16(0x1234), Ok(()));
            assert_eq!(w.put_u16(0x5678), Err(StateError::BufferTooSmall));
            assert_eq!(w.position(), 2);
            assert_eq!(w.put_u8(0x56), Ok(()));
            assert_eq!(w.put_u8(0x57), Err(StateError::BufferTooSmall));
            assert_eq!(w.put_bytes(&[]), Ok(()));
        }
        assert_eq!(buf, [0x34, 0x12, 0x56]);
        let mut small = [0u8; HEADER_LEN - 1];
        assert_eq!(
            StateWriter::new(&mut small).put_header(0x13),
            Err(StateError::BufferTooSmall)
        );
        assert_eq!(small, [0u8; HEADER_LEN - 1]);
    }

    #[test]
    fn e09_01_corrupted_bool_is_invalid_state() {
        for (byte, expected) in [
            (0u8, Ok(false)),
            (1, Ok(true)),
            (2, Err(StateError::InvalidState)),
            (0xFF, Err(StateError::InvalidState)),
        ] {
            let data = [byte];
            assert_eq!(StateReader::new(&data).get_bool(), expected);
        }
    }
}
