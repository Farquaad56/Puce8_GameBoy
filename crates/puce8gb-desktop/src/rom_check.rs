//! ROM header inspection before the ROM is sent to the core (E13_01, decision A_10).
//! Pure functions, no egui type. The Nintendo logo is not checked (the real boot ROM does it).

use crate::rom_info::rom_title;

/// Smallest ROM the loader accepts: the header ends at $014F.
const HEADER_END: usize = 0x150;
const SGB_FLAG: usize = 0x146;
const CGB_FLAG: usize = 0x143;
const CART_TYPE: usize = 0x147;
const ROM_SIZE_CODE: usize = 0x148;
const RAM_SIZE_CODE: usize = 0x149;
const CHECKSUM_FIRST: usize = 0x134;
const CHECKSUM_LAST: usize = 0x14C;
const CHECKSUM_BYTE: usize = 0x14D;

/// Error: the ROM cannot run. Warning: the ROM runs anyway.
#[allow(dead_code)] // used by E13_04
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One problem found in a ROM image.
#[allow(dead_code)] // used by E13_04
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomIssue {
    pub severity: Severity,
    pub message: String,
}

/// Facts read from the cartridge header.
#[allow(dead_code)] // used by E13_04
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomReport {
    pub title: String,
    pub cart_type: u8,
    pub rom_size_declared: usize,
    pub rom_size_actual: usize,
    pub ram_size: usize,
    pub cgb_flag: u8,
    pub sgb_flag: u8,
    pub header_checksum_ok: bool,
}

fn warning(message: String) -> RomIssue {
    RomIssue {
        severity: Severity::Warning,
        message,
    }
}

/// Inspects a ROM image: a report (None when the ROM is too small) and the issues found.
#[allow(dead_code)] // used by E13_04
pub fn inspect(bytes: &[u8]) -> (Option<RomReport>, Vec<RomIssue>) {
    if bytes.len() < HEADER_END {
        let issue = RomIssue {
            severity: Severity::Error,
            message: format!("ROM too small ({} bytes)", bytes.len()),
        };
        return (None, vec![issue]);
    }
    let mut issues = Vec::new();

    let size_code = bytes[ROM_SIZE_CODE];
    let declared = if size_code <= 8 {
        32768usize << size_code
    } else {
        issues.push(warning(format!("unknown ROM size code ${size_code:02X}")));
        0
    };

    let ram_code = bytes[RAM_SIZE_CODE];
    let ram_size = match ram_code {
        0 => 0,
        2 => 8192,
        3 => 32768,
        4 => 131072,
        5 => 65536,
        _ => {
            issues.push(warning(format!("unknown RAM size code ${ram_code:02X}")));
            0
        }
    };

    let mut sum: u8 = 0;
    for &byte in &bytes[CHECKSUM_FIRST..=CHECKSUM_LAST] {
        sum = sum.wrapping_sub(byte).wrapping_sub(1);
    }
    let header_checksum_ok = sum == bytes[CHECKSUM_BYTE];
    if !header_checksum_ok {
        issues.push(warning(String::from("header checksum mismatch")));
    }

    let actual = bytes.len();
    if declared != 0 && declared != actual {
        issues.push(warning(format!(
            "declared size {declared} differs from file size {actual}"
        )));
    }

    let report = RomReport {
        title: rom_title(bytes),
        cart_type: bytes[CART_TYPE],
        rom_size_declared: declared,
        rom_size_actual: actual,
        ram_size,
        cgb_flag: bytes[CGB_FLAG],
        sgb_flag: bytes[SGB_FLAG],
        header_checksum_ok,
    };
    (Some(report), issues)
}

/// "Error: <message>" or "Warning: <message>".
#[allow(dead_code)] // used by E13_04
pub fn issue_text(issue: &RomIssue) -> String {
    match issue.severity {
        Severity::Error => format!("Error: {}", issue.message),
        Severity::Warning => format!("Warning: {}", issue.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes the correct header checksum at $014D.
    fn fix_checksum(rom: &mut [u8]) {
        let mut sum: u8 = 0;
        for &byte in &rom[CHECKSUM_FIRST..=CHECKSUM_LAST] {
            sum = sum.wrapping_sub(byte).wrapping_sub(1);
        }
        rom[CHECKSUM_BYTE] = sum;
    }

    /// ROM image with the title at $0134, the size code at $0148 and a correct checksum.
    fn make_rom(title: &[u8], size_code: u8, len: usize) -> Vec<u8> {
        let mut rom = vec![0u8; len];
        rom[CHECKSUM_FIRST..CHECKSUM_FIRST + title.len()].copy_from_slice(title);
        rom[ROM_SIZE_CODE] = size_code;
        fix_checksum(&mut rom);
        rom
    }

    #[test]
    fn e13_01_checksum_known_values() {
        assert_eq!(make_rom(b"", 0, 32768)[CHECKSUM_BYTE], 0xE7);
        assert_eq!(make_rom(b"TEST", 0, 32768)[CHECKSUM_BYTE], 0xA7);
    }

    #[test]
    fn e13_01_good_rom_has_no_issue() {
        let rom = make_rom(b"TEST", 0, 32768);
        let (report, issues) = inspect(&rom);
        let report = report.expect("report");
        assert!(issues.is_empty());
        assert!(report.header_checksum_ok);
        assert_eq!(report.title, "TEST");
        assert_eq!(report.rom_size_declared, 32768);
        assert_eq!(report.rom_size_actual, 32768);
        assert_eq!(report.ram_size, 0);
    }

    #[test]
    fn e13_01_corrupt_title_byte_gives_checksum_warning() {
        let mut rom = make_rom(b"TEST", 0, 32768);
        rom[CHECKSUM_FIRST] ^= 0x01;
        let (report, issues) = inspect(&rom);
        assert!(!report.expect("report").header_checksum_ok);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].message, "header checksum mismatch");
    }

    #[test]
    fn e13_01_too_small_gives_error_and_no_report() {
        let (report, issues) = inspect(&vec![0u8; 0x100]);
        assert!(report.is_none());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(issues[0].message, "ROM too small (256 bytes)");
    }

    #[test]
    fn e13_01_declared_size_mismatch_gives_warning() {
        let rom = make_rom(b"TEST", 1, 32768);
        let (_, issues) = inspect(&rom);
        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].message,
            "declared size 65536 differs from file size 32768"
        );
    }

    #[test]
    fn e13_01_unknown_size_and_ram_codes_warn() {
        let mut rom = make_rom(b"TEST", 9, 32768);
        rom[RAM_SIZE_CODE] = 1;
        fix_checksum(&mut rom);
        let (report, issues) = inspect(&rom);
        let report = report.expect("report");
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.severity == Severity::Warning));
        assert_eq!(report.rom_size_declared, 0);
        assert_eq!(report.ram_size, 0);
    }

    #[test]
    fn e13_01_ram_size_codes() {
        for (code, expected) in [
            (0u8, 0usize),
            (2, 8192),
            (3, 32768),
            (4, 131072),
            (5, 65536),
        ] {
            let mut rom = make_rom(b"TEST", 0, 32768);
            rom[RAM_SIZE_CODE] = code;
            fix_checksum(&mut rom);
            let (report, issues) = inspect(&rom);
            assert!(issues.is_empty());
            assert_eq!(report.expect("report").ram_size, expected);
        }
    }

    #[test]
    fn e13_01_report_reads_the_header_flags() {
        let mut rom = make_rom(b"TEST", 0, 32768);
        rom[CGB_FLAG] = 0x80;
        rom[SGB_FLAG] = 0x03;
        rom[CART_TYPE] = 0x19;
        fix_checksum(&mut rom);
        let report = inspect(&rom).0.expect("report");
        assert_eq!(report.cgb_flag, 0x80);
        assert_eq!(report.sgb_flag, 0x03);
        assert_eq!(report.cart_type, 0x19);
        assert!(report.header_checksum_ok);
    }

    #[test]
    fn e13_01_issue_text_prefixes() {
        let error = RomIssue {
            severity: Severity::Error,
            message: String::from("boom"),
        };
        let warn = RomIssue {
            severity: Severity::Warning,
            message: String::from("careful"),
        };
        assert_eq!(issue_text(&error), "Error: boom");
        assert_eq!(issue_text(&warn), "Warning: careful");
    }
}
