#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'note is valid (<=150 lines, >=8 facts with Source and Statut, no TODO)' 'note_ok docs/annexes/notes/05a_audio_channels.md 8'
score
