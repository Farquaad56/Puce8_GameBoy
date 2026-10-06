#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'old opcode table removed' '! test -f crates/puce8gb-core/src/cpu/opcode_table.rs && ! test -f crates/puce8gb-core/tests/e02_03_cpu_tests.rs'
ct puce8gb-core c01_01_
score
