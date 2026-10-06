#!/usr/bin/env python3
"""Split phase E02 (16 tasks) into 46 smaller tasks.

Usage (repo root):  python3 docs/tools/migrate_e02.py <dir with E02_*.md + manifest.json> [repo_root]
Updates: task files, docs/tasks/INDEX.tsv, docs/checks/E02_*.sh, PROGRESS.json,
and 'Prerequisites:' lines of other tasks that cite an old E02 id.
Backups: INDEX.tsv.bak2, PROGRESS.json.bak2, docs/checks/_old_e02/.
"""
import json
import os
import posixpath
import re
import shutil
import sys

src = sys.argv[1]
root = sys.argv[2] if len(sys.argv) > 2 else "."
INDEX = os.path.join(root, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(root, "PROGRESS.json")
CHK = os.path.join(root, "docs", "checks")
OLD = ["E02_%02d" % i for i in range(1, 17)]
NEW = ["E02_%02d" % i for i in range(1, 47)]
LAST = dict(zip(OLD, ["E02_%02d" % n for n in
                      (2, 3, 6, 8, 11, 13, 15, 18, 20, 24, 27, 31, 35, 39, 42, 46)]))
MOVED = {"E02_02": "E02_03"}  # same content, new id: keep its old check script


def die(m):
    print("ABORT: " + m)
    sys.exit(1)


rows = []
with open(INDEX, encoding="utf-8") as f:
    for line in f:
        line = line.rstrip("\n")
        if line and not line.startswith("#"):
            rows.append(line.split("\t"))
ids = [r[0] for r in rows]
if "E02_46" in ids:
    die("already migrated (E02_46 is in INDEX.tsv)")
for i in OLD:
    if i not in ids:
        die("%s not found in INDEX.tsv" % i)
for i in NEW:
    if not os.path.exists(os.path.join(src, i + ".md")):
        die("missing %s.md in %s" % (i, src))
if not os.path.exists(os.path.join(src, "manifest.json")):
    die("missing manifest.json in " + src)
manifest = json.load(open(os.path.join(src, "manifest.json")))

shutil.copy(INDEX, INDEX + ".bak2")
shutil.copy(PROG, PROG + ".bak2")
bk = os.path.join(CHK, "_old_e02")
os.makedirs(bk, exist_ok=True)
for i in OLD:
    p = os.path.join(CHK, i + ".sh")
    if os.path.exists(p):
        shutil.copy(p, os.path.join(bk, i + ".sh"))


def old_text(i):
    p = os.path.join(bk, i + ".sh")
    return open(p, encoding="utf-8").read() if os.path.exists(p) else None


def write_check(i, text):
    with open(os.path.join(CHK, i + ".sh"), "w", encoding="utf-8", newline="\n") as f:
        f.write(text if text.endswith("\n") else text + "\n")


def unit_template(i):
    m = manifest[i]
    L = ["#!/usr/bin/env bash", 'source "$(dirname "$0")/lib.sh"']
    for p in m["files"]:
        L.append("t \"%s exists\" 'test -f %s'" % (posixpath.basename(p), p))
    L.append("ct %s %s_" % (m["crate"], i.lower()))
    L.append("score")
    return "\n".join(L)


def rom_script(i, label):
    """Rebuild a per-ROM script from the old multi-ROM script (matched by the ROM label)."""
    warn = None
    for o in ("E02_13", "E02_14", "E02_15", "E02_16"):
        t = old_text(o)
        if t and label in t:
            lines = t.splitlines()
            head, hit, k = [], [], 0
            while k < len(lines):
                block = [lines[k]]
                while block[-1].rstrip().endswith("\\") and k + 1 < len(lines):
                    k += 1
                    block.append(lines[k])
                k += 1
                j = " ".join(block)
                if re.match(r"\s*(ct\s|score\b)", j):
                    continue
                if "test ROM passes" in j or re.match(r"\s*t\s", j):
                    if label in j:
                        hit.extend(block)
                    continue
                head.extend(block)
            if hit:
                return "\n".join(head + hit + ["score"]), None
            warn = "label found in %s but no 't' line matched" % o
    msg = warn or "label not found in any old E02_13..16 script"
    return ('#!/usr/bin/env bash\nsource "$(dirname "$0")/lib.sh"\n'
            't "REBUILD THIS CHECK for %s (%s)" false\nscore' % (label, msg)), msg


warnings = []
for i in NEW:
    m = manifest[i]
    if i in MOVED.values():
        o = [k for k, v in MOVED.items() if v == i][0]
        t = old_text(o)
        if t:
            write_check(i, t.replace(o, i).replace(o.lower() + "_", i.lower() + "_"))
            continue
    if m.get("rom"):
        t, w = rom_script(i, m["rom"])
        write_check(i, t)
        if w:
            warnings.append("%s: %s" % (i, w))
    else:
        write_check(i, unit_template(i))

d = posixpath.dirname(rows[ids.index("E02_01")][3])
if not os.path.isdir(os.path.join(root, d)):
    die("task dir %s missing" % d)
block = []
for i in NEW:
    shutil.copy(os.path.join(src, i + ".md"), os.path.join(root, d, i + ".md"))
    text = open(os.path.join(src, i + ".md"), encoding="utf-8").read()
    title = re.match(r"# \S+ - (.*)", text.splitlines()[0]).group(1)
    role = re.search(r"^Role:\s*(\w+)", text, re.M).group(1)
    gate = "1" if re.search(r"^GATE after done", text, re.M) else "0"
    block.append([i, role, gate, posixpath.join(d, i + ".md"), title])

out = []
for r in rows:
    if r[0] == "E02_01":
        out.extend(block)
    elif r[0] in OLD:
        continue
    else:
        out.append(r)
with open(INDEX, "w", encoding="utf-8", newline="\n") as f:
    f.write("# id\trole\tgate\tpath\ttitle\n")
    for r in out:
        f.write("\t".join(r) + "\n")

# prerequisites of other tasks that cite an old E02 id -> last new task of that group
fixed = []
for r in out:
    if r[0] in NEW:
        continue
    p = os.path.join(root, r[3])
    if not os.path.exists(p):
        continue
    t = open(p, encoding="utf-8").read()

    def fix(m):
        return re.sub(r"\bE02_\d\d\b", lambda x: LAST.get(x.group(0), x.group(0)), m.group(0))

    t2 = re.sub(r"^Prerequisites:.*$", fix, t, flags=re.M)
    if t2 != t:
        open(p, "w", encoding="utf-8", newline="\n").write(t2)
        fixed.append(r[0])

prog = json.load(open(PROG, encoding="utf-8"))
was_done = [i for i in OLD if prog["tasks"].get(i) == "DONE"]
tasks = {}
for r in out:
    tasks[r[0]] = "TODO" if r[0] in NEW else prog["tasks"].get(r[0], "TODO")
prog["tasks"] = tasks
prog["current_task"] = next((r[0] for r in out if tasks.get(r[0]) != "DONE"), None)
prog.update(awaiting_gate=False, failures=0, sessions=0)
with open(PROG, "w", encoding="utf-8") as f:
    json.dump(prog, f, indent=2)
    f.write("\n")
print("OK. INDEX rows: %d | current_task: %s" % (len(out), prog["current_task"]))
if fixed:
    print("Prerequisites updated in: " + ", ".join(fixed))
if was_done:
    print("WARNING: these old E02 tasks were DONE and are now TODO: " + ", ".join(was_done))
for w in warnings:
    print("WARNING check script: " + w)
print("Check: python3 docs/tools/task.py status")
