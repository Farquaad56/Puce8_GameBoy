#!/usr/bin/env python
"""Add a fixed hardware spec section to docs/tasks/C01/C01_32.md (EI/DI/IME, dispatch, RETI).
Run from the repo root with a CLEAN git tree and NO running Hermes session:
    python _pack_cpu_reset/patch_spec_c01_32.py --commit
Stdlib only. Backup: C01_32.md.bak_spec"""
import os, shutil, subprocess, sys

ROOT = os.getcwd()
PATH = os.path.join(ROOT, "docs", "tasks", "C01", "C01_32.md")
if not os.path.exists(PATH):
    sys.exit("ERROR: run from the repo root (docs/tasks/C01/C01_32.md not found)")
txt = open(PATH, encoding="utf-8").read()
MARK = "## Hardware spec (fixed by the project)"
if MARK in txt:
    sys.exit("ERROR: spec already present")

SPEC = MARK + """
Source: Pan Docs, "Interrupts" and "CPU Instruction Set". This is a project decision: do NOT search the notes for it, do NOT discuss it. If a line above contradicts this section, this section wins.
- Registers: IF at 0xFF0F (bits 0..4 writable, bits 5..7 read as 1), IE at 0xFFFF (8 bits, read/write). If the bus does not implement them yet, make them plain registers. IME is a CPU flag, 0 at power-on.
- Sources, lowest bit has priority: bit0 VBlank vector 0x40, bit1 STAT 0x48, bit2 Timer 0x50, bit3 Serial 0x58, bit4 Joypad 0x60.
- DI (0xF3): IME = 0 immediately, and it cancels a pending EI.
- EI (0xFB): IME becomes 1 only AFTER the instruction that follows EI has completed (one-instruction delay). Implementation: EI sets a delay counter, done() decrements it, IME = 1 when it reaches 0. DI (0xF3) already exists: keep it working.
- RETI (0xD9): same as RET (4 M-cycles) and IME = 1 IMMEDIATELY (no delay).
- Dispatch is checked only at an instruction boundary (step == 0, before the opcode fetch). Condition: IME && (IE & IF & 0x1F) != 0. Then, instead of a fetch, it takes 5 M-cycles: M1 internal, M2 internal, M3 write PC high byte at [SP-1], M4 write PC low byte at [SP-2], M5 internal. At the end: SP = SP-2, PC = vector of the lowest pending bit, IME = 0, that IF bit is cleared. The pushed PC is the address of the instruction that was NOT executed.
- Out of scope for this task: HALT, STOP, the timer, ROM 02 (do not run it, it needs later tasks). Do not implement the corner case where the push overwrites IE.
## Test vectors (use exactly these, names start with c01_32_)
- ei_delay: program at 0x0100 = FB 00 00 (EI, NOP, NOP), SP=0xDFFE, IE=0x01, IF=0x01. Tick EI (1 M-cycle): IME must still be 0. Tick the first NOP: IME is now 1. Then 5 M-cycles of dispatch: PC=0x0040, SP=0xDFFC, [0xDFFD]=0x01, [0xDFFC]=0x02, IF bit0 cleared, IME=0. Total 1+1+5 = 7 M-cycles from the start.
- di_blocks: program FB F3 00 (EI, DI, NOP), IE=0x01, IF=0x01: after 3 instructions PC=0x0103 and no dispatch happened.
- priority: IME=1, IE=0x1F, IF=0x05 (VBlank + Timer): vector 0x40 is taken, IF becomes 0x04.
- reti_immediate: SP=0xDFFC, [0xDFFC]=0x34, [0xDFFD]=0x12, program D9 at 0x0100, IME=0, IE=0x01, IF=0x01: RETI takes 4 M-cycles (PC=0x1234), IME=1, and the dispatch starts right after (5 M-cycles) with 0x1234 pushed.
- no_dispatch_when_ime_clear: IME=0, IE=0x01, IF=0x01, NOP: the NOP executes normally, PC=0x0101.
- if_register: write 0x01 to 0xFF0F then read it: 0xE1. Write 0xFF to 0xFFFF then read it: 0xFF.
"""
for marker in ("\n## Rules", "\n## Exit tests"):
    if marker in txt:
        new = txt.replace(marker, "\n" + SPEC + marker, 1)
        break
else:
    new = txt.rstrip("\n") + "\n" + SPEC
shutil.copy2(PATH, PATH + ".bak_spec")
open(PATH, "w", encoding="utf-8", newline="\n").write(new)
print("OK: spec added to C01_32.md")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "C01_32: add fixed interrupt hardware spec"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
