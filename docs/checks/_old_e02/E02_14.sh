#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: cpu_instrs 06 ld r,r' 'r=$(rom cpu_instrs 06-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
t 'test ROM passes: cpu_instrs 07 jr,jp,call,ret,rst' 'r=$(rom cpu_instrs 07-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
t 'test ROM passes: cpu_instrs 08 misc instrs' 'r=$(rom cpu_instrs 08-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
t 'test ROM passes: cpu_instrs 09 op r,r' 'r=$(rom cpu_instrs 09-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 60000000 --expect-serial Passed'
score
