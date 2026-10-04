#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: dmg_sound 07' 'r=$(rom dmg_sound/ 07-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 08' 'r=$(rom dmg_sound/ 08-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 09' 'r=$(rom dmg_sound/ 09-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 10' 'r=$(rom dmg_sound/ 10-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 11' 'r=$(rom dmg_sound/ 11-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 12' 'r=$(rom dmg_sound/ 12-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
score
