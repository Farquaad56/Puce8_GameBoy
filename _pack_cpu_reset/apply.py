#!/usr/bin/env python3
"""Install the CPU restart pack (epic C01) into the repo. Run from the repo root:
    python3 _pack_cpu_reset/apply.py [--commit]
Stdlib only. Makes .bak_cpu_reset backups. Safe to read before running."""
import json, os, re, shutil, subprocess, sys, time

PACK = os.path.dirname(os.path.abspath(__file__))
ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
OLD_IDS = ["E02_%02d" % n for n in range(2, 47)]


def die(msg):
    print("ERROR: " + msg)
    sys.exit(1)


if not (os.path.exists(IDX) and os.path.exists(PROG)):
    die("run this from the repo root (PROGRESS.json and docs/tasks/INDEX.tsv not found)")
rows = open(IDX, encoding="utf-8").read().splitlines()
if any(r.startswith("C01_01\t") for r in rows):
    die("pack already applied (C01_01 is in INDEX.tsv)")

for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_cpu_reset")

# 1. move old E02_02..E02_46 tasks and checks away
for sub, ext, dest in (("tasks/E02", ".md", "tasks/_old_e02"), ("checks", ".sh", "checks/_old_e02")):
    os.makedirs(os.path.join(ROOT, "docs", dest), exist_ok=True)
    for i in OLD_IDS:
        src = os.path.join(ROOT, "docs", sub, i + ext)
        if os.path.exists(src):
            shutil.move(src, os.path.join(ROOT, "docs", dest, i + ext))

# 2. install new tasks and checks (all contained in payload.json)
payload = json.load(open(os.path.join(PACK, "payload.json"), encoding="utf-8"))
os.makedirs(os.path.join(ROOT, "docs", "tasks", "C01"), exist_ok=True)
new_rows = payload["tasks/C01_INDEX.part.tsv"].splitlines()
ids = [r.split("\t")[0] for r in new_rows]
for rel, content in payload.items():
    if rel.endswith("INDEX.part.tsv"):
        continue
    dst = os.path.join(ROOT, "docs", *rel.split("/"))
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    open(dst, "w", encoding="utf-8", newline="\n").write(content)

# 3. INDEX.tsv: drop old rows, insert new ones after E02_01
out = []
inserted = False
for r in rows:
    if r.split("\t")[0] in OLD_IDS:
        continue
    out.append(r)
    if r.startswith("E02_01\t"):
        out.extend(new_rows)
        inserted = True
if not inserted:
    die("row E02_01 not found in INDEX.tsv")
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")

# 4. PROGRESS.json
prog = json.load(open(PROG, encoding="utf-8"))
for i in OLD_IDS:
    prog["tasks"].pop(i, None)
for i in ids:
    prog["tasks"][i] = "TODO"
prog.update(current_task=ids[0], awaiting_gate=False, failures=0, sessions=0, updated=time.strftime("%Y-%m-%d %H:%M:%S"))
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")

# 5. decision C_00
dec = os.path.join(ROOT, "docs", "annexes", "decisions.md")
txt = open(dec, encoding="utf-8").read()
if "\n## C_00\n" not in txt:
    add = open(os.path.join(PACK, "decision_C_00.md"), encoding="utf-8").read()
    open(dec, "w", encoding="utf-8", newline="\n").write(txt.rstrip("\n") + "\n" + add)

# 6. AGENTS.md rule
ag = os.path.join(ROOT, "AGENTS.md")
a = open(ag, encoding="utf-8").read()
old = "Cycle accurate: one micro-op = exactly one bus access."
new = "Cycle accurate: one tick = one M-cycle = AT MOST one bus access (decision C_00; internal M-cycles have none)."
if old in a:
    open(ag, "w", encoding="utf-8", newline="\n").write(a.replace(old, new))
else:
    print("warn: AGENTS.md rule not found, edit it by hand (see decision C_00)")

print("OK: %d tasks installed (%s .. %s), old E02_02..E02_46 moved to _old_e02." % (len(ids), ids[0], ids[-1]))
print("Next: python3 docs/tools/task.py status   then commit before the first session.")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "cpu restart pack: tasks C01_01..C01_41, decision C_00"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
