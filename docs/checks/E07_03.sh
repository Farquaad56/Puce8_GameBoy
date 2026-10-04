#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e07_03_* pass (at least one)' 'ct puce8gb-desktop e07_03_'
t 'no Mutex in audio module' '! grep -q Mutex crates/puce8gb-desktop/src/audio.rs'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
score
