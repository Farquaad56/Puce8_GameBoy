# AGENTS.md - Game Boy (DMG) emulator, Rust, cycle accurate

You are one agent in a strict sequential pipeline. ONE sub-task per session.
Speak to the user in French. Code, identifiers, comments, commit messages: English, ASCII only.
Hardware: original Game Boy (DMG). CPU = Sharp SM83 (NOT a Zilog Z80). Docs = Pan Docs in refs/pandocs.

## Session start (always, in this order)
1. Run: python3 docs/tools/task.py show
2. If the first line is not "STATE: RUN", say so in French and STOP.
3. The task file printed is your ONLY job. Do not start anything else. Do not read other tasks.

## Session end
1. Run: python3 docs/tools/task.py check   (it prints SCORE p/n and [FAIL] lines)
2. Repeat fix + check until all lines are [ok]. Then run: python3 docs/tools/task.py done
3. `done` runs the non-regression tests, commits and pushes. Never run git commit/push yourself.
4. After `done`: if the task was a GATE, print the GATE block below and STOP. Otherwise STOP.
5. NEVER run: task.py approve, reopen, unblock, addfix. Only the user does.

## GATE block (print exactly, in French, then stop)
=== GATE <task id> ===
Done: <2 to 4 lines>
Written: <files>
Checks: <SCORE line>
Open questions: <numbered, or none>
Reply: "ok" pour continuer | ou indiquer quoi changer.
=== STOP ===
Silence is not approval. Only the user's explicit ok/go/validate lets the pipeline continue.

## Anti-loop rules (important)
- The same command failing twice without any change in between: STOP repeating it. Change approach or call:
  python3 docs/tools/task.py blocked "<reason>"
- Never retry a failing check more than 3 times with the same idea.
- If something is missing from the sources: do not guess. Write "Statut : UNKNOWN - to confirm" and add a line to
  docs/annexes/open_questions.md, then continue with what does not depend on it.

## Context budget
- Never open a file of more than 150 lines whole: use grep -n, head, sed -n 'a,bp'.
- Truncate command output (| head -40). Read at most 3 note files per task, with line ranges.
- Files marked "Copy without reading" are copied with cp, never opened.

## Zero invention
- Hardware facts come from refs/pandocs, the test ROM docs, and docs/annexes/notes. Never from memory of other emulators.
- Every numeric value (cycles, addresses, sizes) needs a source line. Conflicts: document both values, decide with a test ROM.
- Notes are paraphrases with a reference (file#section). Quote at most a few words. Never paste long source text.

## Fact format (notes, in French)
## <Sujet precis>
Fait : <enonce court, chiffre>
Source : <fichier#section ou url + date>
Fiabilite : officielle | communautaire | testee sur ROM | deduite
Impact code : <module et fonction>
Statut : CONFIRME | CONFLIT | UNKNOWN - to confirm

## Write scope
- Write only in the paths listed in "Write only in:" of the task. `check` fails on any other changed file.
- Never edit refs/, never add a ROM or BIOS to git (roms/ is ignored), never write secrets anywhere.

## Rust rules (core crate puce8gb-core)
- #![forbid(unsafe_code)], zero external dependencies, no I/O, no threads, no std::fs, no system clock, deterministic.
- Hardware arithmetic: wrapping_add / wrapping_sub / overflowing_* explicitly.
- No panic on external input (bad ROM => Result<_, LoadError>). No Rc<RefCell>/Arc<Mutex> for the bus.
- No allocation inside tick() (no Vec::push, String, Box in the hot path).
- Cycle accurate: one micro-op = exactly one bus access. Integer clock ratios only (no f32/f64). Chip order inside
  a tick is defined in docs/annexes/decisions.md and never changed silently.
- Debug/viewer code reads memory with peek() (no side effects).
- Frontend crates (cli, desktop) may use deps; the core must never know egui, cpal, gilrs or the file system.

## Style of work
- Small diffs. Unit test first for each instruction group / register. Test fn names start with the task id in lower case
  (e.g. e02_03_ld_r_r) - the check script filters on that prefix.
- A failing test is never disabled. Fix the hardware cause, never add a game-specific hack.
