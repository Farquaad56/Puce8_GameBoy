#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'hash is deterministic' 'r=$(rom dmg-acid2 dmg-acid2); test -n "$r"; a=$(cargo run -q --release -p puce8gb-cli -- run "$r" --frames 120 --hash); b=$(cargo run -q --release -p puce8gb-cli -- run "$r" --frames 120 --hash); test -n "$a" && test "$a" = "$b"'
t 'ppm dump works' 'r=$(rom dmg-acid2 dmg-acid2); cargo run -q --release -p puce8gb-cli -- run "$r" --frames 120 --dump-ppm /tmp/acid2.ppm && test -s /tmp/acid2.ppm'
score
