#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_06_
t 'load_ptr.rs exists' 'test -f crates/puce8gb-core/src/cpu/load_ptr.rs'
score
