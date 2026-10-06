#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_12_
t 'incdec8.rs exists' 'test -f crates/puce8gb-core/src/cpu/incdec8.rs'
score
