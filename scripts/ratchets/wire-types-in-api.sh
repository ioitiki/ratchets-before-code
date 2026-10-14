#!/usr/bin/env bash
# wire-types-in-api: a type that crosses a process boundary is declared once.
#
# The decay this prevents: the same request shape defined twice, once by the
# server and once by a client, drifting one renamed field apart until a deploy
# breaks only in production. Serialization is how a type crosses a boundary,
# so serialization lives in ledger-api and nowhere else.
#
# Fails when a crate other than ledger-api derives or hand-implements serde's
# Serialize or Deserialize on a non-comment line.
#
# Usage: wire-types-in-api.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
API=ledger-api
PATTERN='derive[[:space:]]*\([^)]*(Serialize|Deserialize)|impl[^{]*(Serialize|Deserialize)[^{]*[[:space:]]for[[:space:]]'

if [[ ! -d "$ROOT/crates" ]]; then
  echo "wire-types-in-api: no crates yet; nothing to protect"
  exit 0
fi

status=0
for crate in "$ROOT"/crates/*/; do
  crate=${crate%/}
  name=$(basename "$crate")
  if [[ "$name" == "$API" ]]; then
    continue
  fi
  files=()
  while IFS= read -r file; do files+=("$file"); done < <(rust_files "$crate")
  if hits=$(scan "$PATTERN" "${files[@]}"); then
    echo "wire-types-in-api: $name serializes types; wire types belong in $API:"
    report <<<"$hits"
    status=1
  fi
done

if (( status == 0 )); then
  echo "wire-types-in-api: ok"
fi
exit "$status"
