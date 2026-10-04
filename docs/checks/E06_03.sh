#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'file exists: docs/annexes/baseline_score.txt' 'test -f docs/annexes/baseline_score.txt'
t 'suite meets baseline' 'cargo run -q --release -p puce8gb-cli -- suite --baseline docs/annexes/baseline_score.txt'
score
