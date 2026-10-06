#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_07_
t 'load_abs.rs exists' 'test -f crates/puce8gb-core/src/cpu/load_abs.rs'
score
