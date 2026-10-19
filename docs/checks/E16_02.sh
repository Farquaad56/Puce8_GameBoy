#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note exists with >= 14 facts' 'note_ok docs/annexes/notes/12b_sgb_graphics.md 14'
score
