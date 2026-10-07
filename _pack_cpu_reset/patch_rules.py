#!/usr/bin/env python
"""Add the 'no test on an unimplemented real opcode' rule to every NOT-DONE C01 task.
Run from the repo root, AFTER C01_48 is DONE:  python _pack_cpu_reset/patch_rules.py --commit
Stdlib only."""
import glob, json, os, re, subprocess, sys

ROOT = os.getcwd()
prog = json.load(open(os.path.join(ROOT, "PROGRESS.json"), encoding="utf-8"))["tasks"]
RULE = ("- FORBIDDEN: a test that ticks a REAL opcode of another group and expects it to be unimplemented "
        "(it breaks when that group is implemented and `done` replays old checks). To test the unimplemented path use "
        "`record_unimplemented` directly (core tests) or `Cpu::debug_set_unimplemented` (other crates). "
        "\"Implement ONLY the LIST\" means: write no code for other opcodes, not: write tests for them.")
OLD = "5. Implement ONLY the LIST. Every other opcode stays unimplemented (the engine records it)."
NEW = "5. Implement ONLY the LIST: write no code for other opcodes. Do NOT write any test about opcodes outside the LIST."
n = 0
for path in sorted(glob.glob(os.path.join(ROOT, "docs", "tasks", "C01", "C01_*.md"))):
    tid = os.path.basename(path)[:-3]
    if prog.get(tid) == "DONE":
        continue
    txt = open(path, encoding="utf-8").read()
    new = txt.replace(OLD, NEW)
    if "FORBIDDEN: a test that ticks a REAL opcode" not in new and "FORBIDDEN: a test that needs a real opcode" not in new:
        new = new.replace("\n## Exit tests", "\n" + RULE + "\n## Exit tests", 1)
    if new != txt:
        open(path, "w", encoding="utf-8", newline="\n").write(new)
        n += 1
print("OK: %d task files updated" % n)
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "tasks: forbid tests on unimplemented real opcodes"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
