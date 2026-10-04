#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: cpu_instrs 01 special' 'r=$(rom cpu_instrs 01-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
t 'test ROM passes: cpu_instrs 03 op sp,hl' 'r=$(rom cpu_instrs 03-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
t 'test ROM passes: cpu_instrs 04 op r,imm' 'r=$(rom cpu_instrs 04-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
t 'test ROM passes: cpu_instrs 05 op rp' 'r=$(rom cpu_instrs 05-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
score
