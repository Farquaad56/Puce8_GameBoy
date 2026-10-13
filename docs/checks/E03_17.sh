#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'halt_bug image identical to the reference' 'r=$(rom blargg halt_bug); png=$(find roms/test-roms -path "*/blargg/*" -name "halt_bug*.png" | sort | head -1); test -n "$r" && test -n "$png" && cargo run -q --release -p puce8gb-cli -- run "$r" --frames 300 --dump-ppm docs/annexes/halt_bug.ppm && python docs/tools/compare_png_ppm.py docs/annexes/halt_bug.ppm "$png"'
score
