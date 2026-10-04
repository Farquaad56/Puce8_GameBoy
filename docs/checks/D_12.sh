#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note is valid (<=150 lines, >=6 facts with Source and Statut, no TODO)' 'note_ok docs/annexes/notes/05b_audio_mixing.md 6'
score
