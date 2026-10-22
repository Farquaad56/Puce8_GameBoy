//! Pure logic for the `suite` runner: argument parsing, manifest parsing, baseline
//! comparison, mooneye signature detection and ROM matching. No file or machine access
//! here; suite_cmd wires these into the CLI.

use crate::run_args::ModelChoice;

/// How an entry's result is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Serial,
    Mooneye,
    Hash,
}

/// One line of the test manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub dir_pattern: String,
    pub prefix: String,
    pub kind: Kind,
    pub max_cycles: u64,
    pub param: String,
    /// Console model of the entry: the optional 7th manifest column `dmg` or `cgb`; default dmg (E15_22).
    pub model: ModelChoice,
}

/// Parse a manifest: one entry per line as `name|dir-pattern|file-prefix|kind|max-cycles|param`.
/// Blank lines and lines starting with '#' are skipped; errors report the 1-based line number.
pub fn parse_manifest(text: &str) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let n = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('|').map(str::trim).collect();
        if parts.len() != 6 && parts.len() != 7 {
            return Err(format!(
                "line {}: expected 6 or 7 fields, found {}",
                n,
                parts.len()
            ));
        }
        let kind = if parts[3].eq_ignore_ascii_case("serial") {
            Kind::Serial
        } else if parts[3].eq_ignore_ascii_case("mooneye") {
            Kind::Mooneye
        } else if parts[3].eq_ignore_ascii_case("hash") {
            Kind::Hash
        } else {
            return Err(format!("line {}: unknown kind '{}'", n, parts[3]));
        };
        let max_cycles = parts[4]
            .parse::<u64>()
            .map_err(|_| format!("line {}: max-cycles is not a u64: '{}'", n, parts[4]))?;
        let model = match parts.get(6).map(|m| m.to_ascii_lowercase()) {
            None => ModelChoice::Dmg,
            Some(m) if m.is_empty() || m == "dmg" => ModelChoice::Dmg,
            Some(m) if m == "cgb" => ModelChoice::Cgb,
            Some(m) => return Err(format!("line {}: unknown model '{}'", n, m)),
        };
        entries.push(Entry {
            name: parts[0].to_string(),
            dir_pattern: parts[1].to_string(),
            prefix: parts[2].to_string(),
            kind,
            max_cycles,
            param: parts[5].to_string(),
            model,
        });
    }
    Ok(entries)
}

/// Keeps the entries of one model (E15_22): the Cgb ones when `cgb` is true, the Dmg ones otherwise.
pub fn entries_for_model(entries: Vec<Entry>, cgb: bool) -> Vec<Entry> {
    entries
        .into_iter()
        .filter(|e| (e.model == ModelChoice::Cgb) == cgb)
        .collect()
}

/// Parse a baseline file: the first non-blank line that does not start with '#', as usize.
pub fn parse_baseline(text: &str) -> Result<usize, String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return trimmed
            .parse::<usize>()
            .map_err(|_| format!("baseline is not a number: '{trimmed}'"));
    }
    Err("no baseline found".to_string())
}

/// The suite passes when at least the baseline number of tests passed.
pub fn baseline_ok(passed: usize, baseline: usize) -> bool {
    passed >= baseline
}

/// Mooneye signature detection on a synthetic program's final register values.
pub fn mooneye_signature(b: u8, c: u8, d: u8, e: u8, h: u8, l: u8) -> Option<bool> {
    if (b, c, d, e, h, l) == (3, 5, 8, 13, 21, 34) {
        Some(true)
    } else if b == 0x42 && c == 0x42 && d == 0x42 && e == 0x42 && h == 0x42 && l == 0x42 {
        Some(false)
    } else {
        None
    }
}

/// Case-insensitive ROM matching: the path contains `dir_pattern` and the file name
/// (last component) starts with `prefix` and ends with ".gb". Backslashes are separators.
pub fn rom_matches(path: &str, dir_pattern: &str, prefix: &str) -> bool {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    let file_name = path.rsplit('/').next().unwrap_or(&path);
    file_name.starts_with(&prefix.to_ascii_lowercase())
        && file_name.ends_with(".gb")
        && path.contains(&dir_pattern.to_ascii_lowercase())
}

/// CLI arguments for the `suite` subcommand (E06_01 step 2b).
#[derive(Debug, PartialEq, Eq)]
pub struct SuiteArgs {
    pub only: Option<String>,
    pub baseline: Option<String>,
    pub manifest: String,
    /// `--cgb`: run the manifest entries of the Cgb model instead of the Dmg ones (E15_22).
    pub cgb: bool,
}

/// Parse `suite` arguments: `--only NAME`, `--baseline FILE`, `--manifest FILE`. The
/// default manifest is `docs/annexes/test_manifest.txt`; any other flag or a positional
/// argument is an error (E06_01 step 2b).
pub fn parse_suite_args(args: &[String]) -> Result<SuiteArgs, String> {
    let mut only = None;
    let mut baseline = None;
    let mut manifest = "docs/annexes/test_manifest.txt".to_string();
    let mut cgb = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            return Err(format!("unexpected argument: {arg}"));
        }
        match arg.as_str() {
            "--only" => only = Some(value_after(args, &mut i, "--only")?),
            "--baseline" => baseline = Some(value_after(args, &mut i, "--baseline")?),
            "--manifest" => manifest = value_after(args, &mut i, "--manifest")?,
            "--cgb" => {
                cgb = true;
                i += 1;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(SuiteArgs {
        only,
        baseline,
        manifest,
        cgb,
    })
}

/// The argument following `flag`; an error when the flag is the last argument. Leaves
/// `*i` pointing just past the consumed value.
fn value_after(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    let next = *i + 1;
    match args.get(next) {
        Some(value) => {
            *i = next + 1;
            Ok(value.clone())
        }
        None => Err(format!("flag {flag} needs a value")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e06_01_parse_manifest_single_entry() {
        let entries = parse_manifest("dmg_sound 07|dmg_sound/|07-|serial|100000000|Passed")
            .expect("a valid manifest line parses");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            Entry {
                name: "dmg_sound 07".to_string(),
                dir_pattern: "dmg_sound/".to_string(),
                prefix: "07-".to_string(),
                kind: Kind::Serial,
                max_cycles: 100_000_000,
                param: "Passed".to_string(),
                model: ModelChoice::Dmg,
            }
        );
    }

    #[test]
    fn e06_01_parse_manifest_skips_comments_and_blank_lines() {
        let text = "# c\n\nA|d/|p-|mooneye|5|-\nB|d/|q-|hash|7|60:0123456789abcdef\n";
        let entries = parse_manifest(text).expect("comments and blank lines are skipped");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, Kind::Mooneye);
        assert_eq!(entries[1].kind, Kind::Hash);
        assert_eq!(entries[1].param, "60:0123456789abcdef");
    }

    #[test]
    fn e06_01_parse_manifest_field_errors() {
        let err = parse_manifest("A|d/|p-|serial|5|x\nB|d/|q-|hash|7")
            .expect_err("a 5-field line is rejected");
        assert!(err.contains("line 2"));
        assert!(err.contains("fields"));

        let err = parse_manifest("A|d/|p-|bogus|5|x").expect_err("an unknown kind is rejected");
        assert!(err.contains("kind"));

        let err =
            parse_manifest("A|d/|p-|serial|abc|x").expect_err("a non-u64 max-cycles is rejected");
        assert!(err.contains("max-cycles"));
    }

    #[test]
    fn e15_22_manifest_line_without_model_column_is_dmg() {
        let entries = parse_manifest("A|d/|p-|serial|5|x").expect("a 6-field line parses");
        assert_eq!(entries[0].model, ModelChoice::Dmg);
    }

    #[test]
    fn e15_22_manifest_model_column_selects_the_model() {
        let text = "A|d/|p-|serial|5|x|cgb\nB|d/|q-|serial|5|x|dmg\nC|d/|r-|serial|5|x|CGB\n";
        let entries = parse_manifest(text).expect("7-field lines parse");
        assert_eq!(entries[0].model, ModelChoice::Cgb);
        assert_eq!(entries[1].model, ModelChoice::Dmg);
        assert_eq!(entries[2].model, ModelChoice::Cgb);
    }

    #[test]
    fn e15_22_unknown_model_name_is_an_error() {
        let err =
            parse_manifest("A|d/|p-|serial|5|x|gbc").expect_err("an unknown model is rejected");
        assert!(err.contains("line 1"));
        assert!(err.contains("model"));
        let err =
            parse_manifest("A|d/|p-|serial|5|x|cgb|extra").expect_err("8 fields are rejected");
        assert!(err.contains("fields"));
    }

    #[test]
    fn e15_22_suite_args_cgb_flag() {
        assert!(!parse_suite_args(&[]).expect("defaults").cgb);
        let args = vec!["--cgb".to_string(), "--only".to_string(), "x".to_string()];
        let parsed = parse_suite_args(&args).expect("flags parse");
        assert!(parsed.cgb);
        assert_eq!(parsed.only.as_deref(), Some("x"));
    }

    #[test]
    fn e15_22_model_filter_keeps_only_the_requested_model() {
        let text = "A|d/|p-|serial|5|x\nB|d/|q-|serial|5|x|cgb\n";
        let entries = parse_manifest(text).expect("parses");
        let dmg = entries_for_model(entries.clone(), false);
        let cgb = entries_for_model(entries, true);
        assert_eq!(dmg.len(), 1);
        assert_eq!(dmg[0].name, "A");
        assert_eq!(cgb.len(), 1);
        assert_eq!(cgb[0].name, "B");
    }

    #[test]
    fn e06_01_parse_baseline() {
        assert_eq!(parse_baseline("# score\n\n42\n"), Ok(42));
        assert!(parse_baseline("abc").is_err());
        assert!(parse_baseline("# only\n").is_err());
    }

    #[test]
    fn e06_01_baseline_ok() {
        assert!(!baseline_ok(41, 42));
        assert!(baseline_ok(42, 42));
        assert!(baseline_ok(43, 42));
    }

    #[test]
    fn e06_01_mooneye_signature() {
        assert_eq!(mooneye_signature(3, 5, 8, 13, 21, 34), Some(true));
        assert_eq!(
            mooneye_signature(0x42, 0x42, 0x42, 0x42, 0x42, 0x42),
            Some(false)
        );
        assert_eq!(mooneye_signature(3, 5, 8, 13, 21, 35), None);
    }

    #[test]
    fn e06_01_rom_matches() {
        assert!(rom_matches(
            "roms/test-roms/dmg_sound/rom_singles/07-wave.gb",
            "dmg_sound/",
            "07-"
        ));
        assert!(rom_matches(
            r"roms\test-roms\DMG_Sound\07-X.GB",
            "dmg_sound/",
            "07-"
        ));
        assert!(!rom_matches(
            "roms/test-roms/dmg_sound/rom_singles/07-wave.gb",
            "dmg_sound/",
            "08-"
        ));
        assert!(!rom_matches(
            "roms/test-roms/dmg_sound/07-wave.gbc",
            "dmg_sound/",
            "07-"
        ));
    }

    #[test]
    fn e06_01_parse_suite_args_defaults() {
        assert_eq!(
            parse_suite_args(&[]),
            Ok(SuiteArgs {
                only: None,
                baseline: None,
                manifest: "docs/annexes/test_manifest.txt".into(),
                cgb: false,
            })
        );
    }

    #[test]
    fn e06_01_parse_suite_args_all_flags() {
        let args = vec![
            "--only".to_string(),
            "dmg_sound 07".to_string(),
            "--baseline".to_string(),
            "b.txt".to_string(),
            "--manifest".to_string(),
            "m.txt".to_string(),
        ];
        assert_eq!(
            parse_suite_args(&args),
            Ok(SuiteArgs {
                only: Some("dmg_sound 07".into()),
                baseline: Some("b.txt".into()),
                manifest: "m.txt".into(),
                cgb: false,
            })
        );
    }

    #[test]
    fn e06_01_parse_suite_args_errors() {
        let err =
            parse_suite_args(&["--bogus".to_string()]).expect_err("an unknown flag is rejected");
        assert!(err.contains("unknown flag"));

        let err = parse_suite_args(&["--only".to_string()])
            .expect_err("a flag without a value is rejected");
        assert!(err.contains("needs a value"));

        let err = parse_suite_args(&["extra".to_string()])
            .expect_err("a positional argument is rejected");
        assert!(err.contains("unexpected argument"));
    }
}
