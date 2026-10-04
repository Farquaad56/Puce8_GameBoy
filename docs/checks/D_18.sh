#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'matrix present' 'grep -q -- Couvert docs/annexes/coverage.md'
t '9 subjects rated' 'test $(grep -c '"'"'^|'"'"' docs/annexes/coverage.md) -ge 10'
score
