#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_10_
t 'jump.rs exists' 'test -f crates/puce8gb-core/src/cpu/jump.rs'
score
