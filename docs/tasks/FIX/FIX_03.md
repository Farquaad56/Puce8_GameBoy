# FIX_03 - Fix test: mooneye acceptance/oam_dma_start
Role: TEST
Goal: Make test "mooneye acceptance/oam_dma_start" pass in the suite without regressing any other test.
Prerequisites: none
Read (max 3 files, give line ranges): docs/annexes/notes/09_test_roms.md; the one note about the failing subsystem
Write only in: crates/
## Work
1. Run: cargo run -q --release -p puce8gb-cli -- suite --only "mooneye acceptance/oam_dma_start"  (note the failure text).
2. Find the smallest reproduction (one instruction, one register, one cycle). Write a failing unit test first, named fix_03_*.
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
