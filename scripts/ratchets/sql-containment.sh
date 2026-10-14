#!/usr/bin/env bash
# sql-containment: the storage engine is a one-crate decision.
#
# The decay this prevents: a query written wherever it was convenient. One in a
# handler, one in a migration helper, one in the core to check a duplicate.
# After that the schema is known in five places, every schema change becomes a
# search, and replacing SQLite is a rewrite instead of a crate swap.
#
# Fails when, outside crates/ledger-store:
#   - a non-comment source line contains a SQL statement or names rusqlite
#   - a Cargo.toml names rusqlite on a non-comment line
#
# Usage: sql-containment.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
STORE=ledger-store
START='(^|[^[:alnum:]_])'
END='([^[:alnum:]_]|$)'
STATEMENT="select[[:space:]].+[[:space:]]from|insert[[:space:]]+into|update[[:space:]]+[[:alnum:]_\"]+[[:space:]]+set|delete[[:space:]]+from|(create|alter|drop)[[:space:]]+table"
SQL="${START}(${STATEMENT})${END}|${START}rusqlite${END}"

if [[ ! -d "$ROOT/crates" ]]; then
  echo "sql-containment: no crates yet; nothing to protect"
  exit 0
fi

status=0
for crate in "$ROOT"/crates/*/; do
  crate=${crate%/}
  name=$(basename "$crate")
  if [[ "$name" == "$STORE" ]]; then
    continue
  fi
  files=()
  while IFS= read -r file; do files+=("$file"); done < <(rust_files "$crate")
  if hits=$(scan "$SQL" "${files[@]}"); then
    echo "sql-containment: SQL outside $STORE, in $name:"
    report <<<"$hits"
    status=1
  fi
  if [[ -f "$crate/Cargo.toml" ]] && hits=$(scan_toml "${START}rusqlite${END}" "$crate/Cargo.toml"); then
    echo "sql-containment: $name's manifest depends on rusqlite:"
    report <<<"$hits"
    status=1
  fi
done

if (( status == 0 )); then
  echo "sql-containment: ok"
fi
exit "$status"
