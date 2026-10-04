#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e09_04_* pass (at least one)' 'ct puce8gb-desktop e09_04_'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
score
