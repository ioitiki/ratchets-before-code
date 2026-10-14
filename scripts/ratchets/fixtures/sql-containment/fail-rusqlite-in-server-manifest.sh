#!/usr/bin/env bash
# The server takes its own database dependency.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
printf '\n[target.%s.dependencies]\nrusqlite = "0.32"\n' "$NEVER" | append_manifest ledgerd
