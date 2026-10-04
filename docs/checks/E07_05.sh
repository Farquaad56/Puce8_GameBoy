#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e07_05_* pass (at least one)' 'ct puce8gb-desktop e07_05_'
t 'desktop builds' 'cargo build -q -p puce8gb-desktop'
score
