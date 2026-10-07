//! Stub executor for the `run` command (task C01_43): no ROM is executed yet.

use std::process::ExitCode;

use crate::run_args::RunArgs;

/// Print a notice and succeed: executing a ROM arrives in a later task. The stub
/// ignores its arguments for now (the fields are read by a later task).
pub fn execute(args: &RunArgs) -> ExitCode {
    println!("run: not implemented yet");
    let RunArgs {
        rom,
        max_cycles,
        expect_serial,
        trace,
    } = args;
    let _ = (rom, max_cycles, expect_serial, trace);
    ExitCode::SUCCESS
}
