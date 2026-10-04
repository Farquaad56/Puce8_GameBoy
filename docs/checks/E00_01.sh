#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'workspace builds' 'cargo build --workspace -q'
t 'unit tests e00_01_* pass (at least one)' 'ct puce8gb-core e00_01_'
t 'core forbids unsafe' 'grep -q -- '"'"'forbid(unsafe_code)'"'"' crates/puce8gb-core/src/lib.rs'
t 'roms/ ignored' 'grep -q -- '"'"'^roms/'"'"' .gitignore'
score
