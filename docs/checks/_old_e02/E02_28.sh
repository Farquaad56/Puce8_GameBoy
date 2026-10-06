#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "serial.rs exists" 'test -f crates/puce8gb-core/src/serial.rs'
ct puce8gb-core e02_28_
score
