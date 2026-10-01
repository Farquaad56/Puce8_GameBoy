#!/usr/bin/env bash
# Scan the repository for leaked secrets.
# Prints every matching line; exits 1 if any secret is found, 0 otherwise.
set -uo pipefail

found=0

if grep -rnE 'ghp_[A-Za-z0-9]{20,}' --exclude-dir=target --exclude-dir=.git . ; then
    found=1
fi

if grep -rnEi 'password|token *[:=]' --exclude=secret_scan.sh crates scripts ; then
    found=1
fi

exit "$found"
