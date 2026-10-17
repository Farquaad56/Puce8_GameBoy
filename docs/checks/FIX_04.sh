#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t "target test passes" 'cargo run -q --release -p puce8gb-cli -- suite --only "mooneye acceptance/interrupts/ie_push"'
t "suite >= baseline" 'cargo run -q --release -p puce8gb-cli -- suite --baseline docs/annexes/baseline_score.txt'
t "fix unit test exists" 'ct puce8gb-core fix_04_'
score
