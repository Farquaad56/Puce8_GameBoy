#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note exists with >= 8 facts' 'note_ok docs/annexes/notes/10_boot_rom.md 8'
score
