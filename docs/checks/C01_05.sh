#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_05_
t 'load8.rs exists' 'test -f crates/puce8gb-core/src/cpu/load8.rs'
score
