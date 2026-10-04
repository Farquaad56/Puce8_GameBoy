---
name: gb-arch
description: GB emulator: propose one architecture decision
version: 1.1.0
---
# gb-arch

## When to Use
Preloaded by docs/tools/drive.sh when the current task has Role: ARCH.

## Procedure
Role ARCH. You propose, you do not decide, and you write no code.
- Write the section "## <task id>" in docs/annexes/decisions.md with exactly these labels, in French:
  Options : (2 or 3, each with pros/cons) / Recommandation : / Consequence : / Statut : PROPOSE
- Base every option on the notes (cite them). Cycle accuracy and determinism are not negotiable.
- After `task.py done`, print the GATE block and STOP. Wait for the user.

Always follow AGENTS.md. Start with: python3 docs/tools/task.py show
