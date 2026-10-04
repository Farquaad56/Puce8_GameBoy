#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e01_01_* pass (at least one)' 'ct puce8gb-core e01_01_'
t 'trait Machine present' 'grep -q -- '"'"'trait Machine'"'"' crates/puce8gb-core/src/machine.rs'
score
