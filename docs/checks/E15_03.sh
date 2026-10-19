#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note exists with >= 12 facts' 'note_ok docs/annexes/notes/11c_cgb_speed_dma.md 12'
score
