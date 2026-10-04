#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note is valid (<=150 lines, >=10 facts with Source and Statut, no TODO)' 'note_ok docs/annexes/notes/07b_mappers.md 10'
score
