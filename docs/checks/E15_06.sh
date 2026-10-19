#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e15_06_* pass in puce8gb-core (at least one)' 'ct puce8gb-core e15_06_'
score
