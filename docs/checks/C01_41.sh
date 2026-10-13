#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
for n in 01 02 03 04 05 06 07 08 09 10 11; do
  t "cpu_instrs individual $n passes" "r=\$(rom cpu_instrs/individual $n-); test -n \"\$r\" && cargo run -q --release -p puce8gb-cli -- run \"\$r\" --max-cycles 200000000 --expect-serial Passed"
done
t 'HALT unit tests' 'cargo test -q -p puce8gb-core c01_33_ 2>&1 | grep -Eq "test result: ok\. [1-9][0-9]* passed"'
score
