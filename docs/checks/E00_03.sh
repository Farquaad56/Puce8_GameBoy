#!/usr/bin/env bash
source "$(dirname "$0")/lib.sh"
t 'file exists: .github/workflows/ci.yml' 'test -f .github/workflows/ci.yml'
t 'CI runs clippy' 'grep -q -- clippy .github/workflows/ci.yml'
t 'file exists: README.md' 'test -f README.md'
score
