//! System choice of the desktop frontend (E15_23): which console model a ROM runs on.
use puce8gb_core::Model;

/// User choice; Auto picks the model from the ROM header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemChoice {
    Auto,
    Dmg,
    Cgb,
    Sgb,
}

impl SystemChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            SystemChoice::Auto => "auto",
            SystemChoice::Dmg => "dmg",
            SystemChoice::Cgb => "cgb",
            SystemChoice::Sgb => "sgb",
        }
    }

    /// Unknown text gives Auto.
    // Named like the E12_04 LcdMode::from_str on purpose; not the FromStr trait.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> SystemChoice {
        match text {
            "dmg" => SystemChoice::Dmg,
            "cgb" => SystemChoice::Cgb,
            "sgb" => SystemChoice::Sgb,
            _ => SystemChoice::Auto,
        }
    }
}

/// Console model to build for a ROM image.
pub fn resolve(choice: SystemChoice, rom: &[u8]) -> Model {
    match choice {
        SystemChoice::Auto => puce8gb_core::auto_model(rom),
        SystemChoice::Dmg => Model::Dmg,
        SystemChoice::Cgb => Model::Cgb,
        SystemChoice::Sgb => Model::Sgb,
    }
}

/// Menu / status bar text of a choice.
pub fn label(choice: SystemChoice) -> &'static str {
    match choice {
        SystemChoice::Auto => "Auto",
        SystemChoice::Dmg => "Game Boy",
        SystemChoice::Cgb => "Game Boy Color",
        SystemChoice::Sgb => "Super Game Boy",
    }
}

/// The explicit choice that matches a resolved model (used for the status bar text).
pub fn from_model(model: Model) -> SystemChoice {
    match model {
        Model::Dmg => SystemChoice::Dmg,
        Model::Cgb => SystemChoice::Cgb,
        Model::Sgb => SystemChoice::Sgb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_rom::{self, BootKind};

    fn rom_with_flag(flag: u8) -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0143] = flag;
        rom
    }

    #[test]
    fn e15_23_as_str_from_str_round_trip() {
        for choice in [
            SystemChoice::Auto,
            SystemChoice::Dmg,
            SystemChoice::Cgb,
            SystemChoice::Sgb,
        ] {
            assert_eq!(SystemChoice::from_str(choice.as_str()), choice);
        }
    }

    #[test]
    fn e15_23_unknown_text_gives_auto() {
        assert_eq!(SystemChoice::from_str("???"), SystemChoice::Auto);
        assert_eq!(SystemChoice::from_str(""), SystemChoice::Auto);
    }

    #[test]
    fn e15_23_auto_resolves_cgb_flag_to_cgb() {
        assert_eq!(
            resolve(SystemChoice::Auto, &rom_with_flag(0x80)),
            Model::Cgb
        );
    }

    #[test]
    fn e15_23_auto_resolves_plain_rom_to_dmg() {
        assert_eq!(
            resolve(SystemChoice::Auto, &rom_with_flag(0x00)),
            Model::Dmg
        );
    }

    #[test]
    fn e15_23_explicit_dmg_overrides_cgb_only_flag() {
        assert_eq!(resolve(SystemChoice::Dmg, &rom_with_flag(0xC0)), Model::Dmg);
    }

    #[test]
    fn e15_23_explicit_cgb_overrides_plain_rom() {
        assert_eq!(resolve(SystemChoice::Cgb, &rom_with_flag(0x00)), Model::Cgb);
    }

    #[test]
    fn e15_23_labels_and_from_model() {
        assert_eq!(label(SystemChoice::Auto), "Auto");
        assert_eq!(label(SystemChoice::Dmg), "Game Boy");
        assert_eq!(label(SystemChoice::Cgb), "Game Boy Color");
        assert_eq!(label(SystemChoice::Sgb), "Super Game Boy");
        assert_eq!(from_model(Model::Dmg), SystemChoice::Dmg);
        assert_eq!(from_model(Model::Cgb), SystemChoice::Cgb);
    }

    #[test]
    fn e15_23_kind_for_maps_models_to_boot_kinds() {
        assert_eq!(boot_rom::kind_for(Model::Dmg), BootKind::Dmg);
        assert_eq!(boot_rom::kind_for(Model::Cgb), BootKind::Gbc);
        assert_eq!(boot_rom::kind_for(Model::Sgb), BootKind::Sgb);
    }
}
