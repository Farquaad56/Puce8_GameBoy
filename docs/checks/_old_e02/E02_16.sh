#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "cb.rs exists" 'test -f crates/puce8gb-core/src/cpu/instructions/cb.rs'
ct puce8gb-core e02_16_
score
