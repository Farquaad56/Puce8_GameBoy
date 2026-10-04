#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e01_02_* pass (at least one)' 'ct puce8gb-core e01_02_'
score
