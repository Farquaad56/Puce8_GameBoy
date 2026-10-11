#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
ct puce8gb-core c01_49_
ct puce8gb-cli c01_49_
t 'option exists' 'grep -rq "ly-fixed" crates/puce8gb-cli/src'
t 'override exists' 'grep -rq "ly_override" crates/puce8gb-core/src'
score
