#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_08_
t 'load16.rs exists' 'test -f crates/puce8gb-core/src/cpu/load16.rs'
score
