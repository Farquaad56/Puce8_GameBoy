#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: mem_timing 01 read' 'r=$(rom mem_timing/ 01-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 30000000 --expect-serial Passed'
score
