#!/usr/bin/env bash
# Sourced by every docs/checks/<id>.sh. Prints "[ok]/[FAIL]" lines and a final "SCORE: p/n".
cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 2
P=0; N=0
ct() {   # ct <crate> <test-name-filter> : needs exit 0 AND at least one test executed
  local out rc
  out=$(cargo test -q -p "$1" "$2" 2>&1); rc=$?
  echo "$out" | tail -n 3
  [ $rc -eq 0 ] && echo "$out" | grep -Eq 'test result: ok\. [1-9][0-9]* passed'
}
rom() {  # rom <dir-pattern> <name-prefix> : first matching test ROM under roms/test-roms
  find roms/test-roms -ipath "*$1*" -name "$2*.gb" 2>/dev/null | sort | head -1
}
note_ok() {  # note_ok <file> <min-facts> : exists, <=150 lines, enough Source/Statut lines
  [ -f "$1" ] && [ "$(wc -l < "$1")" -le 150 ] \
    && [ "$(grep -c '^Source :' "$1")" -ge "$2" ] && [ "$(grep -c '^Statut :' "$1")" -ge "$2" ] \
    && ! grep -q 'TODO' "$1"
}
dec_ok() {  # dec_ok <A_xx> : decision section has the 4 required labels
  local s k
  s=$(awk -v id="## $1" '$0==id{f=1;next} /^## /{f=0} f' docs/annexes/decisions.md)
  for k in 'Options :' 'Recommandation :' 'Consequence :' 'Statut : '; do
    echo "$s" | grep -q "^$k" || return 1
  done
}
export -f ct rom note_ok dec_ok
t() {    # t "<description>" "<shell command>"
  N=$((N+1))
  local tmp; tmp=$(mktemp)
  if bash -c "$2" >"$tmp" 2>&1; then P=$((P+1)); echo "[ok]   $1"
  else echo "[FAIL] $1"; tail -n 6 "$tmp" | cut -c1-200; fi
  rm -f "$tmp"
}
score() { echo "SCORE: $P/$N"; }
