//! Boot ROM kinds, size validation and the Enable/Skip decision (E14_06, decision A_11).
//! No file access here: callers pass the bytes. Boot ROMs are never committed (.gitignore).

/// The three boot ROM files the user can set.
#[allow(dead_code)] // used by E14_07
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BootKind {
    Dmg,
    Gbc,
    Sgb,
}

/// Expected file size in bytes. Gbc: UNKNOWN - to confirm. Note 10 gives 256 + 1792 = 2048
/// bytes (pandocs); 2304 is the size of a $0000-$08FF dump including the header gap, the value
/// fixed by task E14_06.
#[allow(dead_code)] // used by E14_07
pub fn expected_size(kind: BootKind) -> usize {
    match kind {
        BootKind::Dmg => 256,
        BootKind::Sgb => 256,
        BootKind::Gbc => 2304,
    }
}

/// Checks the size of a boot ROM image.
#[allow(dead_code)] // used by E14_07
pub fn validate(kind: BootKind, bytes: &[u8]) -> Result<(), String> {
    let expected = expected_size(kind);
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "boot ROM must be {expected} bytes (got {})",
            bytes.len()
        ))
    }
}

/// What to do at load time.
#[allow(dead_code)] // used by E14_07
#[derive(Debug, Clone, PartialEq)]
pub enum BootChoice {
    /// Start without a boot ROM (post-boot state).
    Skip,
    /// Start from the boot ROM.
    Use,
    /// Boot ROM enabled but unusable: start without it and show the text.
    FallbackSkip(String),
}

/// Decides between Skip, Use and FallbackSkip from the Enable flag and the file content.
#[allow(dead_code)] // used by E14_07
pub fn choose(enabled: bool, kind: BootKind, bytes: Option<&[u8]>) -> BootChoice {
    if !enabled {
        return BootChoice::Skip;
    }
    match bytes {
        None => BootChoice::FallbackSkip("no boot ROM file set".to_string()),
        Some(bytes) => match validate(kind, bytes) {
            Ok(()) => BootChoice::Use,
            Err(text) => BootChoice::FallbackSkip(text),
        },
    }
}

/// Text shown next to a boot ROM menu entry: "(none)" for an empty path, else the file name (E14_07).
pub fn boot_label(path: &str) -> String {
    if path.is_empty() {
        "(none)".to_string()
    } else {
        crate::recent::recent_label(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e14_06_expected_sizes() {
        assert_eq!(expected_size(BootKind::Dmg), 256);
        assert_eq!(expected_size(BootKind::Sgb), 256);
        assert_eq!(expected_size(BootKind::Gbc), 2304);
    }

    #[test]
    fn e14_06_validate_ok_and_error_text() {
        assert_eq!(validate(BootKind::Dmg, &[0u8; 256]), Ok(()));
        assert_eq!(validate(BootKind::Gbc, &[0u8; 2304]), Ok(()));
        assert_eq!(
            validate(BootKind::Dmg, &[0u8; 255]),
            Err("boot ROM must be 256 bytes (got 255)".to_string())
        );
    }

    #[test]
    fn e14_07_boot_label_empty_path_is_none() {
        assert_eq!(boot_label(""), "(none)");
    }

    #[test]
    fn e14_07_boot_label_is_the_file_name() {
        assert_eq!(boot_label("C:\\x\\dmg_boot.bin"), "dmg_boot.bin");
        assert_eq!(boot_label("dmg_boot.bin"), "dmg_boot.bin");
    }

    #[test]
    fn e14_06_choose_covers_the_four_cases() {
        let good = [0u8; 256];
        let bad = [0u8; 255];
        assert_eq!(
            choose(false, BootKind::Dmg, Some(&good[..])),
            BootChoice::Skip
        );
        assert_eq!(
            choose(true, BootKind::Dmg, None),
            BootChoice::FallbackSkip("no boot ROM file set".to_string())
        );
        assert_eq!(
            choose(true, BootKind::Dmg, Some(&bad[..])),
            BootChoice::FallbackSkip("boot ROM must be 256 bytes (got 255)".to_string())
        );
        assert_eq!(
            choose(true, BootKind::Dmg, Some(&good[..])),
            BootChoice::Use
        );
    }
}
