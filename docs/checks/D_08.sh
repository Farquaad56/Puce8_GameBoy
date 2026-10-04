#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note is valid (<=150 lines, >=4 facts with Source and Statut, no TODO)' 'note_ok docs/annexes/notes/03b_io_registers.md 4'
score
