//! Cartridge header parsing and validation (note 07a): the header occupies $0134-$014F of
//! bank 0. The DMG ignores the CGB/SGB/licensee fields, so they are not stored here.

/// Error returned when a cartridge cannot be loaded (bad ROM => Result, never panic).
/// Mirrors `crate::machine::LoadError` semantics; kept local to this module because the
/// task scope is limited to `media/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeLoadError {
    /// The ROM is shorter than the header ($0134-$014F, note 07a).
    TooShort,
    /// The ROM size code at $0148 is not a documented value (note 07a "Taille ROM").
    UnknownRomSize,
}

/// Title region start in bank 0 ($0134, note 07a).
pub const ROM_TITLE_START: usize = 0x134;
/// Title region length: $0134-$0143 (note 07a "titre").
const ROM_TITLE_LEN: usize = 16;
/// Cartridge type / mapper code at $0147 (note 07a "Type de cartouche").
pub const ROM_CARTRIDGE_TYPE: usize = 0x147;
/// ROM size code at $0148 (note 07a "Taille ROM").
pub const ROM_ROM_SIZE: usize = 0x148;
/// RAM size code at $0149 (note 07a "Taille RAM").
pub const ROM_RAM_SIZE: usize = 0x149;
/// Header checksum byte at $014D (note 07a).
pub const ROM_HEADER_CHECKSUM: usize = 0x14D;

/// Minimum ROM length to contain the full header ($0134-$014F, note 07a).
const MIN_ROM_LEN: usize = 0x150;

/// Cartridge header fields (note 07a). The DMG ignores the CGB/SGB/licensee fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Title in ASCII uppercase, padded with $00 ($0134-$0143, note 07a).
    pub title: [u8; ROM_TITLE_LEN],
    /// Cartridge type / mapper code at $0147 (note 07a "Type de cartouche").
    pub cartridge_type: u8,
    /// ROM size code at $0148 (note 07a "Taille ROM").
    pub rom_size_code: u8,
    /// RAM size code at $0149 (note 07a "Taille RAM").
    pub ram_size_code: u8,
    /// Header checksum byte stored at $014D (note 07a).
    pub header_checksum: u8,
    /// Validity flag: true if the stored checksum matches the computed one (note 07a).
    pub checksum_ok: bool,
}

impl Header {
    /// Parse the header from bank 0 of a ROM image. The image must be at least
    /// `MIN_ROM_LEN` bytes; otherwise `CartridgeLoadError::TooShort`. A bad checksum is
    /// reported via `checksum_ok`, never a panic (note 07a).
    pub fn parse(rom: &[u8]) -> Result<Header, CartridgeLoadError> {
        if rom.len() < MIN_ROM_LEN {
            return Err(CartridgeLoadError::TooShort);
        }
        let mut title = [0u8; ROM_TITLE_LEN];
        title.copy_from_slice(&rom[ROM_TITLE_START..ROM_TITLE_START + ROM_TITLE_LEN]);
        Ok(Header {
            title,
            cartridge_type: rom[ROM_CARTRIDGE_TYPE],
            rom_size_code: rom[ROM_ROM_SIZE],
            ram_size_code: rom[ROM_RAM_SIZE],
            header_checksum: rom[ROM_HEADER_CHECKSUM],
            checksum_ok: Self::header_checksum_ok(rom),
        })
    }

    /// Header checksum algorithm (note 07a "Checksum d'en-tete"): x = 0; for each byte of
    /// $0134-$014C, x = x - byte - 1. Returns the low 8 bits.
    pub fn header_checksum(rom: &[u8]) -> u8 {
        let mut x = 0u8;
        for &b in &rom[ROM_TITLE_START..ROM_HEADER_CHECKSUM] {
            x = x.wrapping_sub(b).wrapping_sub(1);
        }
        x
    }

    /// True if the stored checksum at $014D matches the computed header checksum (note 07a).
    pub fn header_checksum_ok(rom: &[u8]) -> bool {
        rom.len() >= MIN_ROM_LEN && rom[ROM_HEADER_CHECKSUM] == Self::header_checksum(rom)
    }
}

/// A loaded cartridge: its parsed header plus the full ROM image (decision A_02).
#[derive(Debug, Clone)]
pub struct Cartridge {
    /// Parsed header fields ($0134-$014F, note 07a).
    pub header: Header,
    /// Full ROM image, bank 0 first.
    pub rom: Vec<u8>,
}

impl Cartridge {
    /// Parse and validate a cartridge from a ROM image. Rejects an unknown ROM size code
    /// (note 07a "Taille ROM") with `CartridgeLoadError::UnknownRomSize`. The header
    /// checksum is not required to be valid; it is reported via `header.checksum_ok`
    /// instead of panicking.
    pub fn new(rom: Vec<u8>) -> Result<Cartridge, CartridgeLoadError> {
        let header = Header::parse(&rom)?;
        if !Self::is_valid_rom_size_code(header.rom_size_code) {
            return Err(CartridgeLoadError::UnknownRomSize);
        }
        Ok(Cartridge { header, rom })
    }

    /// True if the ROM size code at $0148 is a documented value (note 07a "Taille ROM"):
    /// $00-$08. The unofficial $52/$53/$54 codes are rejected as unknown.
    pub fn is_valid_rom_size_code(code: u8) -> bool {
        matches!(code, 0x00..=0x08)
    }

    /// ROM size in bytes for a valid code: 32 KiB x (1 << code) (note 07a "Taille ROM").
    pub fn rom_size_bytes(code: u8) -> Option<usize> {
        match code {
            0x00..=0x08 => Some(32 * 1024 * (1usize << code)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic ROM image of at least `MIN_ROM_LEN` bytes with the given header
    /// fields set and a valid stored checksum.
    fn rom_with(len: usize, title: &[u8], cart_type: u8, rom_size: u8, ram_size: u8) -> Vec<u8> {
        let mut r = vec![0x00u8; len.max(MIN_ROM_LEN)];
        for (i, b) in title.iter().enumerate() {
            if i < ROM_TITLE_LEN {
                r[ROM_TITLE_START + i] = *b;
            }
        }
        r[ROM_CARTRIDGE_TYPE] = cart_type;
        r[ROM_ROM_SIZE] = rom_size;
        r[ROM_RAM_SIZE] = ram_size;
        // Set the stored checksum to the computed value so it is valid by default.
        r[ROM_HEADER_CHECKSUM] = Header::header_checksum(&r);
        r
    }

    #[test]
    fn e01_02_good_header_parses() {
        let title: &[u8] = b"MY GAME";
        let rom = rom_with(32 * 1024, title, 0x00, 0x00, 0x00);
        let header = Header::parse(&rom).expect("a valid ROM parses");
        assert_eq!(&header.title[..title.len()], title);
        assert_eq!(header.cartridge_type, 0x00);
        assert_eq!(header.rom_size_code, 0x00);
        assert_eq!(header.ram_size_code, 0x00);
        assert!(
            header.checksum_ok,
            "the stored checksum matches the computed one"
        );
    }

    #[test]
    fn e01_02_bad_checksum_reported_not_panicked() {
        let mut rom = rom_with(32 * 1024, b"MY GAME", 0x00, 0x00, 0x00);
        // Corrupt the stored checksum so it no longer matches.
        rom[ROM_HEADER_CHECKSUM] ^= 0xFF;
        let header = Header::parse(&rom).expect("a ROM with a bad checksum still parses");
        assert!(
            !header.checksum_ok,
            "the mismatched checksum is reported as invalid"
        );
        assert!(!Header::header_checksum_ok(&rom));
    }

    #[test]
    fn e01_02_unknown_rom_size_rejected() {
        // $52 is an unofficial ROM size code (note 07a "Taille ROM"): rejected.
        let rom = rom_with(32 * 1024, b"MY GAME", 0x00, 0x52, 0x00);
        assert!(matches!(
            Cartridge::new(rom),
            Err(CartridgeLoadError::UnknownRomSize)
        ));
    }

    #[test]
    fn e01_02_valid_rom_size_accepted() {
        let rom = rom_with(32 * 1024, b"MY GAME", 0x00, 0x03, 0x00); // $03 = 256K
        let cart = Cartridge::new(rom).expect("a valid ROM size code loads");
        assert_eq!(cart.header.rom_size_code, 0x03);
        assert_eq!(Cartridge::rom_size_bytes(0x03), Some(256 * 1024));
    }

    #[test]
    fn e01_02_short_rom_rejected() {
        let rom = vec![0x00u8; 100]; // shorter than the header
        assert!(matches!(
            Header::parse(&rom),
            Err(CartridgeLoadError::TooShort)
        ));
    }

    #[test]
    fn e01_02_checksum_algorithm_all_zero() {
        // All-zero $0134-$014C region: x = 0; repeat (x - 0 - 1) 25 times => 0xE7.
        let rom = vec![0x00u8; MIN_ROM_LEN];
        assert_eq!(Header::header_checksum(&rom), 0xE7);
    }
}
