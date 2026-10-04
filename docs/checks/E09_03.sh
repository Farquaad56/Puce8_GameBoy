#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e09_03_* pass (at least one)' 'ct puce8gb-core e09_03_'
t 'CLI save/load round trip' 'r=$(rom cpu_instrs 01-); cargo run -q --release -p puce8gb-cli -- run "$r" --frames 60 --save-state /tmp/s.state && cargo run -q --release -p puce8gb-cli -- run "$r" --frames 60 --load-state /tmp/s.state --hash | grep -q .'
score
