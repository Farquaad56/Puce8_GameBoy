#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_18_
t 'sp_offset.rs exists' 'test -f crates/puce8gb-core/src/cpu/sp_offset.rs'
score
