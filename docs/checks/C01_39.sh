#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: mem_timing 02 write_timing' 'r=$(rom mem_timing/individual 02-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
score
