#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "timer.rs exists" 'test -f crates/puce8gb-core/src/timer.rs'
ct puce8gb-core e02_25_
score
