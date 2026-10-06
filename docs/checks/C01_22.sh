#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_22_
t 'acc_misc.rs exists' 'test -f crates/puce8gb-core/src/cpu/acc_misc.rs'
score
