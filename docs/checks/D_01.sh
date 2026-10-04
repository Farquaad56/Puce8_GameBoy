#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'table present' 'grep -q -- '"'"'feeds note'"'"' docs/annexes/sources_inventory.md'
t 'at least 15 rows' 'test $(grep -c '"'"'^|'"'"' docs/annexes/sources_inventory.md) -ge 15'
score
