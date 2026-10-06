#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_11_
t 'call.rs exists' 'test -f crates/puce8gb-core/src/cpu/call.rs'
score
