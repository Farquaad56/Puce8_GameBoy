#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e06_01_* pass (at least one)' 'ct puce8gb-cli e06_01_'
score
