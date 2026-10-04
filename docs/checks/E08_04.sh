#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e08_04_* pass (at least one)' 'ct puce8gb-core e08_04_'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
score
