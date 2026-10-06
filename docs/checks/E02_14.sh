#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "misc.rs exists" 'test -f crates/puce8gb-core/src/cpu/instructions/misc.rs'
ct puce8gb-core e02_14_
score
