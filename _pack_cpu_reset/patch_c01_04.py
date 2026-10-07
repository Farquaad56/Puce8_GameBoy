#!/usr/bin/env python
"""Split C01_04 into small tasks C01_42..C01_47 (+ C01_04 becomes a verification GATE).
Run from the repo root:  python _pack_cpu_reset/patch_c01_04.py [--commit]
Stdlib only. Backups: *.bak_c0104"""
import json, os, shutil, subprocess, sys, time

ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
if not (os.path.exists(IDX) and os.path.exists(PROG)):
    sys.exit("ERROR: run from the repo root")
rows = open(IDX, encoding="utf-8").read().splitlines()
if any(r.startswith("C01_42\t") for r in rows):
    sys.exit("ERROR: patch already applied")
for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_c0104")

CLOSURE = """## CLOSURE
python docs/tools/task.py check   (repeat until SCORE is full)
python docs/tools/task.py done    (commit + push + next task)"""

def rules(tid, scope="core"):
    return """- Identifiers and comments in English, ASCII only. Zero invention: unknown => UNKNOWN - to confirm (open_questions.md).
- Name every unit test fn starting with `%s_` (the check filters on it).
- %s
- A test is changed only if it contradicts the notes, never to make it pass. Never disable a test.
- FORBIDDEN: a test that needs a real opcode to stay unimplemented (it breaks as soon as a later task implements it, and `done` replays old checks). Test pure functions instead.""" % (
        tid.lower(),
        "Core crate rules apply (no I/O, no allocation in tick())." if scope == "core" else "Only the CLI touches files, stdout and stderr. Keep each file under 150 lines: split into modules.")

def task(tid, title, role, prereq, goal, read, write, work, exits, gate=False, scope="cli"):
    md = """# %s - %s
Role: %s
Goal: %s
Prerequisites: %s
Read (max 3 files, give line ranges): %s
Write only in: %s
## Work
%s
## Rules
%s
## Exit tests
%s
- cargo fmt --check, clippy -D warnings, cargo test --workspace (added automatically)
- non-regression: all earlier exit tests (run by `done`)
%s
%s""" % (tid, title, role, goal, prereq, read, write, work, rules(tid, scope),
         "\n".join("- " + e for e in exits), CLOSURE,
         "\nGATE after done: print the GATE block of AGENTS.md, then STOP and wait for the user.\n" if gate else "")
    return md

CLI_READ = "docs/annexes/notes/09_test_roms.md (grep first); crates/puce8gb-cli/src/main.rs (grep -n \"fn \")"
CLI = "crates/puce8gb-cli/"

T = {}
T["C01_42"] = (task("C01_42", "Harden C01_01 tests (no dependency on an unimplemented opcode)", "CODE", "C01_03",
    "The c01_01_ tests keep passing when later tasks implement opcodes.",
    "crates/puce8gb-core/src/cpu/mod.rs (grep -n \"unimplemented\" and \"fn tick\"); docs/annexes/decisions.md (C_00 only)",
    "crates/puce8gb-core/src/cpu/",
    """1. grep -n "c01_01_" crates/puce8gb-core/src/cpu/*.rs and list the tests that tick an opcode and expect it to be recorded as unimplemented.
2. Make that recording path testable WITHOUT a real opcode: extract (if not already separate) a private `fn record_unimplemented(&mut self, opcode: u8)` that stores `(opcode, address of the opcode byte)` and ends the instruction. `tick()` calls it when no group claims the opcode.
3. Rewrite those c01_01_ tests to call `record_unimplemented` directly (set pc first) and assert: the record, step back to 0, at_boundary() true. Do NOT rename or delete any c01_01_ test, only change its setup.
4. Add test `c01_42_empty_dispatch_fetches_one_byte`: only if it can be written without a real opcode (e.g. through a helper that runs the fetch with an empty chain). Otherwise add `c01_42_record_unimplemented_keeps_pc` and say why in a comment.
5. Do not touch any other file.""",
    ["unit tests c01_42_* pass (at least one)", "unit tests c01_01_* still pass"], scope="core"),
    "ct puce8gb-core c01_42_\nct puce8gb-core c01_01_")

T["C01_43"] = (task("C01_43", "CLI run: argument parsing", "CODE", "C01_42",
    "`run <rom> [--max-cycles N] [--expect-serial TEXT] [--trace N]` is parsed into a struct. No ROM is executed yet.",
    CLI_READ, CLI,
    """1. Create crates/puce8gb-cli/src/run_args.rs (`mod run_args;` in main.rs): `pub struct RunArgs { pub rom: String, pub max_cycles: u64, pub expect_serial: Option<String>, pub trace: u64 }`. Defaults: max_cycles = 100_000_000 (T-cycles), trace = 0, expect_serial = None.
2. `pub fn parse_run_args(args: &[String]) -> Result<RunArgs, String>`; `args` = everything after the word `run`. First positional = rom path. Errors (return Err with a short message): missing rom, unknown flag, flag without value, non-numeric number.
3. In main.rs add the `run` arm: on Err print `error: <msg>` on stderr and return ExitCode::from(64) (64 = bad run arguments; 2 is reserved for timeout). On Ok call a stub `run::execute(&RunArgs) -> ExitCode` in a new run.rs that prints "run: not implemented yet" and returns ExitCode::SUCCESS.
4. Tests (pure, no files): defaults; all flags together; each error case.""",
    ["unit tests c01_43_* pass (at least one)"]),
    "ct puce8gb-cli c01_43_\nt 'run_args.rs exists' 'test -f crates/puce8gb-cli/src/run_args.rs'")

T["C01_44"] = (task("C01_44", "CLI run: load the ROM and run the machine loop", "CODE", "C01_43",
    "run loads the ROM, runs it for max-cycles T-cycles, exits with the right code. No serial yet.",
    CLI_READ, CLI,
    """1. In run.rs make the logic testable: `pub enum Outcome { LoadError, MaxCycles }` and `pub fn run_rom(rom: &[u8], args: &RunArgs) -> Outcome` (no file access inside). `pub fn exit_code(o: &Outcome, args: &RunArgs) -> u8`: LoadError => 3; MaxCycles => 2 if args.expect_serial.is_some() else 0.
2. run_rom: `Dmg::new(rom)`; Err(_) => LoadError. Then loop in M-cycles: `for _ in 0..(args.max_cycles / 4) { for _ in 0..4 { dmg.tick(); } }` (Dmg::tick is one dot; the CPU runs every 4th dot, task C01_02). Keep the loop body small: later tasks add one call each.
3. execute(): read the file with std::fs::read (error => message on stderr, exit 3), call run_rom, return ExitCode::from(exit_code(..)).
4. Tests (pure, synthetic ROM): a helper `test_rom(code: &[u8]) -> Vec<u8>` builds a ROM of 0x150+ bytes (copy the style of the helper in main.rs tests); ROM too short => LoadError; valid ROM with max_cycles 400 => MaxCycles; exit_code mapping for each case.""",
    ["unit tests c01_44_* pass (at least one)"]),
    "ct puce8gb-cli c01_44_")

T["C01_45"] = (task("C01_45", "CLI run: serial output and --expect-serial", "CODE", "C01_44",
    "Serial bytes are echoed on stdout; the run stops early on the expected text or on the failure marker.",
    CLI_READ + "; crates/puce8gb-core/src/machine.rs (grep -n take_serial_output)", CLI,
    """1. Pure helper in run.rs: `pub enum Scan { Continue, Found, Failed }` and `pub fn scan_serial(collected: &mut String, new_bytes: &[u8], expect: Option<&str>) -> Scan`: append the bytes (lossy UTF-8), then Found if `expect` is contained, else Failed if the text contains the constant `FAIL_MARKER`, else Continue. Define `const FAIL_MARKER: &str = "Failed";` and verify with grep in note 09_test_roms.md that the Blargg ROMs print it; not in the note => keep it, add UNKNOWN - to confirm in open_questions.md.
2. In the run_rom loop, once per M-cycle: `let n = dmg.take_serial_output(&mut buf)` (fixed `[u8; 64]` buffer), call scan_serial, echo the new text on stdout (print! and flush). Extend Outcome with `Found` and `Failed` and the serial text (`String`) when useful.
3. exit_code: Found => 0; Failed => 1; MaxCycles => 2 if an expectation exists else 0.
4. Tests: scan_serial for Continue, Found (text split across two calls), Failed; and one machine-level test: build a Dmg from a synthetic ROM, write the bytes directly with `dmg.bus.write(0xFF01, b'A')` then `dmg.bus.write(0xFF02, 0x81)`, then check that the drain + scan sees "A". Do not run any instruction.""",
    ["unit tests c01_45_* pass (at least one)"]),
    "ct puce8gb-cli c01_45_")

T["C01_46"] = (task("C01_46", "CLI run: report an unimplemented opcode (exit 4)", "CODE", "C01_45",
    "When the CPU records an unimplemented opcode, run stops and prints it.",
    CLI_READ + "; crates/puce8gb-core/src/cpu/mod.rs (grep -n \"pub fn unimplemented\")", CLI,
    """1. Pure helpers in run.rs: `pub fn unimplemented_message(opcode: u8, pc: u16) -> String` returning exactly `UNIMPLEMENTED opcode 0x%02X at PC=0x%04X` (uppercase hex, 2 and 4 digits) and `pub fn check_unimplemented(rec: Option<(u8, u16)>) -> Option<Outcome>`.
2. Extend Outcome with `Unimplemented { opcode: u8, pc: u16 }`; exit_code => 4. In the loop, once per M-cycle after the ticks: `check_unimplemented(dmg.cpu.unimplemented())`; on Some, return it. The serial scan keeps priority (check the serial first).
3. execute() prints the message on stdout.
4. Tests: message format for (0x00, 0x0100) and (0xFF, 0xC000); check_unimplemented None/Some; exit_code => 4. NO test that runs a real opcode through the CPU (it breaks when that opcode gets implemented).""",
    ["unit tests c01_46_* pass (at least one)"]),
    "ct puce8gb-cli c01_46_")

T["C01_47"] = (task("C01_47", "CLI run: --trace", "CODE", "C01_46",
    "--trace N prints one line per instruction on stderr, Gameboy-Doctor style.",
    CLI_READ + "; crates/puce8gb-core/src/cpu/registers.rs (grep -n \"pub fn\")", CLI,
    """1. Pure helper in run.rs: `pub fn trace_line(a: u8, f: u8, b: u8, c: u8, d: u8, e: u8, h: u8, l: u8, sp: u16, pc: u16, mem: [u8; 4]) -> String` returning exactly `A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100 PCMEM:00,C3,50,01` (uppercase hex).
2. In the run_rom loop, at the START of each M-cycle: if `trace_left > 0 && dmg.cpu.at_boundary()` then eprintln!(trace_line(...)) using the public register fields of dmg.cpu and `dmg.bus.peek(pc.wrapping_add(i))` for the 4 memory bytes, then decrement trace_left. One line per instruction, never during its extra M-cycles.
3. Tests: trace_line exact string for the example above and for a second set of values with SP and PC wrapping edge ($FFFF).""",
    ["unit tests c01_47_* pass (at least one)"]),
    "ct puce8gb-cli c01_47_")

T["C01_04"] = (task("C01_04", "CLI run: verification and GATE", "CODE", "C01_47",
    "Check by hand that the whole run command works, then stop for the user. No new code unless something is broken.",
    CLI_READ, CLI,
    """1. Run: cargo run -q --release -p puce8gb-cli -- run "roms/test-roms/blargg/cpu_instrs/individual/06-ld r,r.gb" --max-cycles 400 --trace 3
2. Expected: the first stderr line is `A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100 PCMEM:..` and the exit code is NOT 3 and NOT 64. Copy the 3 trace lines and the exit code into the GATE block.
3. If something is broken: fix it in the smallest module with a pure unit test named c01_04_*. Otherwise change nothing.""",
    ["CLI run prints the first trace line and does not exit with 3 or 64"], gate=True),
    "t 'run prints first trace line' 'r=$(rom cpu_instrs 06-); test -n \"$r\" && cargo run -q --release -p puce8gb-cli -- run \"$r\" --max-cycles 400 --trace 1 2>&1 | grep -q \"A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100\"'\nt 'run exit code is not 3 or 64' 'r=$(rom cpu_instrs 06-); cargo run -q --release -p puce8gb-cli -- run \"$r\" --max-cycles 400 >/dev/null 2>&1; rc=$?; test $rc -ne 3 && test $rc -ne 64'")

order = ["C01_42", "C01_43", "C01_44", "C01_45", "C01_46", "C01_47"]
titles = {"C01_42": "Harden C01_01 tests (no dependency on an unimplemented opcode)",
          "C01_43": "CLI run: argument parsing", "C01_44": "CLI run: load the ROM and run the machine loop",
          "C01_45": "CLI run: serial output and --expect-serial", "C01_46": "CLI run: report an unimplemented opcode (exit 4)",
          "C01_47": "CLI run: --trace", "C01_04": "CLI run: verification and GATE"}
os.makedirs(os.path.join(ROOT, "docs", "tasks", "C01"), exist_ok=True)
for tid, (md, check) in T.items():
    open(os.path.join(ROOT, "docs", "tasks", "C01", tid + ".md"), "w", encoding="utf-8", newline="\n").write(md)
    sh = '#!/usr/bin/env bash\nsource "$(dirname "$0")/lib.sh"\n' + check + "\nscore\n"
    open(os.path.join(ROOT, "docs", "checks", tid + ".sh"), "w", encoding="utf-8", newline="\n").write(sh)

# INDEX: new rows right after C01_03, C01_04 keeps its place (now gate verification)
out = []
for r in rows:
    f = r.split("\t")
    if f[0] == "C01_04":
        f[2] = "1"
        f[4] = titles["C01_04"]
        r = "\t".join(f)
    out.append(r)
    if f[0] == "C01_03":
        for tid in order:
            out.append("\t".join([tid, "CODE", "0", "docs/tasks/C01/%s.md" % tid, titles[tid]]))
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")

prog = json.load(open(PROG, encoding="utf-8"))
for tid in order:
    prog["tasks"][tid] = "TODO"
prog["tasks"]["C01_04"] = "TODO"
prog.update(current_task="C01_42", awaiting_gate=False, failures=0, sessions=0,
            updated=time.strftime("%Y-%m-%d %H:%M:%S"))
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")
print("OK: C01_04 split into C01_42..C01_47 + verification gate C01_04. Current task: C01_42.")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "split C01_04 into C01_42..C01_47"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
