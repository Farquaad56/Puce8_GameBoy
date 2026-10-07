#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-cli c01_43_
t 'run_args.rs exists' 'test -f crates/puce8gb-cli/src/run_args.rs'
score
