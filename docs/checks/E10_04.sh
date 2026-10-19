#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e10_04_* pass in puce8gb-desktop (at least one)' 'ct puce8gb-desktop e10_04_'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
score
