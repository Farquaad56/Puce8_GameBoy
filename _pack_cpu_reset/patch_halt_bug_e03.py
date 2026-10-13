#!/usr/bin/env python
"""halt_bug.gb is screen-only. Fix the plan:
  - C01_41 becomes the CPU closure gate (cpu_instrs 01..11 + HALT unit tests), no halt_bug ROM.
  - New task E03_17 (after E03_16, before E03_06): halt_bug.gb image compared to the reference PNG.
  - Adds docs/tools/compare_png_ppm.py. Unblocks C01_41.
Run from the repo root, CLEAN git tree, NO running Hermes session, AFTER patch_split_e03.py:
    python _pack_cpu_reset/patch_halt_bug_e03.py --commit
Stdlib only. Backups: *.bak_haltbug"""
import json, os, re, shutil, subprocess, sys, time

ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
if not (os.path.exists(IDX) and os.path.exists(PROG)):
    sys.exit("ERROR: run from the repo root")
rows = open(IDX, encoding="utf-8").read().splitlines()
ids = [r.split("\t")[0] for r in rows]
for t in ("C01_41", "E03_16", "E03_06"):
    if t not in ids:
        sys.exit("ERROR: %s not in INDEX.tsv (apply patch_split_e03.py first?)" % t)
if "E03_17" in ids:
    sys.exit("ERROR: patch already applied")
src_cmp = os.path.join(os.path.dirname(os.path.abspath(__file__)), "compare_png_ppm.py")
if not os.path.exists(src_cmp):
    sys.exit("ERROR: compare_png_ppm.py must be in the same folder as this script")

for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_haltbug")
tdir = os.path.join(ROOT, "docs", "tasks")
for rel in ("C01/C01_41.md", "E03/E03_06.md"):
    shutil.copy2(os.path.join(tdir, rel), os.path.join(tdir, rel) + ".bak_haltbug")
cm = re.search(r"^Prerequisites:\s*(.*)$", open(os.path.join(tdir, "C01/C01_41.md"), encoding="utf-8").read(), re.M)
prev41 = cm.group(1).strip() if cm else "C01_40"

C41 = """# C01_41 - CPU restart closure gate
Role: TEST
Goal: Close the CPU restart: all eleven cpu_instrs individual ROMs pass together with the HALT unit tests. (The ROM halt_bug is screen-only: it is validated by task E03_17, after the PPU exists.)
Prerequisites: %s
Read (max 3 files, give line ranges): docs/annexes/notes/09_test_roms.md (grep first)
Write only in: crates/puce8gb-core/src/, crates/puce8gb-cli/
## Work
1. Run the check script. It runs the eleven cpu_instrs individual ROMs (01..11) with `--expect-serial Passed` and the unit tests c01_33_*.
2. If one ROM fails, read the exit code and the printed text FIRST, then fix the CPU with a unit test named c01_41_* first. Never edit a ROM, never loosen --expect-serial, never add a ROM-specific hack.
3. Nothing else to implement. Do NOT run halt_bug.gb here: it prints on the screen only.
## Rules
- Identifiers and comments in English, ASCII only. Zero invention: unknown => UNKNOWN - to confirm (open_questions.md).
- Name every unit test fn starting with `c01_41_` (the check filters on it); if no new test is needed, the check does not require one.
- A test is changed only if it contradicts the notes, never to make it pass. Never disable a test.
- Same command failing twice with no change in between: STOP and run `python docs/tools/task.py blocked "<reason>"`.
## Exit tests
- the eleven cpu_instrs individual ROMs pass
- unit tests c01_33_* pass
- cargo fmt --check, clippy -D warnings, cargo test --workspace (added automatically)
- non-regression: all earlier exit tests (run by `done`)
## CLOSURE
python docs/tools/task.py check   (repeat until SCORE is full)
python docs/tools/task.py done    (commit + push + next task)

GATE after done: print the GATE block of AGENTS.md, then STOP and wait for the user.
""" % prev41

C41SH = '''#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
for n in 01 02 03 04 05 06 07 08 09 10 11; do
  t "cpu_instrs individual $n passes" "r=\\$(rom cpu_instrs/individual $n-); test -n \\"\\$r\\" && cargo run -q --release -p puce8gb-cli -- run \\"\\$r\\" --max-cycles 200000000 --expect-serial Passed"
done
t 'HALT unit tests' 'cargo test -q -p puce8gb-core c01_33_ 2>&1 | grep -Eq "test result: ok\\. [1-9][0-9]* passed"'
score
'''

E17 = """# E03_17 - blargg halt_bug on screen (image comparison)
Role: TEST
Goal: The ROM halt_bug.gb draws its result on the screen; our image has the same shades as the reference PNG of the pack.
Prerequisites: E03_16
Read (max 3 files, give line ranges): docs/annexes/notes/09_test_roms.md (grep first); docs/annexes/notes/04b_video_render.md (grep first)
Write only in: crates/puce8gb-core/src/, crates/puce8gb-cli/
## Hardware spec (fixed by the project)
This is a project decision: do NOT search the notes for it and do NOT discuss it.
- halt_bug.gb writes nothing on the serial port. Its result is the final screen. The reference image is the PNG found by the check script (roms/test-roms/blargg/halt_bug-dmg-cgb.png).
- The ROM ends in an infinite loop, so any frame after the test is finished shows the same final image. Use `--frames 300`.
- The comparison tool is `docs/tools/compare_png_ppm.py <ours.ppm> <reference.png> [--ascii]`. It ranks the colors by luminance, so our grey table does not need to match the PNG colors. Exit 0 = identical, 1 = different (prints the first differing pixel and the box), `--ascii` prints both images inside the box.
## Work
1. Run: `cargo run -q --release -p puce8gb-cli -- run "<rom>" --frames 300 --dump-ppm docs/annexes/halt_bug.ppm` (rom path: see the check script).
2. Run: `python docs/tools/compare_png_ppm.py docs/annexes/halt_bug.ppm <reference.png> --ascii` and read ONLY its output.
3. If the image is entirely one shade: the ROM did not draw. Check in order: LCDC value written by the ROM, the PPU reached mode 3 and wrote lines, VRAM writes were not ignored while the ROM expected them to be accepted (LCD off or mode), BGP value. Find the cause in the notes, fix in ONE module, add a unit test named e03_17_*.
4. If the text is drawn but wrong or missing letters: compare the ascii boxes; the cause is a PPU or HALT bug. Fix with a unit test first.
5. Never edit the ROM, never edit the reference, never special-case this ROM.
## Rules
- Identifiers and comments in English, ASCII only. Zero invention: unknown => UNKNOWN - to confirm.
- A test is changed only if it contradicts the notes, never to make it pass. Never disable a test.
- Same command failing twice with no change in between: STOP and run `python docs/tools/task.py blocked "<reason>"`.
- GATE: after done, print the GATE block of AGENTS.md and ask the user to open halt_bug.ppm and compare it with the reference PNG by eye. Then STOP.
## Exit tests
- halt_bug image identical to the reference
- cargo fmt --check, clippy -D warnings, cargo test --workspace (added automatically)
- non-regression: all earlier exit tests (run by `done`)
## CLOSURE
python docs/tools/task.py check   (repeat until SCORE is full)
python docs/tools/task.py done    (commit + push + next task)

GATE after done: print the GATE block of AGENTS.md, then STOP and wait for the user.
"""

E17SH = '''#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'halt_bug image identical to the reference' 'r=$(rom blargg halt_bug); png=$(find roms/test-roms -path "*/blargg/*" -name "halt_bug*.png" | sort | head -1); test -n "$r" && test -n "$png" && cargo run -q --release -p puce8gb-cli -- run "$r" --frames 300 --dump-ppm docs/annexes/halt_bug.ppm && python docs/tools/compare_png_ppm.py docs/annexes/halt_bug.ppm "$png"'
score
'''

def w(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w", encoding="utf-8", newline="\n").write(text)

w(os.path.join(tdir, "C01", "C01_41.md"), C41)
w(os.path.join(ROOT, "docs", "checks", "C01_41.sh"), C41SH)
w(os.path.join(tdir, "E03", "E03_17.md"), E17)
w(os.path.join(ROOT, "docs", "checks", "E03_17.sh"), E17SH)
shutil.copy2(src_cmp, os.path.join(ROOT, "docs", "tools", "compare_png_ppm.py"))

p6 = os.path.join(tdir, "E03", "E03_06.md")
t6 = open(p6, encoding="utf-8").read()
t6 = re.sub(r"^Prerequisites:.*$", "Prerequisites: E03_17", t6, count=1, flags=re.M)
open(p6, "w", encoding="utf-8", newline="\n").write(t6)

out = []
for r in rows:
    c = r.split("\t")
    if c[0] == "C01_41":
        c[1], c[4] = "TEST", "CPU restart closure gate (cpu_instrs 01-11)"
        c[2] = "1"
        r = "\t".join(c)
    out.append(r)
    if c[0] == "E03_16":
        out.append("\t".join(["E03_17", "TEST", "1", "docs/tasks/E03/E03_17.md", "blargg halt_bug on screen (image comparison)"]))
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")

prog = json.load(open(PROG, encoding="utf-8"))
st = prog["tasks"]
st["C01_41"] = "TODO"
st["E03_17"] = "TODO"
prog.update(current_task="C01_41", awaiting_gate=False, failures=0, sessions=0,
            updated=time.strftime("%Y-%m-%d %H:%M:%S"))
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")
print("OK: C01_41 is now the CPU closure gate (TODO), E03_17 added after E03_16, E03_06 now depends on E03_17.")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "halt_bug is screen-only: C01_41 closure gate, add E03_17 image check"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
