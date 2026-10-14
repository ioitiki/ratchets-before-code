#!/usr/bin/env bash
# core-purity: ledger-core decides; it never does.
#
# The decay this prevents: the pure core picks up I/O one reasonable exception
# at a time. A file read to load a setting, a clock call to stamp an event, a
# query to check for a duplicate. Each is small. Together they mean the
# decision logic can no longer be tested exhaustively, because testing it now
# needs a disk, a clock and a database.
#
# Two checks:
#   1. ledger-core's normal dependencies are an ALLOWLIST, read from
#      `cargo metadata` rather than grepped out of Cargo.toml, so
#      target-specific and renamed dependencies are seen too. A denylist only
#      stops the crate someone already thought of.
#   2. No non-comment source line may reach for I/O, ambient time or entropy.
#      Time is an argument to the core (ADR 0002).
#
# Usage: core-purity.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
CORE="$ROOT/crates/ledger-core"
ALLOWED='["ledger-api"]'
FORBIDDEN='std::(net|fs|process|env)|(SystemTime|Instant)::now|(^|[^[:alnum:]_])(tokio|rusqlite|reqwest|rand)([^[:alnum:]_]|$)'

if [[ ! -d "$CORE" ]]; then
  echo "core-purity: no crates/ledger-core yet; nothing to protect"
  exit 0
fi
need cargo jq

errors=$(mktemp)
trap 'rm -f "$errors"' EXIT
if ! metadata=$(cd "$ROOT" && cargo metadata --format-version 1 --no-deps --offline 2>"$errors"); then
  echo "ratchet error: cargo metadata failed:" >&2
  cat "$errors" >&2
  exit 2
fi
if ! jq -e 'any(.packages[]; .name == "ledger-core")' >/dev/null <<<"$metadata"; then
  echo "ratchet error: crates/ledger-core exists but is not a workspace member, so its dependencies cannot be checked" >&2
  exit 2
fi

status=0

# shellcheck disable=SC2016  # $allowed is a jq variable, not a shell one
outside=$(jq -r --argjson allowed "$ALLOWED" '
  .packages[]
  | select(.name == "ledger-core")
  | .dependencies[]
  | select(.kind == null)
  | select(.name | IN($allowed[]) | not)
  | .name + (if .target then " (target " + .target + ")" else "" end)
' <<<"$metadata")
if [[ -n "$outside" ]]; then
  echo "core-purity: ledger-core has normal dependencies outside its allowlist $ALLOWED:"
  report <<<"$outside"
  status=1
fi

files=()
while IFS= read -r file; do files+=("$file"); done < <(rust_files "$CORE/src")
if hits=$(scan "$FORBIDDEN" "${files[@]}"); then
  echo "core-purity: ledger-core source reaches for I/O, ambient time or entropy:"
  report <<<"$hits"
  status=1
fi

if (( status == 0 )); then
  echo "core-purity: ok"
fi
exit "$status"
