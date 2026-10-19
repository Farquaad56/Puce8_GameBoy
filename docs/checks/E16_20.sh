#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e16_20_* pass in puce8gb-cli (at least one)' 'ct puce8gb-cli e16_20_'
score
