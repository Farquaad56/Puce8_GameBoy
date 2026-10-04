#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'pandocs present' 'test -d refs/pandocs && ls refs/pandocs | head -1 | grep -q .'
t 'test ROMs present' 'find roms/test-roms -name '"'"'*.gb'"'"' | head -1 | grep -q .'
t 'inventory records commits' 'grep -q -- pandocs docs/annexes/sources_inventory.md'
score
