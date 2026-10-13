#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-cli e03_16_
t 'option frames' 'grep -rq "\-\-frames" crates/puce8gb-cli/src'
t 'option dump' 'grep -rq "dump-ppm" crates/puce8gb-cli/src'
score
