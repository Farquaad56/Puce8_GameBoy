#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e02_12_* pass (at least one)' 'ct puce8gb-core e02_12_'
t 'CLI runs a ROM and exits with a defined code' 'r=$(rom cpu_instrs 01-); test -n "$r"; cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000; test $? -le 2'
score
