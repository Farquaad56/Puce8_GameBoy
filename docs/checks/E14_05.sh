#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e14_05_* pass in puce8gb-cli (at least one)' 'ct puce8gb-cli e14_05_'
score
