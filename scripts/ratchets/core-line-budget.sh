#!/usr/bin/env bash
# core-line-budget: the core stays small enough to read in one sitting.
#
# The decay this prevents: the "pure core" grows by accretion until the phrase
# describes a directory, not a constraint. A budget only constrains if it starts
# below the size the code would drift to on its own. Set at today's size plus
# slack, it measures nothing.
#
# Raising BUDGET is allowed, and it should cost something: it means editing the
# number below in the same diff as the code that needs it, where a reviewer can
# see the trade.
#
# What counts: non-blank, non-comment lines in crates/ledger-core/src/**/*.rs,
# excluding files named tests.rs and everything from a `#[cfg(test)]` line to
# the end of its file. Test modules go at the bottom of their file.
#
# Usage: core-line-budget.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

BUDGET=250
ROOT=$(ratchet_root "${1:-}")
SRC="$ROOT/crates/ledger-core/src"

if [[ ! -d "$SRC" ]]; then
  echo "core-line-budget: no crates/ledger-core yet; nothing to measure"
  exit 0
fi

files=()
while IFS= read -r file; do
  if [[ "$(basename "$file")" != tests.rs ]]; then
    files+=("$file")
  fi
done < <(rust_files "$SRC")

lines=0
if (( ${#files[@]} > 0 )); then
  lines=$(awk '
    FNR == 1 { in_tests = 0 }
    /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/ { in_tests = 1 }
    in_tests { next }
    /^[[:space:]]*$/ { next }
    /^[[:space:]]*\/\// { next }
    { n++ }
    END { print n + 0 }
  ' "${files[@]}")
fi

if (( lines > BUDGET )); then
  echo "core-line-budget: ledger-core has $lines implementation lines; the budget is $BUDGET"
  echo "  Shrink the core, or raise BUDGET in scripts/ratchets/core-line-budget.sh in this same diff."
  exit 1
fi
echo "core-line-budget: ok ($lines of $BUDGET lines)"
