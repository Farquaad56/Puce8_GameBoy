#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e16_17_* pass in puce8gb-core (at least one)' 'ct puce8gb-core e16_17_'
score
