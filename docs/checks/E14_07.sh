#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e14_07_* pass in puce8gb-desktop (at least one)' 'ct puce8gb-desktop e14_07_'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
score
