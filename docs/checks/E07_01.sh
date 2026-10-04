#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e07_01_* pass (at least one)' 'ct puce8gb-desktop e07_01_'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
t 'nearest filter used' 'grep -q -- NEAREST crates/puce8gb-desktop/src/main.rs'
t 'repaint requested' 'grep -q -- request_repaint crates/puce8gb-desktop/src/main.rs'
score
