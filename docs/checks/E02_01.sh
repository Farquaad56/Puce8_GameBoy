#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "registers.rs exists" 'test -f crates/puce8gb-core/src/cpu/registers.rs'
ct puce8gb-core e02_01_
score
