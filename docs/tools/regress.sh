#!/usr/bin/env bash
# Manual full non-regression: runs the exit tests of every DONE task.
cd "$(dirname "$0")/../.." || exit 2
python3 - <<'PY'
import json, subprocess, sys
prog = json.load(open("PROGRESS.json"))
bad = 0
for tid, st in prog["tasks"].items():
    if st != "DONE":
        continue
    out = subprocess.run(["bash", "docs/checks/%s.sh" % tid], capture_output=True, text=True).stdout
    last = [l for l in out.splitlines() if l.startswith("SCORE:")]
    print(tid, last[-1] if last else "NO SCORE")
    bad += 0 if last and last[-1].split()[1].split("/")[0] == last[-1].split("/")[1] else 1
sys.exit(1 if bad else 0)
PY
