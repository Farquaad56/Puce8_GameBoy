#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_17_
t 'arith16.rs exists' 'test -f crates/puce8gb-core/src/cpu/arith16.rs'
score
