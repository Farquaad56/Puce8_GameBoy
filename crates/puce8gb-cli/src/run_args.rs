//! Argument parsing for the `run` command (task C01_43):
//! `run <rom> [--max-cycles N] [--expect-serial TEXT] [--trace N] [--ly-fixed HEX]`
//! `[--frames N] [--dump-ppm FILE] [--boot-rom FILE]`.

/// Default execution budget in T-cycles: 100_000_000.
const DEFAULT_MAX_CYCLES: u64 = 100_000_000;

/// Console model selection of `--model auto|dmg|cgb` (E15_20). `auto` picks the model from the
/// cartridge header with `puce8gb_core::auto_model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    Auto,
    Dmg,
    Cgb,
}

/// Parsed arguments of the `run` command (task C01_43). No ROM is executed yet.
pub struct RunArgs {
    /// Path of the ROM to run (first positional argument).
    pub rom: String,
    /// Maximum number of T-cycles to execute (default 100_000_000).
    pub max_cycles: u64,
    /// Expected serial output, if any (`--expect-serial TEXT`).
    pub expect_serial: Option<String>,
    /// Trace window size in instructions; 0 disables tracing (default 0) (task C01_47).
    pub trace: u64,
    /// Fixed value returned by every $FF44 (LY) read; None keeps normal behaviour
    /// (`--ly-fixed HEX`, task C01_49). Debug tool for trace comparison.
    pub ly_fixed: Option<u8>,
    /// Number of frames to run (`--frames N`, E03_16).
    pub frames: Option<u64>,
    /// PPM output path (`--dump-ppm FILE`, E03_16).
    pub dump_ppm: Option<String>,
    /// Print the framebuffer hash after `frames` frames (`--hash`, E03_07).
    pub hash: bool,
    /// Write the machine save state to this file after the frames ran (`--save-state FILE`, E09_03).
    pub save_state: Option<String>,
    /// Load this save state file before running the frames (`--load-state FILE`, E09_03).
    pub load_state: Option<String>,
    /// Boot ROM file to start from (`--boot-rom FILE`, E14_05).
    pub boot_rom: Option<String>,
    /// Console model (`--model auto|dmg|cgb`, E15_20); default auto.
    pub model: ModelChoice,
    /// Suppress the serial echo on stdout; set by the suite runner, not a CLI flag (E06_01).
    pub quiet: bool,
}

/// Parse the arguments that follow the word `run`. The first positional argument is the
/// ROM path; extra positional arguments are ignored. Errors: missing rom, unknown flag,
/// flag without value, non-numeric number.
pub fn parse_run_args(args: &[String]) -> Result<RunArgs, String> {
    let mut rom: Option<String> = None;
    let mut max_cycles = DEFAULT_MAX_CYCLES;
    let mut expect_serial: Option<String> = None;
    let mut trace: u64 = 0;
    let mut ly_fixed: Option<u8> = None;
    let mut frames: Option<u64> = None;
    let mut dump_ppm: Option<String> = None;
    let mut hash = false;
    let mut save_state: Option<String> = None;
    let mut load_state: Option<String> = None;
    let mut boot_rom: Option<String> = None;
    let mut model = ModelChoice::Auto;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--max-cycles" => max_cycles = number(args, &mut i, "--max-cycles")?,
            "--trace" => trace = number(args, &mut i, "--trace")?,
            "--expect-serial" => expect_serial = Some(value(args, &mut i, "--expect-serial")?),
            "--ly-fixed" => ly_fixed = Some(hex_byte(args, &mut i, "--ly-fixed")?),
            "--frames" => frames = Some(number(args, &mut i, "--frames")?),
            "--dump-ppm" => dump_ppm = Some(value(args, &mut i, "--dump-ppm")?),
            "--hash" => hash = true,
            "--save-state" => save_state = Some(value(args, &mut i, "--save-state")?),
            "--load-state" => load_state = Some(value(args, &mut i, "--load-state")?),
            "--boot-rom" => boot_rom = Some(value(args, &mut i, "--boot-rom")?),
            "--model" => model = model_choice(args, &mut i)?,
            other if other.starts_with("--") => return Err(format!("unknown flag: {other}")),
            positional => {
                if rom.is_none() {
                    rom = Some(positional.to_string());
                }
            }
        }
        i += 1;
    }

    let rom = rom.ok_or_else(|| "missing rom".to_string())?;
    if dump_ppm.is_some() && frames.is_none() {
        return Err("--dump-ppm requires --frames".to_string());
    }
    if hash && frames.is_none() {
        return Err("--hash requires --frames".to_string());
    }
    if save_state.is_some() && frames.is_none() {
        return Err("--save-state requires --frames".to_string());
    }
    if load_state.is_some() && frames.is_none() {
        return Err("--load-state requires --frames".to_string());
    }
    if boot_rom.is_some() && (save_state.is_some() || load_state.is_some()) {
        return Err("--boot-rom cannot be combined with --save-state or --load-state".to_string());
    }
    Ok(RunArgs {
        rom,
        max_cycles,
        expect_serial,
        trace,
        ly_fixed,
        frames,
        dump_ppm,
        hash,
        save_state,
        load_state,
        boot_rom,
        model,
        quiet: false,
    })
}

/// Take the value of `--model` (E15_20): `auto`, `dmg` or `cgb`. `sgb` is rejected until E16.
fn model_choice(args: &[String], i: &mut usize) -> Result<ModelChoice, String> {
    match value(args, i, "--model")?.as_str() {
        "auto" => Ok(ModelChoice::Auto),
        "dmg" => Ok(ModelChoice::Dmg),
        "cgb" => Ok(ModelChoice::Cgb),
        "sgb" => Err("--model sgb is not supported yet".to_string()),
        _ => Err("--model expects auto, dmg or cgb".to_string()),
    }
}

/// Take the value of a flag that expects a hex byte (the next argument), with or without
/// a `0x` prefix (`90` and `0x90` are the same, task C01_49). Errors if it is missing
/// or not a valid hex byte.
fn hex_byte(args: &[String], i: &mut usize, flag: &str) -> Result<u8, String> {
    let raw = value(args, i, flag)?;
    let digits = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(&raw);
    u8::from_str_radix(digits, 16).map_err(|_| format!("{flag} expects a hex byte"))
}

/// Take the value of a flag that expects a number (the next argument).
fn number(args: &[String], i: &mut usize, flag: &str) -> Result<u64, String> {
    let raw = value(args, i, flag)?;
    raw.parse::<u64>()
        .map_err(|_| format!("{flag} expects a number"))
}

/// Take the value of a flag (the next argument). Errors if it is missing.
fn value(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn e14_05_boot_rom_default_is_none() {
        let a = parse_run_args(&[arg("game.gb")]).expect("a rom path parses");
        assert_eq!(a.boot_rom, None);
    }

    #[test]
    fn e14_05_boot_rom_flag_stores_the_path() {
        let a = parse_run_args(&[arg("game.gb"), arg("--boot-rom"), arg("x.bin")])
            .expect("the flag parses");
        assert_eq!(a.boot_rom.as_deref(), Some("x.bin"));
    }

    #[test]
    fn e14_05_boot_rom_missing_value_is_a_usage_error() {
        let err = err_of(&[arg("game.gb"), arg("--boot-rom")]);
        assert_eq!(err, "--boot-rom requires a value");
    }

    #[test]
    fn e14_05_boot_rom_conflicts_with_state_flags() {
        let err = err_of(&[
            arg("game.gb"),
            arg("--frames"),
            arg("1"),
            arg("--save-state"),
            arg("s.bin"),
            arg("--boot-rom"),
            arg("b.bin"),
        ]);
        assert_eq!(
            err,
            "--boot-rom cannot be combined with --save-state or --load-state"
        );
    }

    #[test]
    fn e15_20_model_default_is_auto() {
        let a = parse_run_args(&[arg("game.gb")]).expect("a rom path parses");
        assert_eq!(a.model, ModelChoice::Auto);
    }

    #[test]
    fn e15_20_model_parses_auto_dmg_and_cgb() {
        for (text, expected) in [
            ("auto", ModelChoice::Auto),
            ("dmg", ModelChoice::Dmg),
            ("cgb", ModelChoice::Cgb),
        ] {
            let a = parse_run_args(&[arg("game.gb"), arg("--model"), arg(text)])
                .expect("the value parses");
            assert_eq!(a.model, expected, "value {text}");
        }
    }

    #[test]
    fn e15_20_model_bad_value_is_a_usage_error() {
        let err = err_of(&[arg("game.gb"), arg("--model"), arg("gbc")]);
        assert_eq!(err, "--model expects auto, dmg or cgb");
        let err = err_of(&[arg("game.gb"), arg("--model")]);
        assert_eq!(err, "--model requires a value");
    }

    #[test]
    fn e15_20_model_sgb_is_rejected_until_e16() {
        let err = err_of(&[arg("game.gb"), arg("--model"), arg("sgb")]);
        assert_eq!(err, "--model sgb is not supported yet");
    }

    /// The error message of a parse that must fail.
    fn err_of(args: &[String]) -> String {
        match parse_run_args(args) {
            Ok(_) => panic!("expected a parse error"),
            Err(msg) => msg,
        }
    }

    #[test]
    fn c01_43_defaults() {
        let a = parse_run_args(&[arg("game.gb")]).expect("a rom path parses");
        assert_eq!(a.rom, "game.gb");
        assert_eq!(a.max_cycles, 100_000_000);
        assert_eq!(a.expect_serial, None);
        assert_eq!(a.trace, 0);
        assert_eq!(a.ly_fixed, None);
    }

    #[test]
    fn c01_43_all_flags_together() {
        let a = parse_run_args(&[
            arg("game.gb"),
            arg("--max-cycles"),
            arg("5000"),
            arg("--expect-serial"),
            arg("PASS"),
            arg("--trace"),
            arg("7"),
        ])
        .expect("all flags parse");
        assert_eq!(a.rom, "game.gb");
        assert_eq!(a.max_cycles, 5000);
        assert_eq!(a.expect_serial.as_deref(), Some("PASS"));
        assert_eq!(a.trace, 7);
    }

    #[test]
    fn c01_43_flags_before_rom() {
        let a = parse_run_args(&[arg("--trace"), arg("2"), arg("game.gb")])
            .expect("flags before the rom path parse");
        assert_eq!(a.rom, "game.gb");
        assert_eq!(a.trace, 2);
    }

    #[test]
    fn c01_43_missing_rom() {
        assert!(parse_run_args(&[]).is_err());
        assert_eq!(err_of(&[arg("--max-cycles"), arg("5")]), "missing rom");
    }

    #[test]
    fn c01_43_unknown_flag() {
        let err = err_of(&[arg("game.gb"), arg("--bogus"), arg("x")]);
        assert_eq!(err, "unknown flag: --bogus");
    }

    #[test]
    fn c01_43_flag_without_value() {
        for flag in ["--max-cycles", "--expect-serial", "--trace"] {
            let err = err_of(&[arg("game.gb"), arg(flag)]);
            assert_eq!(err, format!("{flag} requires a value"));
        }
    }

    #[test]
    fn c01_43_non_numeric_number() {
        for flag in ["--max-cycles", "--trace"] {
            let err = err_of(&[arg("game.gb"), arg(flag), arg("abc")]);
            assert_eq!(err, format!("{flag} expects a number"));
        }
        // A negative or overflowing value is not a valid u64 either.
        let err = err_of(&[arg("game.gb"), arg("--max-cycles"), arg("-1")]);
        assert_eq!(err, "--max-cycles expects a number");
    }

    #[test]
    fn c01_49_ly_fixed_parses_hex_byte() {
        let a = parse_run_args(&[arg("game.gb"), arg("--ly-fixed"), arg("90")])
            .expect("a hex byte parses");
        assert_eq!(a.ly_fixed, Some(0x90));
    }

    #[test]
    fn c01_49_ly_fixed_parses_with_0x_prefix() {
        let a = parse_run_args(&[arg("game.gb"), arg("--ly-fixed"), arg("0x90")])
            .expect("a 0x-prefixed hex byte parses");
        assert_eq!(a.ly_fixed, Some(0x90));
    }

    #[test]
    fn c01_49_ly_fixed_invalid_value_is_usage_error() {
        // A parse error routes to exit code 64 in main (EXIT_BAD_RUN_ARGS).
        let err = err_of(&[arg("game.gb"), arg("--ly-fixed"), arg("zz")]);
        assert_eq!(err, "--ly-fixed expects a hex byte");
    }

    #[test]
    fn e03_16_frames_and_dump_ppm_parse() {
        let a = parse_run_args(&[
            arg("game.gb"),
            arg("--frames"),
            arg("3"),
            arg("--dump-ppm"),
            arg("out.ppm"),
        ])
        .expect("both flags parse");
        assert_eq!(a.frames, Some(3));
        assert_eq!(a.dump_ppm.as_deref(), Some("out.ppm"));
    }

    #[test]
    fn e03_16_missing_values() {
        let err = err_of(&[arg("game.gb"), arg("--frames")]);
        assert_eq!(err, "--frames requires a value");
        let err = err_of(&[arg("game.gb"), arg("--frames"), arg("1"), arg("--dump-ppm")]);
        assert_eq!(err, "--dump-ppm requires a value");
    }

    #[test]
    fn e03_16_frames_not_a_number() {
        let err = err_of(&[arg("game.gb"), arg("--frames"), arg("abc")]);
        assert_eq!(err, "--frames expects a number");
    }

    #[test]
    fn e03_16_dump_ppm_requires_frames() {
        let err = err_of(&[arg("game.gb"), arg("--dump-ppm"), arg("out.ppm")]);
        assert_eq!(err, "--dump-ppm requires --frames");
    }

    #[test]
    fn e03_07_hash_flag_is_off_by_default() {
        let a =
            parse_run_args(&[arg("rom.gb"), arg("--frames"), arg("10")]).expect("frames parses");
        assert!(!a.hash);
    }

    #[test]
    fn e03_07_hash_flag_parses_with_frames() {
        let a = parse_run_args(&[arg("rom.gb"), arg("--frames"), arg("10"), arg("--hash")])
            .expect("hash with frames parses");
        assert!(a.hash);
        assert_eq!(a.frames, Some(10));
    }

    #[test]
    fn e03_07_hash_requires_frames() {
        let err = err_of(&[arg("rom.gb"), arg("--hash")]);
        assert!(err.contains("--hash requires --frames"));
    }

    #[test]
    fn e03_07_hash_combines_with_dump_ppm() {
        let a = parse_run_args(&[
            arg("rom.gb"),
            arg("--frames"),
            arg("10"),
            arg("--hash"),
            arg("--dump-ppm"),
            arg("out.ppm"),
        ])
        .expect("hash with dump-ppm parses");
        assert!(a.hash);
        assert_eq!(a.dump_ppm, Some("out.ppm".to_string()));
    }
}
