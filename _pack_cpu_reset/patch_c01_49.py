#!/usr/bin/env python
"""Insert task C01_49 (--ly-fixed option for Gameboy Doctor traces) before C01_30.
Run from the repo root, with a CLEAN git tree and NO running Hermes session:
    python _pack_cpu_reset/patch_c01_49.py --commit
Stdlib only. Backups: *.bak_c0149"""
import json, os, re, shutil, subprocess, sys, time

ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
if not (os.path.exists(IDX) and os.path.exists(PROG)):
    sys.exit("ERROR: run from the repo root")
rows = open(IDX, encoding="utf-8").read().splitlines()
ids = [r.split("\t")[0] for r in rows]
if "C01_49" in ids:
    sys.exit("ERROR: patch already applied")
if "C01_30" not in ids:
    sys.exit("ERROR: C01_30 not found in INDEX.tsv")
prev = ids[ids.index("C01_30") - 1]
for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_c0149")

MD = """# C01_49 - Doctor mode: fixed LY value (--ly-fixed)
Role: CODE
Goal: A CLI option makes every read of LY (0xFF44) return a fixed value, so the trace can be compared with Gameboy Doctor (it requires LY = 0x90).
Prerequisites: %s
Read (max 3 files, give line ranges): crates/puce8gb-core/src/bus.rs (grep -n "FF44\\|ly"); crates/puce8gb-core/src/machine.rs (grep -n "pub fn"); crates/puce8gb-cli/src/run/mod.rs (grep -n "RunArgs\\|fn parse\\|--trace")
Write only in: crates/puce8gb-core/src/, crates/puce8gb-cli/
## Why (do not discuss, just do it)
Gameboy Doctor expects every read of 0xFF44 to return 0x90. Our LY stub counts, so the first LDH A,(0x44) already diverges from the reference trace. This option is a debug tool only: when it is not given, behaviour is unchanged.
## Work
1. Core, bus: add a private field `ly_override: Option<u8>` (default None). In the read of address 0xFF44, when `ly_override` is Some(v) return v BEFORE the normal LY logic. Add `pub fn debug_set_ly_override(&mut self, v: Option<u8>)` on the bus and a forwarding `pub fn debug_set_ly_override(&mut self, v: Option<u8>)` on `Dmg` (mark both `#[doc(hidden)]`, comment: debug tool for trace comparison, task C01_49). Adapt names to the existing code.
2. CLI: add option `--ly-fixed <HEX>` to the argument parser: a hex byte, with or without a 0x prefix (`90` and `0x90` are the same). Invalid value: usage error, exit code 64, like the other options. Add `ly_fixed: Option<u8>` to the args struct.
3. In `run_rom` (not in run_machine), right after building the Dmg, if `args.ly_fixed` is Some(v) call `dmg.debug_set_ly_override(Some(v))`.
4. Tests (names start with c01_49_):
   - core: with the override Some(0x90) a read of 0xFF44 returns 0x90 before AND after the machine ticked many M-cycles; setting it back to None removes the override (just check the call compiles and the value read after 100000 ticks is not forced: do NOT assert a specific LY value).
   - cli: parsing `--ly-fixed 90` gives Some(0x90); `--ly-fixed 0x90` gives Some(0x90); `--ly-fixed zz` is a usage error (code 64 path).
5. Do NOT change the default LY behaviour and do NOT touch any CPU file.
## Rules
- Identifiers and comments in English, ASCII only.
- FORBIDDEN: a test that ticks a REAL opcode of another group and expects it to be unimplemented. Use `record_unimplemented` (core) or `Cpu::debug_set_unimplemented` (other crates).
- A test is changed only if it contradicts the task, never to make it pass. Never disable a test.
- Same command failing twice with no change in between: STOP and run `python docs/tools/task.py blocked "<reason>"`.
## Exit tests
- unit tests c01_49_* pass in puce8gb-core and in puce8gb-cli (at least one in each)
- cargo fmt --check, clippy -D warnings, cargo test --workspace (added automatically)
- non-regression: all earlier exit tests (run by `done`)
## CLOSURE
python docs/tools/task.py check   (repeat until SCORE is full)
python docs/tools/task.py done    (commit + push + next task)
""" % prev

SH = """#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_49_
ct puce8gb-cli c01_49_
t 'option exists' 'grep -rq "ly-fixed" crates/puce8gb-cli/src'
t 'override exists' 'grep -rq "ly_override" crates/puce8gb-core/src'
score
"""
open(os.path.join(ROOT, "docs", "tasks", "C01", "C01_49.md"), "w", encoding="utf-8", newline="\n").write(MD)
open(os.path.join(ROOT, "docs", "checks", "C01_49.sh"), "w", encoding="utf-8", newline="\n").write(SH)

out = []
for r in rows:
    if r.split("\t")[0] == "C01_30":
        out.append("\t".join(["C01_49", "CODE", "0", "docs/tasks/C01/C01_49.md", "Doctor mode: fixed LY value (--ly-fixed)"]))
    out.append(r)
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")

# C01_30 now depends on C01_49
p30 = os.path.join(ROOT, "docs", "tasks", "C01", "C01_30.md")
t30 = open(p30, encoding="utf-8").read()
new30, n = re.subn(r"^Prerequisites:.*$", "Prerequisites: C01_49", t30, count=1, flags=re.M)
if n != 1:
    sys.exit("ERROR: no Prerequisites line in C01_30.md")
open(p30, "w", encoding="utf-8", newline="\n").write(new30)

prog = json.load(open(PROG, encoding="utf-8"))
st = prog["tasks"]
st["C01_49"] = "TODO"
if st.get("C01_30") != "DONE":
    st["C01_30"] = "TODO"
prog.update(current_task="C01_49", awaiting_gate=False, failures=0, sessions=0,
            updated=time.strftime("%Y-%m-%d %H:%M:%S"))
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")
print("OK: C01_49 inserted before C01_30 (prerequisite %s). Current task: C01_49." % prev)
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "insert C01_49: --ly-fixed option for Gameboy Doctor traces"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
