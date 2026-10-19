#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e15_21_* pass in puce8gb-core (at least one)' 'ct puce8gb-core e15_21_'
t 'observed hash recorded' 'test -f docs/annexes/cgb_acid2_observed.txt'
score
