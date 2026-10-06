#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: cpu_instrs 08 misc instrs' 'r=$(rom cpu_instrs 08-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
score
