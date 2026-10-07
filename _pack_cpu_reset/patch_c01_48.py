#!/usr/bin/env python
"""Insert task C01_48 (fix CLI tests that depend on an unimplemented opcode) before C01_05.
Run from the repo root:  python _pack_cpu_reset/patch_c01_48.py --commit
Stdlib only. Backups: *.bak_c0148"""
import json, os, shutil, subprocess, sys, time

ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
if not (os.path.exists(IDX) and os.path.exists(PROG)):
    sys.exit("ERROR: run from the repo root")
rows = open(IDX, encoding="utf-8").read().splitlines()
if any(r.startswith("C01_48\t") for r in rows):
    sys.exit("ERROR: patch already applied")
for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_c0148")

MD = """# C01_48 - Fix CLI tests that depend on an unimplemented opcode
Role: CODE
Goal: No test relies on a real opcode staying unimplemented. NOP is now implemented, 0x99 will be soon.
Prerequisites: C01_04
Read (max 3 files, give line ranges): crates/puce8gb-cli/src/run/mod.rs (grep -n "fn "); crates/puce8gb-cli/src/run/tests.rs (grep -n "unimplemented\\|Unimplemented"); crates/puce8gb-core/src/cpu/mod.rs (grep -n "unimplemented")
Write only in: crates/puce8gb-core/src/cpu/, crates/puce8gb-cli/
## Why (do not discuss, just do it)
`done` replays every old check. A test that runs a REAL opcode through the CPU and expects it to be unimplemented breaks as soon as a later task implements that opcode. Known cases: `c01_44_valid_rom_stops_on_unimplemented_opcode` (opcode 0x00, broken now) and `c01_46_run_stops_on_unimplemented_opcode` (opcode 0x99, will break later).
## Work
1. In crates/puce8gb-core/src/cpu/mod.rs add a small public seam for tests of other crates:
```rust
    /// Test seam for other crates (task C01_48): store an unimplemented-opcode record
    /// without executing anything. Not used by the engine itself.
    #[doc(hidden)]
    pub fn debug_set_unimplemented(&mut self, opcode: u8, pc: u16) {
        self.unimplemented = Some((opcode, pc));
    }
```
2. In crates/puce8gb-cli/src/run/mod.rs split `run_rom` in two (adapt names to the existing code, keep behaviour): `pub fn run_rom(rom: &[u8], args: &RunArgs) -> Outcome` only builds the `Dmg` (LoadError on failure) and calls `pub fn run_machine(dmg: &mut Dmg, args: &RunArgs) -> Outcome`, which contains the whole M-cycle loop (ticks, serial scan, unimplemented check, trace).
3. In crates/puce8gb-cli/src/run/tests.rs rewrite the setup of the two tests named above (keep their names):
   - build `let mut dmg = Dmg::new(&test_rom(&[])).expect("valid ROM");`
   - `dmg.cpu.debug_set_unimplemented(0x99, 0x0100);`
   - call `run_machine(&mut dmg, &args(400, None))` and assert `Outcome::Unimplemented { opcode: 0x99, pc: 0x0100 }` (the record is already there at the first M-cycle check).
   Do not weaken any other assertion.
4. Add test `c01_48_all_nop_rom_reaches_max_cycles`: `run_rom(&test_rom(&[]), &args(400, None))` returns `Outcome::MaxCycles` (an all-zero ROM is a stream of NOPs; NOP stays implemented forever).
5. Run `grep -rn "unimplemented()" crates --include=*.rs`. For any OTHER test (core or cli) that ticks a real opcode and expects it to be unimplemented, apply the same seam. Tests in machine.rs that only check PC advance (c01_02_*) are fine: leave them.
## Rules
- Identifiers and comments in English, ASCII only.
- Name every NEW unit test fn starting with `c01_48_`. Existing tests keep their names.
- A test is changed only as described above, never to make it pass. Never disable a test.
- Same command failing twice with no change in between: STOP and run `python docs/tools/task.py blocked "<reason>"`.
## Exit tests
- unit tests c01_48_* pass (at least one); all c01_44_ .. c01_47_ tests still pass
- cargo fmt --check, clippy -D warnings, cargo test --workspace (added automatically)
- non-regression: all earlier exit tests (run by `done`)
## CLOSURE
python docs/tools/task.py check   (repeat until SCORE is full)
python docs/tools/task.py done    (commit + push + next task)
"""
SH = """#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-cli c01_48_
ct puce8gb-cli c01_4
t 'seam exists' 'grep -q debug_set_unimplemented crates/puce8gb-core/src/cpu/mod.rs'
score
"""
open(os.path.join(ROOT, "docs", "tasks", "C01", "C01_48.md"), "w", encoding="utf-8", newline="\n").write(MD)
open(os.path.join(ROOT, "docs", "checks", "C01_48.sh"), "w", encoding="utf-8", newline="\n").write(SH)

out = []
for r in rows:
    out.append(r)
    if r.split("\t")[0] == "C01_04":
        out.append("\t".join(["C01_48", "CODE", "0", "docs/tasks/C01/C01_48.md", "Fix CLI tests that depend on an unimplemented opcode"]))
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")

prog = json.load(open(PROG, encoding="utf-8"))
prog["tasks"]["C01_48"] = "TODO"
prog["tasks"]["C01_05"] = "TODO"
prog.update(current_task="C01_48", awaiting_gate=False, failures=0, sessions=0,
            updated=time.strftime("%Y-%m-%d %H:%M:%S"))
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")
print("OK: C01_48 inserted before C01_05. Current task: C01_48.")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "insert C01_48: fix tests depending on unimplemented opcodes"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
