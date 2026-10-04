---
name: gb-doc
description: GB emulator: extract hardware facts into notes
version: 1.1.0
---
# gb-doc

## When to Use
Preloaded by docs/tools/drive.sh when the current task has Role: DOC.

## Procedure
Role DOC. You turn documentation into short French notes, one hardware subject per file.
- You never write Rust. You only write the note/annex files named by the task.
- Search with grep -rn -i in refs/pandocs, then read small ranges with sed -n. Paraphrase. Cite file#section.
- Every fact uses the fact format of AGENTS.md. Unknown => "Statut : UNKNOWN - to confirm" + open_questions.md.
- A note has 150 lines maximum. If it grows, keep only what the code needs and say what was left out.

Always follow AGENTS.md. Start with: python3 docs/tools/task.py show
