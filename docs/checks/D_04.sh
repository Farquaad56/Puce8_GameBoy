#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'file exists: docs/annexes/code/opcodes.rs' 'test -f docs/annexes/code/opcodes.rs'
t '256 base entries' 'test $(grep -c '"'"'OpInfo {'"'"' docs/annexes/code/opcodes.rs) -ge 512'
t 'generated file is ASCII' '! grep -qP '"'"'[^\x00-\x7F]'"'"' docs/annexes/code/opcodes.rs'
t 'source note valid' 'note_ok docs/annexes/notes/02c_cpu_opcodes_src.md 2'
score
