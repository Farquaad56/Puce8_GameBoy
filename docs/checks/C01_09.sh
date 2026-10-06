#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_09_
t 'stack.rs exists' 'test -f crates/puce8gb-core/src/cpu/stack.rs'
score
