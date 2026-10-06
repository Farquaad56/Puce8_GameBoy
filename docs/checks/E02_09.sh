#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "alu8.rs exists" 'test -f crates/puce8gb-core/src/cpu/instructions/alu8.rs'
ct puce8gb-core e02_09_
score
