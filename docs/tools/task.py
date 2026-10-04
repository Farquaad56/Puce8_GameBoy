#!/usr/bin/env python3
"""Sequential task driver for the Game Boy emulator agent system.

One task at a time. Python stdlib only. Run from anywhere inside the repo.
Agent commands : show | check | done | blocked "<reason>"
Human commands (need keyboard YES on /dev/tty) : approve | reopen [id] | unblock | addfix <test name> | status
Driver command : begin | role
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
INDEX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
CHECKS = os.path.join(ROOT, "docs", "checks")
OPENQ = os.path.join(ROOT, "docs", "annexes", "open_questions.md")
DECISIONS = os.path.join(ROOT, "docs", "annexes", "decisions.md")
MAX_FAILS = 5
MAX_SESSIONS = 4
ALWAYS_OK = ("PROGRESS.json", "docs/annexes/open_questions.md", "Cargo.lock")
CLOSURE = [
    ("cargo fmt --check", "cargo fmt --all -- --check"),
    ("cargo clippy -D warnings", "cargo clippy --workspace --all-targets -- -D warnings"),
    ("cargo test (workspace)", "cargo test --workspace -q"),
]

FIX_TEMPLATE = """# {id} - Fix test: {name}
Role: TEST
Goal: Make test "{name}" pass in the suite without regressing any other test.
Prerequisites: none
Read (max 3 files, give line ranges): docs/annexes/notes/09_test_roms.md; the one note about the failing subsystem
Write only in: crates/
## Work
1. Run: cargo run -q --release -p puce8gb-cli -- suite --only "{name}"  (note the failure text).
2. Find the smallest reproduction (one instruction, one register, one cycle). Write a failing unit test first, named fix_{num}_*.
3. Find the hardware cause in the notes. If it is not in the notes: write UNKNOWN - to confirm in open_questions.md and call task.py blocked.
4. Fix the cause in ONE module. No game-specific or ROM-specific hack.
5. Run the whole suite and make sure the score is not below the baseline.
## Rules
- Never disable or skip a test. Never edit the manifest to hide a failure.
- Same command failing twice with no change in between: stop and think, or call task.py blocked.
## Exit tests
- the target test passes, suite score >= baseline, fmt/clippy/test green
## CLOSURE
python3 docs/tools/task.py check   (repeat until SCORE is full)
python3 docs/tools/task.py done
"""

FIX_CHECK = """#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "target test passes" 'cargo run -q --release -p puce8gb-cli -- suite --only {name_q}'
t "suite >= baseline" 'cargo run -q --release -p puce8gb-cli -- suite --baseline docs/annexes/baseline_score.txt'
t "fix unit test exists" 'ct puce8gb-core fix_{num}_'
score
"""


def human_only(what):
    """Human commands need a keyboard confirmation typed in a real interactive terminal."""
    if os.name == "nt":
        # No /dev/tty on native Windows. An agent subprocess has captured (non-tty) stdio,
        # so requiring a real console on both stdin and stdout keeps the agent out.
        if not (sys.stdin and sys.stdin.isatty() and sys.stdout and sys.stdout.isatty()):
            print("Refused: '%s' must be run by the user in a real terminal (no console)." % what)
            return False
        try:
            ans = input("[human check] %s - type YES to confirm: " % what).strip()
        except EOFError:
            ans = ""
    else:
        try:
            tty = open("/dev/tty", "r+")
        except OSError:
            print("Refused: '%s' must be run by the user in a real terminal (no /dev/tty)." % what)
            return False
        tty.write("[human check] %s - type YES to confirm: " % what)
        tty.flush()
        ans = tty.readline().strip()
        tty.close()
    if ans != "YES":
        print("Not confirmed.")
    return ans == "YES"


def load_index():
    rows = []
    with open(INDEX, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            i, role, gate, path, title = line.split("\t")
            rows.append(dict(id=i, role=role, gate=(gate == "1"), path=path, title=title))
    return rows


def save_index(rows):
    with open(INDEX, "w", encoding="utf-8") as f:
        f.write("# id\trole\tgate\tpath\ttitle\n")
        for r in rows:
            f.write("\t".join([r["id"], r["role"], "1" if r["gate"] else "0", r["path"], r["title"]]) + "\n")


def load_prog():
    with open(PROG, encoding="utf-8") as f:
        return json.load(f)


def save_prog(p):
    p["updated"] = time.strftime("%Y-%m-%d %H:%M:%S")
    with open(PROG, "w", encoding="utf-8") as f:
        json.dump(p, f, indent=2)
        f.write("\n")


def row_of(rows, i):
    for r in rows:
        if r["id"] == i:
            return r
    return None


def task_text(row):
    with open(os.path.join(ROOT, row["path"]), encoding="utf-8") as f:
        return f.read()


def field(text, name):
    m = re.search(r"^" + re.escape(name) + r":\s*(.*)$", text, re.M)
    return m.group(1).strip() if m else ""


def trim(text, n=60):
    lines = [ln[:200] for ln in text.splitlines()]
    return "\n".join(lines[-n:])


def append_open_question(msg):
    with open(OPENQ, "a", encoding="utf-8") as f:
        f.write("- " + msg + "\n")


def scope_violations(text):
    allowed = [a.strip().strip("`") for a in field(text, "Write only in").split(",") if a.strip()]
    r = subprocess.run(["git", "-c", "status.renames=false", "status", "--porcelain", "-uall"],
                       cwd=ROOT, capture_output=True, text=True)
    if r.returncode != 0:
        return None
    bad = []
    for line in r.stdout.splitlines():
        path = line[3:].strip().strip('"')
        if path in ALWAYS_OK:
            continue
        ok = False
        for a in allowed:
            base = a.rstrip("/")
            if path == base or path.startswith(base + "/"):
                ok = True
                break
        if not ok:
            bad.append(path)
    return bad


def find_bash():
    """Prefer Git Bash on Windows: the WSL launcher bash.exe on PATH has no cargo."""
    if os.name != "nt":
        return "bash"
    cands = [os.environ.get("GIT_BASH")]
    git = shutil.which("git")
    if git:
        # ...\Git\cmd\git.exe or ...\Git\bin\git.exe -> ...\Git\bin\bash.exe
        cands.append(os.path.join(os.path.dirname(os.path.dirname(git)), "bin", "bash.exe"))
    for base in (os.environ.get("ProgramFiles"), os.environ.get("ProgramFiles(x86)")):
        if base:
            cands.append(os.path.join(base, "Git", "bin", "bash.exe"))
    for p in cands:
        if p and os.path.exists(p):
            return p
    return "bash"


def run_script(i):
    script = os.path.join(CHECKS, i + ".sh")
    if not os.path.exists(script):
        return 0, 1, "[FAIL] missing check script " + script
    # Pass a POSIX-style relative path: Windows backslashes get stripped by bash.
    rel_script = "docs/checks/" + i + ".sh"
    r = subprocess.run([find_bash(), rel_script], cwd=ROOT, capture_output=True, text=True)
    out = r.stdout + r.stderr
    m = re.findall(r"SCORE: (\d+)/(\d+)", out)
    if not m:
        return 0, 1, out + "\n[FAIL] check script printed no SCORE line"
    p, n = map(int, m[-1])
    return p, n, out


def do_check(prog, rows, row, verbose=True):
    text = task_text(row)
    p, n, out = run_script(row["id"])
    extra = []
    if row["role"] in ("CODE", "TEST"):
        for desc, cmd in CLOSURE:
            n += 1
            r = subprocess.run(cmd, shell=True, cwd=ROOT, capture_output=True, text=True)
            if r.returncode == 0:
                p += 1
                extra.append("[ok]   " + desc)
            else:
                extra.append("[FAIL] " + desc)
                extra.append(trim(r.stdout + r.stderr, 15))
    viol = scope_violations(text)
    n += 1
    if viol is None:
        extra.append("[warn] not a git repo: scope not checked")
        p += 1
    elif viol:
        extra.append("[FAIL] files outside 'Write only in': " + ", ".join(viol[:10]))
    else:
        p += 1
        extra.append("[ok]   scope")
    if verbose:
        print(trim(out, 60))
        print("\n".join(extra))
        print("TOTAL SCORE: %d/%d" % (p, n))
    ok = (p == n)
    if not ok:
        prog["failures"] = prog.get("failures", 0) + 1
        if prog["failures"] >= MAX_FAILS:
            prog["tasks"][row["id"]] = "BLOCKED"
            append_open_question("%s blocked after %d failed checks - needs human review" % (row["id"], MAX_FAILS))
            print("\nSTOP: %d failed checks. Task BLOCKED. Tell the user and stop." % MAX_FAILS)
        else:
            print("\nNOT DONE (%d/%d failed checks). Fix the [FAIL] lines only." % (prog["failures"], MAX_FAILS))
    save_prog(prog)
    return ok


def regress(rows, exclude):
    prog = load_prog()
    bad = []
    for r in rows:
        if r["id"] == exclude or prog["tasks"].get(r["id"]) != "DONE":
            continue
        p, n, _ = run_script(r["id"])
        if p != n:
            bad.append("%s %d/%d" % (r["id"], p, n))
    return bad


def current(prog=None):
    prog = prog or load_prog()
    rows = load_index()
    cid = prog.get("current_task")
    return prog, rows, (row_of(rows, cid) if cid else None)


def guard(prog, row):
    if prog.get("awaiting_gate"):
        print("STATE: GATE - waiting for the user. STOP. Do nothing.")
        return False
    if row is None:
        print("STATE: FINISHED - all tasks are done.")
        return False
    if prog["tasks"].get(row["id"]) == "BLOCKED":
        print("STATE: BLOCKED - %s. STOP and tell the user." % row["id"])
        return False
    return True


def cmd_show():
    prog, rows, row = current()
    if not guard(prog, row):
        return 0
    text = task_text(row)
    pre = re.findall(r"\b(?:[A-Z]+[0-9]*_[0-9]+)\b", field(text, "Prerequisites"))
    missing = [x for x in pre if prog["tasks"].get(x) != "DONE"]
    if missing:
        print("STATE: BLOCKED - prerequisites not DONE: " + ", ".join(missing))
        return 0
    print("STATE: RUN | task %s | role %s | failed checks %d/%d" % (row["id"], row["role"], prog.get("failures", 0), MAX_FAILS))
    print("----- TASK FILE (%s) -----" % row["path"])
    print(text)
    return 0


def cmd_check():
    prog, rows, row = current()
    if not guard(prog, row):
        return 1
    return 0 if do_check(prog, rows, row) else 1


def git(*args):
    return subprocess.run(["git"] + list(args), cwd=ROOT, capture_output=True, text=True)


def cmd_done():
    prog, rows, row = current()
    if not guard(prog, row):
        return 1
    if not do_check(prog, rows, row):
        return 1
    bad = regress(rows, row["id"])
    if bad:
        print("NON-REGRESSION FAILED: " + ", ".join(bad))
        print("Fix the regression first (do not edit older check scripts).")
        prog = load_prog()
        prog["failures"] = prog.get("failures", 0) + 1
        save_prog(prog)
        return 1
    prog = load_prog()
    prog["tasks"][row["id"]] = "DONE"
    ids = [r["id"] for r in rows]
    nxt = None
    for i in ids[ids.index(row["id"]) + 1:]:
        if prog["tasks"].get(i) != "DONE":
            nxt = i
            break
    prog["current_task"] = nxt
    prog["failures"] = 0
    prog["sessions"] = 0
    prog["awaiting_gate"] = bool(row["gate"])
    prog["gate_task"] = row["id"] if row["gate"] else prog.get("gate_task")
    save_prog(prog)
    msg = "%s: %s" % (row["id"], row["title"])
    if git("rev-parse", "--is-inside-work-tree").returncode == 0:
        git("add", "-A")
        c = git("commit", "-m", msg)
        print("git commit:", "ok" if c.returncode == 0 else trim(c.stdout + c.stderr, 5))
        p = git("push")
        if p.returncode != 0:
            p = git("push", "-u", "origin", "HEAD")
        print("git push:", "ok" if p.returncode == 0 else "FAILED (not blocking): " + trim(p.stdout + p.stderr, 3))
    print("DONE: " + row["id"])
    if row["gate"]:
        print("\nThis was a GATE task. Print the GATE block (format in AGENTS.md), then STOP.")
        print("Do NOT run approve. Only the user can.")
    elif nxt:
        print("Next task: %s. This session is over: STOP (a fresh session will start the next task)." % nxt)
    return 0


def cmd_blocked(reason):
    prog, rows, row = current()
    if row is None:
        return 1
    prog["tasks"][row["id"]] = "BLOCKED"
    save_prog(prog)
    append_open_question("%s blocked: %s" % (row["id"], reason))
    print("Task %s marked BLOCKED. Report to the user in French and STOP." % row["id"])
    return 0


def cmd_approve():
    if not human_only("approve gate"):
        return 1
    prog, rows, row = current()
    if not prog.get("awaiting_gate"):
        print("No gate pending.")
        return 1
    gid = prog.get("gate_task")
    if gid and gid.startswith("A_") and os.path.exists(DECISIONS):
        with open(DECISIONS, encoding="utf-8") as f:
            lines = f.read().split("\n")
        inside = False
        for k, ln in enumerate(lines):
            if ln.startswith("## "):
                inside = (ln.strip() == "## " + gid)
            elif inside and ln.startswith("Statut : PROPOSE"):
                lines[k] = "Statut : APPROUVE"
        with open(DECISIONS, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
    prog["awaiting_gate"] = False
    save_prog(prog)
    print("Gate %s approved. Next task: %s" % (gid, prog.get("current_task")))
    return 0


def cmd_reopen(tid=None):
    if not human_only("reopen task"):
        return 1
    prog, rows, row = current()
    tid = tid or prog.get("gate_task")
    if not tid or row_of(rows, tid) is None:
        print("Unknown task id.")
        return 1
    ids = [r["id"] for r in rows]
    for i in ids[ids.index(tid):]:
        if prog["tasks"].get(i) == "DONE":
            prog["tasks"][i] = "TODO"
    prog.update(current_task=tid, awaiting_gate=False, failures=0, sessions=0)
    save_prog(prog)
    print("Reopened " + tid)
    return 0


def cmd_unblock():
    if not human_only("unblock task"):
        return 1
    prog, rows, row = current()
    if row is None:
        return 1
    prog["tasks"][row["id"]] = "TODO"
    prog["failures"] = 0
    prog["sessions"] = 0
    save_prog(prog)
    print("Unblocked " + row["id"])
    return 0


def cmd_begin():
    prog, rows, row = current()
    if prog.get("awaiting_gate"):
        print("STATE: GATE")
        return 0
    if row is None:
        print("STATE: FINISHED")
        return 0
    if prog["tasks"].get(row["id"]) == "BLOCKED":
        print("STATE: BLOCKED")
        return 0
    prog["sessions"] = prog.get("sessions", 0) + 1
    if prog["sessions"] > MAX_SESSIONS:
        prog["tasks"][row["id"]] = "BLOCKED"
        append_open_question("%s blocked: %d sessions without completion" % (row["id"], MAX_SESSIONS))
        save_prog(prog)
        print("STATE: BLOCKED")
        return 0
    save_prog(prog)
    print("STATE: RUN")
    return 0


def cmd_role():
    prog, rows, row = current()
    print(row["role"].lower() if row else "none")
    return 0


def cmd_status():
    prog, rows, row = current()
    done = sum(1 for r in rows if prog["tasks"].get(r["id"]) == "DONE")
    print("Current: %s | gate pending: %s | done %d/%d" % (prog.get("current_task"), prog.get("awaiting_gate"), done, len(rows)))
    for r in rows:
        st = prog["tasks"].get(r["id"], "TODO")
        mark = ">>" if r["id"] == prog.get("current_task") else "  "
        print("%s %-8s %-8s %-5s %s" % (mark, r["id"], st, r["role"], r["title"]))
    return 0


def cmd_addfix(name):
    if not human_only("addfix"):
        return 1
    prog, rows, row = current()
    nums = [int(r["id"].split("_")[1]) for r in rows if r["id"].startswith("FIX_")]
    num = "%02d" % (max(nums) + 1 if nums else 1)
    fid = "FIX_" + num
    os.makedirs(os.path.join(ROOT, "docs", "tasks", "FIX"), exist_ok=True)
    rel = "docs/tasks/FIX/%s.md" % fid
    with open(os.path.join(ROOT, rel), "w", encoding="utf-8") as f:
        f.write(FIX_TEMPLATE.format(id=fid, name=name, num=num))
    import shlex
    with open(os.path.join(CHECKS, fid + ".sh"), "w", encoding="utf-8") as f:
        f.write(FIX_CHECK.format(name_q=shlex.quote(name), num=num))
    new = dict(id=fid, role="TEST", gate=False, path=rel, title="Fix test " + name)
    pos = [r["id"] for r in rows].index(prog["current_task"]) if prog.get("current_task") else len(rows)
    rows.insert(pos, new)
    save_index(rows)
    prog["tasks"][fid] = "TODO"
    prog.update(current_task=fid, awaiting_gate=False, failures=0, sessions=0)
    save_prog(prog)
    print("Created %s and made it the current task." % fid)
    return 0


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    c = argv[1]
    a = argv[2:]
    table = {
        "show": lambda: cmd_show(), "check": lambda: cmd_check(), "done": lambda: cmd_done(),
        "blocked": lambda: cmd_blocked(" ".join(a) or "no reason given"),
        "approve": lambda: cmd_approve(), "reopen": lambda: cmd_reopen(a[0] if a else None),
        "unblock": lambda: cmd_unblock(), "begin": lambda: cmd_begin(), "role": lambda: cmd_role(),
        "status": lambda: cmd_status(), "addfix": lambda: cmd_addfix(" ".join(a)),
    }
    if c not in table:
        print(__doc__)
        return 2
    return table[c]()


if __name__ == "__main__":
    sys.exit(main(sys.argv))