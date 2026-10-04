#!/usr/bin/env python3
"""Read-only live dashboard server (stdlib only). Usage: python3 docs/tools/dashboard.py [port]
Serves docs/dashboard/index.html and /api/state (JSON built from PROGRESS.json, INDEX.tsv, git, annexes)."""
import json
import os
import re
import statistics
import subprocess
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PAGE = os.path.join(ROOT, "docs", "dashboard", "index.html")


def read(rel):
    try:
        with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
            return f.read()
    except OSError:
        return ""


def git_log():
    try:
        r = subprocess.run(["git", "log", "-n", "400", "--pretty=%h|%ct|%s"], cwd=ROOT,
                           capture_output=True, text=True, timeout=5)
    except Exception:
        return []
    out = []
    for line in r.stdout.splitlines():
        p = line.split("|", 2)
        if len(p) == 3 and p[1].isdigit():
            out.append(dict(hash=p[0], ts=int(p[1]), msg=p[2]))
    return out


def eta(commits):
    ts = sorted(c["ts"] for c in commits if re.match(r"^(?:[A-Z]+[0-9]*_[0-9]+):", c["msg"]))
    gaps = [b - a for a, b in zip(ts, ts[1:]) if 0 < b - a < 4 * 3600]
    return int(statistics.median(gaps)) if len(gaps) >= 3 else None


def decisions():
    res = {}
    cur = None
    for ln in read("docs/annexes/decisions.md").splitlines():
        m = re.match(r"^## (A_\d+)\s*$", ln)
        if m:
            cur = m.group(1)
            res[cur] = "PROPOSE"
        elif cur and ln.startswith("Statut : APPROUVE"):
            res[cur] = "APPROUVE"
    return res


def state():
    try:
        prog = json.loads(read("PROGRESS.json") or "{}")
    except ValueError:
        prog = {}
    tasks = []
    for ln in read("docs/tasks/INDEX.tsv").splitlines():
        if not ln or ln.startswith("#"):
            continue
        p = ln.split("\t")
        if len(p) == 5:
            tasks.append(dict(id=p[0], role=p[1], gate=(p[2] == "1"), title=p[4],
                              status=prog.get("tasks", {}).get(p[0], "TODO"), phase=p[0].split("_")[0]))
    notes_dir = os.path.join(ROOT, "docs", "annexes", "notes")
    notes = [n for n in (os.listdir(notes_dir) if os.path.isdir(notes_dir) else []) if n.endswith(".md")]
    unknown = 0
    for n in notes:
        unknown += read("docs/annexes/notes/" + n).count("UNKNOWN - to confirm")
    oq = [ln[2:] for ln in read("docs/annexes/open_questions.md").splitlines() if ln.startswith("- ")]
    commits = git_log()
    base = read("docs/annexes/baseline_score.txt").strip()
    cur = prog.get("current_task")
    sysstate = "FINISHED"
    if prog.get("awaiting_gate"):
        sysstate = "GATE"
    elif cur:
        sysstate = "BLOCKED" if prog.get("tasks", {}).get(cur) == "BLOCKED" else "RUN"
    return dict(state=sysstate, current=cur, gate_task=prog.get("gate_task"), failures=prog.get("failures", 0),
                sessions=prog.get("sessions", 0), updated=prog.get("updated", ""), tasks=tasks,
                notes=dict(count=len(notes), unknown=unknown), open_questions=oq[-12:], open_total=len(oq),
                commits=commits[:12], eta_per_task=eta(commits), decisions=decisions(),
                baseline=int(base) if base.isdigit() else None)


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def send(self, code, body, ctype):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = self.path.split("?")[0]
        if path == "/api/state":
            self.send(200, json.dumps(state()).encode(), "application/json")
        elif path in ("/", "/index.html"):
            try:
                with open(PAGE, "rb") as f:
                    self.send(200, f.read(), "text/html; charset=utf-8")
            except OSError:
                self.send(404, b"index.html missing", "text/plain")
        else:
            self.send(404, b"not found", "text/plain")


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
    srv = ThreadingHTTPServer(("127.0.0.1", port), H)
    print("Dashboard: http://127.0.0.1:%d  (Ctrl+C to stop)" % port)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
