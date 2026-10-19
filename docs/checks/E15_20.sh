#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e15_20_* pass in puce8gb-cli (at least one)' 'ct puce8gb-cli e15_20_'
score
