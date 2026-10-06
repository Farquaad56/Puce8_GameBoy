#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: halt_bug' 'r=$(rom blargg halt_bug); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
score
