#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: cpu_instrs 11 op a,(hl)' 'r=$(rom cpu_instrs 11-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
score
