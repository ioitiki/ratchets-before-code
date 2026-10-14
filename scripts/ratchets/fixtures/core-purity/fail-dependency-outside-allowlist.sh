#!/usr/bin/env bash
# A normal dependency outside the allowlist, hidden in a target-specific table.
# The allowlist has to see those too.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
printf '\n[target.%s.dependencies]\nserde_json = "1"\n' "$NEVER" | append_manifest ledger-core
