#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "alu16.rs exists" 'test -f crates/puce8gb-core/src/cpu/instructions/alu16.rs'
ct puce8gb-core e02_12_
score
