#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e15_22_* pass in puce8gb-cli (at least one)' 'ct puce8gb-cli e15_22_'
t 'file exists: docs/annexes/baseline_score_cgb.txt' 'test -f docs/annexes/baseline_score_cgb.txt'
t 'DMG baseline untouched' 'test "$(cat docs/annexes/baseline_score.txt | tr -d "[:space:]")" = 117'
score
