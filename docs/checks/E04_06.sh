#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'test ROM passes: dmg_sound 01' 'r=$(rom dmg_sound/ 01-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 02' 'r=$(rom dmg_sound/ 02-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 03' 'r=$(rom dmg_sound/ 03-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 04' 'r=$(rom dmg_sound/ 04-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 05' 'r=$(rom dmg_sound/ 05-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
t 'test ROM passes: dmg_sound 06' 'r=$(rom dmg_sound/ 06-); test -n "$r" && cargo run -q --release -p puce8gb-cli -- run "$r" --max-cycles 100000000 --expect-serial Passed'
score
