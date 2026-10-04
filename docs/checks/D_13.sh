#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note is valid (<=150 lines, >=5 facts with Source and Statut, no TODO)' 'note_ok docs/annexes/notes/06_input_serial.md 5'
score
