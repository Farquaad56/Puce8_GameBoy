//! Argument parsing for the `run` command (task C01_43):
//! `run <rom> [--max-cycles N] [--expect-serial TEXT] [--trace N] [--ly-fixed HEX]`.

/// Default execution budget in T-cycles: 100_000_000.
const DEFAULT_MAX_CYCLES: u64 = 100_000_000;

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

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--max-cycles" => max_cycles = number(args, &mut i, "--max-cycles")?,
            "--trace" => trace = number(args, &mut i, "--trace")?,
            "--expect-serial" => expect_serial = Some(value(args, &mut i, "--expect-serial")?),
            "--ly-fixed" => ly_fixed = Some(hex_byte(args, &mut i, "--ly-fixed")?),
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
    Ok(RunArgs {
        rom,
        max_cycles,
        expect_serial,
        trace,
        ly_fixed,
    })
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
}
