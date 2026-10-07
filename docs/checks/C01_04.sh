#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'run prints first trace line' 'r=$(rom cpu_instrs 06-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 400 --trace 1 2>&1 | grep -q "A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100"'
t 'run exit code is not 3 or 64' 'r=$(rom cpu_instrs 06-); cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 400 >/dev/null 2>&1; rc=$?; test $rc -ne 3 && test $rc -ne 64'
score
