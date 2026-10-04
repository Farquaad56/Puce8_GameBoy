---
name: gb-test
description: GB emulator: make test ROMs pass
version: 1.1.0
---
# gb-test

## When to Use
Preloaded by docs/tools/drive.sh when the current task has Role: TEST.

## Procedure
Role TEST. You make real test ROMs pass.
- Run one ROM at a time with the CLI (see task). Read the serial output / result code first.
- Isolate the smallest reproduction (one instruction, one register, one cycle) and write a failing unit test BEFORE fixing.
- Find the cause in the notes. Not in the notes => UNKNOWN in open_questions.md and task.py blocked.
- Never disable a test, never special-case a ROM. One module changed per fix when possible.

## Coder mode (given in the user message: "Coder: opencode" = default, or "Coder: self")
### Coder: self
Do the work yourself, following the role rules above.
### Coder: opencode
You orchestrate; OpenCode writes the code. Never write Rust yourself in this mode.
1. Run: python3 docs/tools/task.py show   (note the task id and the task file path in the header).
2. Load the skill `opencode`, then delegate the WHOLE task, scoped to the repo root (one OpenCode session at a time):
   opencode run 'Do exactly the sub-task in <TASK_FILE_PATH>. Role: TEST. Follow AGENTS.md, including check then done at the end.'
   (add --model <M> when the user message names an OpenCode model; use the skill's background mode if the terminal times out).
3. When it returns, run: python3 docs/tools/task.py show
   - First line STATE: GATE -> print the GATE block of AGENTS.md in French and STOP.
   - The current task id changed -> success: one French line summary, STOP.
   - Same task, STATE: RUN -> run task.py check, then call opencode run ONCE more with only the [FAIL] lines
     (max 2 retries). Still failing -> python3 docs/tools/task.py blocked "<reason>" and STOP.
4. Never run task.py approve / reopen / unblock / addfix.

Always follow AGENTS.md. Start with: python3 docs/tools/task.py show
