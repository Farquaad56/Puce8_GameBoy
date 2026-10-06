#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_13_
t 'alu_add.rs exists' 'test -f crates/puce8gb-core/src/cpu/alu_add.rs'
score
