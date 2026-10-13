#!/usr/bin/env python
"""Split the PPU epic E03 into small steps (E03_01..E03_06 -> 15 tasks + E03_16 CLI image dump).
Run from the repo root with a CLEAN git tree and NO running Hermes session:
    python _pack_cpu_reset/patch_split_e03.py            (dry run, prints the plan)
    python _pack_cpu_reset/patch_split_e03.py --apply --commit
Stdlib only. Backups: *.bak_e03split. Refuses to run if an E03 task is already started.
New order: 01 08 09 02 10 03 11 12 04 13 05 14 16 06 15 07"""
import glob, json, os, re, shutil, subprocess, sys, time

ROOT = os.getcwd()
IDX = os.path.join(ROOT, "docs", "tasks", "INDEX.tsv")
PROG = os.path.join(ROOT, "PROGRESS.json")
if not (os.path.exists(IDX) and os.path.exists(PROG)):
    sys.exit("ERROR: run from the repo root")

ORDER = ["E03_%02d" % n for n in (1, 8, 9, 2, 10, 3, 11, 12, 4, 13, 5, 14, 16, 6, 15, 7)]
OLD = ["E03_%02d" % n for n in range(1, 8)]
NEW = [t for t in ORDER if t not in OLD]

rows = open(IDX, encoding="utf-8").read().splitlines()
ids = [r.split("\t")[0] for r in rows]
for t in OLD:
    if t not in ids:
        sys.exit("ERROR: %s not in INDEX.tsv" % t)
for t in NEW:
    if t in ids:
        sys.exit("ERROR: %s already exists (patch already applied?)" % t)
pos = [ids.index(t) for t in OLD]
if pos != list(range(pos[0], pos[0] + len(OLD))):
    sys.exit("ERROR: E03_01..E03_07 are not contiguous in INDEX.tsv: %s" % pos)
by_id = {r.split("\t")[0]: r for r in rows}
prog = json.load(open(PROG, encoding="utf-8"))
st = prog["tasks"]
for t in OLD:
    if st.get(t, "TODO") != "TODO":
        sys.exit("ERROR: %s is %s: split only before E03 is started" % (t, st.get(t)))
p1 = os.path.join(ROOT, "docs", "tasks", "E03", "E03_01.md")
m = re.search(r"^Prerequisites:\s*(.*)$", open(p1, encoding="utf-8").read(), re.M)
if not m:
    sys.exit("ERROR: no Prerequisites line in E03_01.md")
FIRST_PREREQ = m.group(1).strip()

NOTES = "docs/annexes/notes/04a_video_regs.md (1-150); docs/annexes/notes/04b_video_render.md (1-150); docs/annexes/decisions.md (A_01)"
WRITE = "crates/puce8gb-core/src/video/, crates/puce8gb-core/src/bus.rs, crates/puce8gb-core/src/machine.rs"
HINT = "If a note is longer than 150 lines, use grep -n on the keywords of the task and read only the matching ranges. Do not read more than 3 files."

SPEC_COMMON = """- Dots: 456 dots per line, 154 lines, 70224 dots per frame. One M-cycle = 4 dots. Lines 0..143 are visible, lines 144..153 are VBlank (mode 1).
- Modes on a visible line: mode 2 = dots 0..79, mode 3 starts at dot 80 and lasts between 172 and 289 dots, mode 0 is the rest of the line.
- LCDC bits: 7 LCD enable, 6 window tile map (0 = 0x9800, 1 = 0x9C00), 5 window enable, 4 BG and window tile data (1 = 0x8000 unsigned, 0 = base 0x9000 with signed index), 3 BG tile map (0 = 0x9800, 1 = 0x9C00), 2 sprite size (0 = 8x8, 1 = 8x16), 1 sprite enable, 0 BG and window enable.
- A tile is 16 bytes, 2 bytes per row: first byte = low bit plane, second byte = high bit plane, bit 7 is the leftmost pixel. Color index = (high bit << 1) | low bit."""

TASKS = {}

def T(tid, title, goal, spec, work, extra_rules, read=NOTES, write=WRITE, check=None):
    TASKS[tid] = dict(title=title, goal=goal, spec=spec, work=work, extra=extra_rules,
                      read=read, write=write, check=check)

T("E03_01", "PPU line and frame timing, modes, LY, VBlank",
  "Replace the LY stub: dot-exact line and frame timing, LY, mode 0-3 sequence, VBlank interrupt. Nothing else.",
  SPEC_COMMON + """
- The LCD is considered ON in this task (LCDC bit 7 handling comes in E03_09). STAT, LYC and the other registers are NOT in this task.
- Mode 3 length is a constant for now: `const MODE3_LEN: u32 = 172;` with a comment saying it is replaced by the real end condition in E03_03, E03_11 and E03_13.""",
  ["Ppu struct with an integer dot counter (0..455), `ly` (0..153) and `mode` (0..3), stepped at the ratio of A_01. No float, no frame-at-once shortcut.",
   "Mode per line as in the spec: lines 0..143 give 2 then 3 (MODE3_LEN dots) then 0, lines 144..153 give 1.",
   "LY readable at 0xFF44 from the Ppu (replace the old stub). The `ly_override` of task C01_49 keeps priority over the real LY.",
   "VBlank: IF bit 0 is set exactly once per frame, when LY becomes 144 (exact dot per the note).",
   "Tests: a line lasts 456 dots, LY wraps from 153 to 0 after 70224 dots, mode sequence over one visible line, mode 1 on lines 144..153, VBlank requested once per frame."],
  ["Integer dot counter; no frame-at-once shortcuts."])

T("E03_08", "LYC, STAT register and STAT interrupt line",
  "Add LYC (0xFF45), STAT (0xFF41) and the STAT interrupt line with rising-edge detection.",
  """- STAT read: bit 7 = 1, bits 6..3 = interrupt select bits as written, bit 2 = coincidence flag (LY == LYC), bits 1..0 = current mode. STAT write: only bits 6..3 are writable.
- STAT interrupt line = (bit6 AND coincidence) OR (bit5 AND mode == 2) OR (bit4 AND mode == 1) OR (bit3 AND mode == 0).
- IF bit 1 is set only on a 0 -> 1 transition of that line (blocking: while the line stays high no new request).
- Special cases (line 144 mode 2 quirk, STAT write quirk, exact LYC compare dot) only if the note documents them; otherwise write a comment UNKNOWN - to confirm and do not implement them.""",
  ["LYC register read/write at 0xFF45. Coincidence flag updated as the note says.",
   "STAT register at 0xFF41 per the spec above (mode bits come from E03_01).",
   "STAT line with edge detection: keep the previous line value in a field, compare after each step.",
   "Tests: flag set and cleared when LY == LYC, STAT write masks bits 2..0 and 7, each of the four sources raises IF bit 1 once, two overlapping sources raise it once, a new request after the line fell."],
  ["Do not touch the VBlank request of E03_01."])

T("E03_09", "LCDC, LCD on and off, plain video registers",
  "Wire LCDC and the plain video registers, and implement LCD enable and disable.",
  SPEC_COMMON + """
- Plain registers (read/write, no side effect in this task): LCDC 0xFF40 (power-up 0x91), SCY 0xFF42, SCX 0xFF43, BGP 0xFF47 (power-up 0xFC), OBP0 0xFF48, OBP1 0xFF49, WY 0xFF4A, WX 0xFF4B. Other power-up values come from the note.
- LCDC bit 7 going 1 -> 0: dot = 0, LY = 0, mode = 0 (STAT mode bits read 0), no VBlank or STAT request while off, the STAT line is low.
- LCDC bit 7 going 0 -> 1: the PPU restarts at line 0, dot 0; the special first line behavior is the one of the note (if the note does not give it, keep the normal line and write UNKNOWN - to confirm).""",
  ["Add the registers above with their power-up values. Add `pub(crate) const` masks for the LCDC bits (LCDC_ENABLE, LCDC_WIN_MAP, LCDC_WIN_ENABLE, LCDC_TILE_DATA, LCDC_BG_MAP, LCDC_OBJ_SIZE, LCDC_OBJ_ENABLE, LCDC_BG_ENABLE); later tasks use them.",
   "LCD off and on as in the spec, using the dot counter of E03_01.",
   "Tests: power-up values, read/write of each register, LCD off resets LY and mode, no interrupt while off, LCD on restarts at line 0."],
  [])

T("E03_02", "OAM scan: sprite selection per line",
  "Mode 2 sprite selection. No pixels, no access blocking in this task.",
  """- OAM is 40 entries of 4 bytes at 0xFE00..0xFE9F: byte 0 = Y, byte 1 = X, byte 2 = tile, byte 3 = attributes.
- A sprite is on line LY when (LY + 16) >= Y and (LY + 16) < (Y + height), with height 8 or 16 from LCDC bit 2.
- At most 10 sprites per line, taken in OAM order. X is NOT used in the selection: a sprite with X = 0 or X >= 168 still counts in the limit.""",
  ["During mode 2 (80 dots, 2 dots per entry or all at the end, the result must be the same) fill a list `line_sprites` of at most 10 OAM indexes for the current line.",
   "Expose it with `pub(crate) fn line_sprites(&self) -> &[u8]` (OAM indexes in OAM order).",
   "The scan reads OAM directly (the CPU access blocking is task E03_10).",
   "Tests: 11 sprites on one line give the first 10 in OAM order, 8x8 and 8x16 boundaries (Y = 16 shows on lines 0..7 for 8x8 and 0..15 for 8x16, Y = 0 never shows), a sprite with X = 0 counts in the limit."],
  [])

T("E03_10", "VRAM and OAM access blocking",
  "CPU access restrictions on VRAM and OAM depending on the PPU mode.",
  """- LCD on: VRAM (0x8000..0x9FFF) cannot be accessed by the CPU in mode 3: reads return 0xFF, writes are ignored.
- LCD on: OAM (0xFE00..0xFE9F) cannot be accessed in modes 2 and 3: reads return 0xFF, writes are ignored.
- VBlank (mode 1) and mode 0: free access. LCD off: free access.
- The PPU's own reads are never blocked. The DMA interaction is task E03_15.
- Exact dot boundaries where the mode change becomes visible: only if the note gives them; otherwise use the mode bits and write UNKNOWN - to confirm.""",
  ["In the bus read and write paths for VRAM and OAM, ask the Ppu whether the access is allowed (a small `pub(crate) fn vram_accessible(&self) -> bool` and `oam_accessible`).",
   "Tests: each mode for VRAM and for OAM (read value 0xFF, write ignored, then allowed again), LCD off allows everything."],
  [])

T("E03_03", "Background pixel pipeline (no fine scroll, no window)",
  "Mode 3 with fetcher and FIFO for the background, with SCX multiple of 8 and no window. Output into a line buffer of color indexes.",
  SPEC_COMMON + """
- Background pixel Y = (LY + SCY) and 0xFF. Tile row index = Y / 8, line inside the tile = Y mod 8. BG tile map base from LCDC bit 3. Tile data from LCDC bit 4 (unsigned from 0x8000, or signed index from 0x9000).
- LCDC bit 0 = 0: the background color index is 0.
- In this task only SCX values that are multiples of 8 are tested (fine scroll is E03_11).""",
  ["Pixel fetcher steps and the FIFO as described in note 04b. One pixel leaves the FIFO per dot when allowed, into `line_buf: [u8; 160]` (color index 0..3, before any palette).",
   "Tile map and tile data addressing as in the spec; the fetcher reads VRAM directly (not through the CPU blocking).",
   "Replace the constant MODE3_LEN of E03_01 by the real end condition: mode 3 ends when 160 pixels were output, then mode 0 starts. With SCX mod 8 = 0, no window and no sprite, mode 3 must last 172 dots.",
   "Tests: pixels of a known tile (colors 0..3 on a row), 0x8000 and signed addressing, map select, SCY wrap, SCX coarse scroll, mode 3 lasts 172 dots."],
  ["Cycle-level pipeline, not a scanline renderer."])

T("E03_11", "Background fine scroll and variable mode 3 length",
  "SCX mod 8 fine scroll and the resulting mode 3 length.",
  """- At the start of each line the first (SCX mod 8) pixels of the first tile are discarded; mode 3 is longer by the amount given in the note for each value of SCX mod 8.
- The values come ONLY from the note; if the note gives no value for a case, write UNKNOWN - to confirm.""",
  ["Discard SCX mod 8 pixels at the start of the line, as the note says (when SCX is read, mid-line changes only if documented).",
   "Mode 3 length follows from the pipeline; mode 0 starts when the line is complete.",
   "Tests: pixels with SCX = 1..7, mode 3 duration for each SCX mod 8 value listed in the note."],
  ["Cycle-level pipeline, not a scanline renderer."])

T("E03_12", "Window",
  "Window rendering: start conditions, internal line counter, restart of the fetcher.",
  """- The window is displayed on a line when LCDC bit 5 = 1 and LCDC bit 0 = 1 (DMG), LY >= WY, and the pixel position reaches WX - 7. WX = 7 means the window starts at the left edge.
- Window tile map base from LCDC bit 6. The window has its own internal line counter: it increases only on lines where the window was actually displayed.
- When the window starts the fetcher restarts on the window map. Extra mode 3 dots and WX < 7 corner cases: only as documented in the note, otherwise UNKNOWN - to confirm.""",
  ["Window start condition, internal window line counter (reset at the start of each frame), fetcher restart.",
   "Tests: window starts at the right pixel for WX = 7, 20 and 166, WY condition, internal line counter does not advance on lines without window, window map select."],
  ["Cycle-level pipeline, not a scanline renderer."])

T("E03_04", "Sprites in the pipeline (without mode 3 penalty)",
  "Sprite pixels: priority, flip, palette select, BG priority flag. Mode 3 penalties are task E03_13.",
  """- Sprite attributes (byte 3): bit 7 BG priority (1 = BG colors 1..3 are drawn over the sprite), bit 6 Y flip, bit 5 X flip, bit 4 palette (0 = OBP0, 1 = OBP1).
- Sprite color index 0 is transparent. LCDC bit 1 = 0 disables all sprites.
- Overlap: the sprite with the lowest X wins; on equal X the lowest OAM index wins.
- 8x16: the tile index has bit 0 ignored; the top half is tile (index and 0xFE), the bottom half is tile (index or 0x01). Y flip also swaps the two halves.
- A sprite covers screen pixels X - 8 .. X - 1.""",
  ["Using `line_sprites` (E03_02), merge sprite pixels into the output stream: keep in each line buffer cell the color index, whether it comes from a sprite and which palette (a small struct, for example `PixelOut { color: u8, obj: bool, pal1: bool }`).",
   "Flip, 8x16, priority and BG priority flag as in the spec. Do not change the mode 3 length in this task.",
   "Tests: single sprite, two overlapping sprites (lower X wins, equal X lower OAM index wins), BG priority flag, X flip, Y flip, 8x16 with Y flip, color 0 transparent."],
  ["Cycle-level pipeline, not a scanline renderer."])

T("E03_13", "Sprite penalty on mode 3 length",
  "Mode 3 extension caused by sprite fetches.",
  """- The extension per sprite and its dependency on the sprite X position come ONLY from the note. A value not given by the note is UNKNOWN - to confirm and is not implemented.""",
  ["Add the sprite fetch penalty to the pipeline as documented (the fetcher pauses, mode 3 gets longer).",
   "Tests: mode 3 length with 0, 1 and 10 sprites, a sprite at X = 0 and one at X = 160 (cases listed in the note only)."],
  ["Penalty values only from the note; otherwise UNKNOWN."])

T("E03_05", "Palettes and framebuffer",
  "Apply BGP, OBP0, OBP1 and write finished lines into a 160x144 framebuffer.",
  """- Shade from a color index and a palette byte: shade = (palette >> (index * 2)) and 3.
- Sprite color index 0 is transparent whatever the palette says.
- Pixel format: u32 as 0xAARRGGBB with AA = 0xFF. Fixed table, neutral greys, documented in a code comment: shade 0 = 0xFFFFFFFF, shade 1 = 0xFFAAAAAA, shade 2 = 0xFF555555, shade 3 = 0xFF000000.
- LCD off: the framebuffer shows shade 0 everywhere (blank) as the note says.""",
  ["Palette application per pixel using the PixelOut of E03_04: BGP for background, OBP0 or OBP1 for sprites.",
   "Framebuffer of 160*144 u32 inside the Ppu; a finished visible line (end of mode 3) is written at row LY. `pub fn framebuffer(&self) -> &[u32]` and a forwarding method on Dmg.",
   "LCD off fills the framebuffer with the shade 0 color.",
   "Tests: BGP 0xE4 is the identity mapping, BGP 0x1B is reversed, sprite index 0 shows the background, a line lands at the right offset, LCD off gives a blank buffer. No floats."],
  ["No floats."])

T("E03_14", "Frame completion: run_frame and determinism",
  "run_frame on the machine and the proof that two runs give the same image.",
  """- A frame is 70224 dots (17556 M-cycles). `run_frame()` runs the machine until the next frame boundary defined in A_01 and returns; if A_01 says otherwise, A_01 wins.
- Keep a `frame_count: u64` incremented at each frame boundary.""",
  ["`pub fn run_frame(&mut self)` on Dmg, using the cycles per frame of A_01.",
   "Tests: one run_frame lasts exactly the cycles of A_01 (count the machine ticks), two machines started identically give identical framebuffers after 3 frames, frame_count increments once per run_frame."],
  ["No floats."])

T("E03_16", "CLI image dump: --frames and --dump-ppm",
  "First visual checkpoint: run N frames of a ROM and write the framebuffer to a PPM file.",
  """- Option `--frames N` (decimal): run N frames with run_frame, then stop with exit code 0. When it is given, --max-cycles and --expect-serial are ignored.
- Option `--dump-ppm FILE`: after the run, write the framebuffer as a binary PPM: the ASCII header `P6`, newline, `160 144`, newline, `255`, newline, then 160*144*3 bytes R,G,B (taken from the 0xAARRGGBB pixels).
- A missing or invalid value is a usage error: exit code 64, like the other options.""",
  ["Add both options to the argument parser and to the args struct. Run in `run_rom`, not in `run_machine`.",
   "Write the PPM writer as a small function that takes the framebuffer slice and a writer, so it can be unit tested.",
   "Tests (names start with e03_16_): PPM header and total size (15 bytes of header for the 160x144 case plus 69120 bytes), pixel conversion of one known pixel, option parsing (valid, missing value, not a number)."],
  ["Do not change any PPU file in this task."],
  read="crates/puce8gb-cli/src/run/mod.rs (grep -n \"RunArgs\\|fn parse\\|--trace\\|fn run_rom\"); crates/puce8gb-core/src/machine.rs (grep -n \"pub fn\"); docs/annexes/decisions.md (A_01)",
  write="crates/puce8gb-cli/",
  check=["ct puce8gb-cli e03_16_", "t 'option frames' 'grep -rq \"\\-\\-frames\" crates/puce8gb-cli/src'", "t 'option dump' 'grep -rq \"dump-ppm\" crates/puce8gb-cli/src'"])

T("E03_06", "OAM DMA: register and transfer",
  "FF46 starts a 160 byte copy into OAM, one byte per M-cycle. CPU restrictions are task E03_15.",
  """- Writing V to 0xFF46 starts a transfer from address (V << 8) + i to 0xFE00 + i for i = 0..159, one byte per M-cycle (160 M-cycles), after the start delay of A_02. Reading 0xFF46 returns the last written value.
- The DMA writes into OAM even when the PPU would block the CPU (DMA is not blocked by E03_10).
- Source ranges and the behavior for sources above the documented range: per the note and A_02, otherwise UNKNOWN - to confirm.
- A new write during a running transfer: only as documented in A_02.""",
  ["DMA state in the machine or bus: active flag, source, index, start delay. One step per M-cycle.",
   "Tests: result of a copy from WRAM (all 160 bytes), the transfer takes exactly the cycles of A_02, read-back of FF46."],
  ["DMA is not instantaneous."],
  read="docs/annexes/notes/03a_memory_map.md (1-150); docs/annexes/decisions.md (A_02)")

T("E03_15", "OAM DMA: bus restrictions for the CPU",
  "While a DMA transfer runs, the CPU can only use HRAM.",
  """- While the transfer is active (from the moment given in A_02 until its end), CPU reads outside HRAM (0xFF80..0xFFFE) return 0xFF and CPU writes outside HRAM are ignored. The status of IE (0xFFFF) and of the I/O registers: only as the note says, otherwise UNKNOWN - to confirm.
- Instruction fetch from outside HRAM during DMA therefore reads 0xFF.""",
  ["Apply the restriction in the bus read and write paths, using the DMA state of E03_06.",
   "Tests: a read of WRAM during DMA returns 0xFF, HRAM stays usable, the restriction ends exactly when the transfer ends, a program running from HRAM can poll until the end."],
  ["DMA is not instantaneous."],
  read="docs/annexes/notes/03a_memory_map.md (1-150); docs/annexes/decisions.md (A_02)")

def render(tid, prereq):
    d = TASKS[tid]
    low = tid.lower()
    out = ["# %s - %s" % (tid, d["title"]), "Role: CODE", "Goal: " + d["goal"],
           "Prerequisites: " + prereq,
           "Read (max 3 files, give line ranges): " + d["read"],
           "Write only in: " + d["write"],
           "## Hardware spec (fixed by the project)",
           "This is a project decision: do NOT search the notes for what is written here and do NOT discuss it. If a line elsewhere contradicts this section, this section wins. " + HINT]
    if d["spec"]:
        out.append(d["spec"].strip("\n"))
    out.append("## Work")
    for i, w in enumerate(d["work"], 1):
        out.append("%d. %s" % (i, w))
    out.append("## Rules")
    out.append("- Identifiers and comments in English, ASCII only. Zero invention: unknown => UNKNOWN - to confirm.")
    out.append("- Name every unit test fn starting with `%s_` (the check filters on it)." % low)
    for e in d["extra"]:
        out.append("- " + e)
    out.append("- A test is changed only if it contradicts the task, never to make it pass. Never disable a test.")
    out.append("- Same command failing twice with no change in between: STOP and run `python docs/tools/task.py blocked \"<reason>\"`.")
    out.append("- Never re-derive what the Hardware spec already states. Do not analyse aloud: act.")
    out.append("## Exit tests")
    out.append("- unit tests %s_* pass (at least one)" % low)
    out.append("- cargo fmt --check, clippy -D warnings, cargo test --workspace (added automatically)")
    out.append("- non-regression: all earlier exit tests (run by `done`)")
    out.append("## CLOSURE")
    out.append("python docs/tools/task.py check   (repeat until SCORE is full)")
    out.append("python docs/tools/task.py done    (commit + push + next task)")
    return "\n".join(out) + "\n"

prereq = {t: (FIRST_PREREQ if i == 0 else ORDER[i - 1]) for i, t in enumerate(ORDER)}
TITLE = {t: TASKS[t]["title"] for t in TASKS}
TITLE["E03_07"] = "dmg-acid2 and framebuffer hash"

print("Plan (id: prerequisite):")
for t in ORDER:
    print("  %s: %s  %s" % (t, prereq[t], "(new)" if t in NEW else ("(rewritten)" if t != "E03_07" else "(prereq + CLI text only)")))
if "--apply" not in sys.argv:
    print("DRY RUN only. Add --apply --commit to write.")
    sys.exit(0)

for p in (IDX, PROG):
    shutil.copy2(p, p + ".bak_e03split")
tdir = os.path.join(ROOT, "docs", "tasks", "E03")
cdir = os.path.join(ROOT, "docs", "checks")
for t in OLD[:-1]:
    shutil.copy2(os.path.join(tdir, t + ".md"), os.path.join(tdir, t + ".md.bak_e03split"))
for t in ORDER:
    if t == "E03_07":
        continue
    open(os.path.join(tdir, t + ".md"), "w", encoding="utf-8", newline="\n").write(render(t, prereq[t]))
    if t in NEW:
        lines = TASKS[t]["check"] or ["ct puce8gb-core %s_" % t.lower()]
        sh = '#!/usr/bin/env bash\nsource "$(dirname "$0")/lib.sh"\n' + "\n".join(lines) + "\nscore\n"
        open(os.path.join(cdir, t + ".sh"), "w", encoding="utf-8", newline="\n").write(sh)

# E03_07: prerequisite and CLI text
p7 = os.path.join(tdir, "E03_07.md")
shutil.copy2(p7, p7 + ".bak_e03split")
t7 = open(p7, encoding="utf-8").read()
t7 = re.sub(r"^Prerequisites:.*$", "Prerequisites: " + prereq["E03_07"], t7, count=1, flags=re.M)
t7, n = re.subn(r"^1\. CLI:.*$",
                "1. CLI: `--frames N` and `--dump-ppm FILE` already exist (task E03_16). Add `--hash`: with `run <rom> --frames N --hash` print a 64-bit FNV-1a hash of the framebuffer after N frames.",
                t7, count=1, flags=re.M)
open(p7, "w", encoding="utf-8", newline="\n").write(t7)
print("E03_07: prerequisite set; CLI line %s" % ("rewritten" if n else "NOT FOUND (edit by hand)"))

# python3 -> python in all task files (the project machine has no python3)
cnt = 0
for f in glob.glob(os.path.join(ROOT, "docs", "tasks", "**", "*.md"), recursive=True):
    s = open(f, encoding="utf-8").read()
    if "python3 docs/tools" in s:
        open(f, "w", encoding="utf-8", newline="\n").write(s.replace("python3 docs/tools", "python docs/tools"))
        cnt += 1
print("python3 -> python in %d task files" % cnt)

new_rows = []
for t in ORDER:
    if t in by_id:
        cols = by_id[t].split("\t")
        cols[4] = TITLE[t]
        new_rows.append("\t".join(cols))
    else:
        new_rows.append("\t".join([t, "CODE", "0", "docs/tasks/E03/%s.md" % t, TITLE[t]]))
rows2 = rows[:pos[0]] + new_rows + rows[pos[-1] + 1:]
open(IDX, "w", encoding="utf-8", newline="\n").write("\n".join(rows2) + "\n")
for t in NEW:
    st[t] = "TODO"
prog["updated"] = time.strftime("%Y-%m-%d %H:%M:%S")
json.dump(prog, open(PROG, "w", encoding="utf-8"), indent=2)
open(PROG, "a").write("\n")
print("OK: E03 split into %d tasks. New order: %s" % (len(ORDER), " ".join(ORDER)))
print("Check scripts of E03_01..E03_06 were NOT changed: run  Get-Content docs\\checks\\E03_0*.sh  and send me the output if a check mentions something moved to another sub-task.")
if "--commit" in sys.argv:
    subprocess.run(["git", "add", "-A"], cwd=ROOT)
    r = subprocess.run(["git", "commit", "-m", "split E03 PPU epic into 16 small tasks"], cwd=ROOT)
    print("git commit:", "ok" if r.returncode == 0 else "failed")
