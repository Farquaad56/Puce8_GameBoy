#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "flow.rs exists" 'test -f crates/puce8gb-core/src/cpu/instructions/flow.rs'
ct puce8gb-core e02_19_
score
