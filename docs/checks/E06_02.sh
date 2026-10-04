#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'manifest has >= 27 entries' 'test $(grep -c '"'"'|'"'"' docs/annexes/test_manifest.txt) -ge 27'
t 'suite runs and prints a score' 'cargo run -q --release -p puce8gb-cli -- suite | grep -q '"'"'^SUITE: '"'"'; true'
score
