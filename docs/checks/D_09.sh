#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note is valid (<=150 lines, >=8 facts with Source and Statut, no TODO)' 'note_ok docs/annexes/notes/04a_video_regs.md 8'
score
