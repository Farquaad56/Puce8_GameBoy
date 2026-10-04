#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e01_04_* pass (at least one)' 'ct puce8gb-core e01_04_'
t 'CLI info runs' 'r=$(rom cpu_instrs 01-); test -n "$r" && cargo run -q -p puce8gb-cli -- info "$r" | grep -qi title'
score
