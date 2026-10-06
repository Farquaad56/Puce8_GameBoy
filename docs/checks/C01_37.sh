#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: instr_timing' 'r=$(rom instr_timing instr_timing); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
score
