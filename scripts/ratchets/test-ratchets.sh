#!/usr/bin/env bash
# test-ratchets: proves every ratchet can fail, and that a comment cannot trip one.
#
# A ratchet that has never failed is decoration. For each ratchet R in this
# directory:
#   - R must exit 0 on a clean copy of the tree.
#   - fixtures/R/ must hold at least one fail-*.sh. Each plants one violation in
#     a fresh copy, and R must then exit exactly 1. Exit 2 means R could not
#     run, which is not the same as R catching the violation.
#   - every fixtures/R/pass-*.sh plants something that must NOT trip R (the
#     forbidden thing named in a comment, or an allowed lookalike), and R must
#     still exit 0.
# A fixtures/ directory that names no ratchet is also a failure.
#
# Usage: test-ratchets.sh [repo-root]
# Exit:  0 everything proven, 1 otherwise.
set -uo pipefail
shopt -s nullglob

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "${1:-$(git -C "$HERE" rev-parse --show-toplevel)}" && pwd)
failures=0
ratchets=0

copy_tree() {
  local dst
  dst=$(mktemp -d)
  tar -C "$ROOT" --exclude=./target --exclude=./.git -cf - . | tar -C "$dst" -xf -
  echo "$dst"
}

expect() {   # expect LABEL WANT RATCHET TREE
  local label=$1 want=$2 ratchet=$3 tree=$4 out got line
  out=$(bash "$HERE/$ratchet.sh" "$tree" 2>&1)
  got=$?
  if (( got == want )); then
    printf '  ok    %-46s exit %d\n' "$label" "$got"
  else
    printf '  FAIL  %-46s wanted exit %d, got %d\n' "$label" "$want" "$got"
    while IFS= read -r line; do printf '        %s\n' "$line"; done <<<"$out"
    failures=$((failures + 1))
  fi
}

run_fixture() {   # run_fixture FIXTURE WANT RATCHET
  local fixture=$1 want=$2 ratchet=$3 tree
  tree=$(copy_tree)
  if bash "$fixture" "$tree"; then
    expect "$(basename "$fixture" .sh)" "$want" "$ratchet" "$tree"
  else
    printf '  FAIL  %-46s the fixture itself failed\n' "$(basename "$fixture" .sh)"
    failures=$((failures + 1))
  fi
  rm -rf "$tree"
}

for script in "$HERE"/*.sh; do
  ratchet=$(basename "$script" .sh)
  case "$ratchet" in lib | all | test-ratchets) continue ;; esac
  ratchets=$((ratchets + 1))
  echo "$ratchet"

  tree=$(copy_tree)
  expect "clean tree" 0 "$ratchet" "$tree"
  rm -rf "$tree"

  fails=("$HERE/fixtures/$ratchet"/fail-*.sh)
  if (( ${#fails[@]} == 0 )); then
    echo "  FAIL  no fail-* fixture: nothing proves this ratchet can fail"
    failures=$((failures + 1))
  fi
  for fixture in "${fails[@]}"; do run_fixture "$fixture" 1 "$ratchet"; done
  for fixture in "$HERE/fixtures/$ratchet"/pass-*.sh; do run_fixture "$fixture" 0 "$ratchet"; done
done

for dir in "$HERE"/fixtures/*/; do
  name=$(basename "$dir")
  if [[ ! -f "$HERE/$name.sh" ]]; then
    echo "$name"
    echo "  FAIL  fixtures/$name exists, but there is no $name.sh ratchet"
    failures=$((failures + 1))
  fi
done

if (( ratchets == 0 )); then
  echo "no ratchets found in $HERE"
  exit 1
fi
if (( failures > 0 )); then
  echo "$failures ratchet self-test(s) failed"
  exit 1
fi
echo "all $ratchets ratchets can fail, and no comment trips one"
