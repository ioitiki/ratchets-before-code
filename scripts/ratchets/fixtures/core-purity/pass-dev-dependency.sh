#!/usr/bin/env bash
# Dev-dependencies are test tooling, not part of the core, so the allowlist
# ignores them.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
printf '\n[target.%s.dev-dependencies]\nproptest = "1"\n' "$NEVER" | append_manifest ledger-core
