#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-cli c01_48_
ct puce8gb-cli c01_4
t 'seam exists' 'grep -q debug_set_unimplemented crates/puce8gb-core/src/cpu/mod.rs'
score
