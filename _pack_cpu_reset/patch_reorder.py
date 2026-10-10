#!/usr/bin/env python
"""Reorder epic C01: all instruction tasks first, Blargg ROM tasks after.
Reason: every cpu_instrs ROM shares runtime code that uses CB-prefixed opcodes.
New order after C01_15:
  17 18 22 23 25 26 (instructions)  then  16 19 20 21 24 (ROMs)  then the rest unchanged.
Run from the repo root:
  python _pack_cpu_reset/patch_reorder.py            (dry run, prints the plan)
  python _pack_cpu_reset/patch_reorder.py --apply --commit
Stdlib only. Backups: *.bak_reorder"""
import json, os, re, shutil, subprocess, sys, time

ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
if not (os.path.exists(IDX) and os.path.exists(PROG)):
    sys.exit("ERROR: run from the repo root")

MOVE = ["C01_%02d" % n for n in (16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26)]
NEW_BLOCK = ["C01_%02d" % n for n in (17, 18, 22, 23, 25, 26, 16, 19, 20, 21, 24)]
rows = open(IDX, encoding="utf-8").read().splitlines()
ids = [r.split("\t")[0] for r in rows]
for t in MOVE:
    if t not in ids:
        sys.exit("ERROR: %s not in INDEX.tsv (already patched?)" % t)
pos = [ids.index(t) for t in MOVE]
if pos != list(range(pos[0], pos[0] + len(MOVE))):
    sys.exit("ERROR: C01_16..C01_26 are not contiguous in INDEX.tsv: %s" % pos)
by_id = {r.split("\t")[0]: r for r in rows}
start = pos[0]
new_rows = rows[:start] + [by_id[t] for t in NEW_BLOCK] + rows[start + len(MOVE):]
new_ids = [r.split("\t")[0] for r in new_rows]

# predecessor of each task of the block in the new order, and of the task right after the block
prev = {}
for i, t in enumerate(NEW_BLOCK):
    prev[t] = new_ids[start + i - 1]
after = new_ids[start + len(MOVE)] if start + len(MOVE) < len(new_ids) else None
if after:
    prev[after] = NEW_BLOCK[-1]

print("New order around the block:")
for i in range(start - 1, min(len(new_ids), start + len(MOVE) + 2)):
    print("  %s" % new_ids[i])
print("Prerequisites that will be rewritten:")
pre_re = re.compile(r"^Prerequisites:.*$", re.M)
plan = []
for t, p in prev.items():
    path = os.path.join(ROOT, by_id[t].split("\t")[3].replace("/", os.sep))
    if not os.path.exists(path):
        sys.exit("ERROR: task file missing: " + path)
    txt = open(path, encoding="utf-8").read()
    m = pre_re.search(txt)
    if not m:
        sys.exit("ERROR: no 'Prerequisites:' line in " + path)
    print("  %s: '%s' -> 'Prerequisites: %s'" % (t, m.group(0), p))
    plan.append((path, txt, "Prerequisites: " + p))

prog = json.load(open(PROG, encoding="utf-8"))
st = prog["tasks"]
print("Statuses now:", {t: st.get(t) for t in MOVE})
if "--apply" not in sys.argv:
    print("DRY RUN only. Add --apply --commit to write.")
    sys.exit(0)

for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_reorder")
for path, txt, line in plan:
    open(path, "w", encoding="utf-8", newline="\n").write(pre_re.sub(line, txt, count=1))
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(new_rows) + "\n")
if st.get("C01_16") == "BLOCKED":
    st["C01_16"] = "TODO"
nxt = next((t for t in new_ids if t.startswith("C01_") and st.get(t) != "DONE"), None)
prog.update(current_task=nxt, awaiting_gate=False, failures=0, sessions=0,
            updated=time.strftime("%Y-%m-%d %H:%M:%S"))
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")
print("OK: reordered. Current task: %s" % nxt)
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "reorder C01: instructions before Blargg ROM tasks"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
