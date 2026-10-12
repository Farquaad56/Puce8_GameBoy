#!/usr/bin/env python
"""Add a fixed hardware spec section to docs/tasks/C01/C01_33.md (HALT, halt bug, STOP, illegal opcodes).
Run from the repo root with a CLEAN git tree and NO running Hermes session:
    python _pack_cpu_reset/patch_spec_c01_33.py --commit
Stdlib only. Backup: C01_33.md.bak_spec"""
import os, shutil, subprocess, sys

ROOT = os.getcwd()
PATH = os.path.join(ROOT, "docs", "tasks", "C01", "C01_33.md")
if not os.path.exists(PATH):
    sys.exit("ERROR: run from the repo root (docs/tasks/C01/C01_33.md not found)")
txt = open(PATH, encoding="utf-8").read()
MARK = "## Hardware spec (fixed by the project)"
if MARK in txt:
    sys.exit("ERROR: spec already present")

SPEC = MARK + """
Source: Pan Docs, "CPU Instruction Set" (HALT, STOP) and "Interrupts". This is a project decision: do NOT search the notes for it, do NOT discuss it. If a line above contradicts this section, this section wins.
- New CPU state: `halted: bool` (false at power-on) and `locked: bool` (false at power-on). Pending = (IE & IF & 0x1F) != 0 (use the same helper as the dispatch).
- HALT (0x76), fetched at step 0 like any opcode (1 M-cycle), then three cases, decided at the end of that fetch:
  1. IME = 1 and not pending, or IME = 0 and not pending: set halted = true.
  2. IME = 1 and pending: do NOT halt. The instruction ends; the dispatch happens at the next boundary (PC already points to the byte after HALT, so that address is pushed).
  3. IME = 0 and pending: HALT BUG. Do NOT halt, and the byte after HALT is fetched TWICE: set a flag `halt_bug = true`; at the next opcode fetch, read the opcode at PC but do NOT increment PC (clear the flag after that one fetch).
- While halted: at each step 0, if not pending, the tick is 1 idle M-cycle (no bus access, no PC change). If pending: halted = false and the tick continues as a normal instruction boundary in the SAME tick (dispatch if IME = 1, otherwise fetch). With IME = 0 the interrupt is NOT dispatched and its IF bit is NOT cleared.
- STOP (0x10): treated as a 2-byte NOP: 2 M-cycles (opcode fetch, then read and ignore the next byte), PC += 2. No low-power mode is emulated.
- Illegal opcodes 0xD3 0xDB 0xDD 0xE3 0xE4 0xEB 0xEC 0xED 0xF4 0xFC 0xFD: the CPU locks up. The fetch takes 1 M-cycle, sets locked = true, PC stays after the opcode. Every later tick is 1 idle M-cycle and nothing changes, not even the interrupt dispatch. Do NOT use the `unimplemented` record for them: they are implemented, they lock.
- Out of scope: EI directly followed by HALT, the HALT bug combined with the EI delay, STOP wake-up by joypad, the timer.
## Test vectors (use exactly these, names start with c01_33_)
- halt_waits: code HALT NOP at the code address, IME=0, IE=0, IF=0. Tick 1 (fetch HALT), then 10 more ticks: PC unchanged (= address+1), halted. Then IE=0x01, IF=0x01: the next tick wakes up and fetches the NOP in that same tick; PC = address+2. IF is still 0x01 (not cleared), IME still 0.
- halt_ime_dispatch: IME=1, IE=0x01, IF=0 at HALT time. HALT then idle ticks. Set IF=0x01: wake up and dispatch (5 M-cycles): PC=0x0040, pushed PC = address of the byte after HALT, IF bit 0 cleared, IME=0.
- halt_bug: code 76 3C 00 at the code address (HALT, INC A, NOP), IME=0, IE=0x01, IF=0x01, A=0x00. After HALT, INC A executes TWICE (A = 0x02) because its byte is read twice, and PC then points to the NOP. HALT must not set halted.
- halt_no_bug_with_ime: IME=1, IE=0x01, IF=0x01 at HALT: no halt bug, dispatch right after HALT, pushed PC = address+1.
- stop_skips_byte: code 10 00 3C, STOP takes 2 M-cycles, PC = address+2, then INC A makes A = 0x01.
- illegal_locks: for each of the 11 illegal opcodes: after the fetch locked = true; after 100 more ticks PC is unchanged, and with IME=1, IE=0x01, IF=0x01 no dispatch happens (SP unchanged).
"""
for marker in ("\n## Rules", "\n## Exit tests"):
    if marker in txt:
        new = txt.replace(marker, "\n" + SPEC + marker, 1)
        break
else:
    new = txt.rstrip("\n") + "\n" + SPEC
shutil.copy2(PATH, PATH + ".bak_spec")
open(PATH, "w", encoding="utf-8", newline="\n").write(new)
print("OK: spec added to C01_33.md")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "C01_33: add fixed HALT/STOP hardware spec"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
