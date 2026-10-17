#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'unit tests e05_07_* pass (at least one)' 'ct puce8gb-core e05_07_'
t 'Bus no longer owns an interim sram field' '! grep -nE "^\s*pub sram:" crates/puce8gb-core/src/bus.rs'
t 'Bus holds a media::Mapper' 'grep -q "Mapper" crates/puce8gb-core/src/bus.rs'
score
