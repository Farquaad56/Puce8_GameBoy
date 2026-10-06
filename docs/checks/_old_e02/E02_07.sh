#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "load16.rs exists" 'test -f crates/puce8gb-core/src/cpu/instructions/load16.rs'
ct puce8gb-core e02_07_
score
