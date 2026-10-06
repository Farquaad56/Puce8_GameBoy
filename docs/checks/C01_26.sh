#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_26_
t 'cb_bit.rs exists' 'test -f crates/puce8gb-core/src/cpu/cb_bit.rs'
score
