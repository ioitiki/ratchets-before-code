#!/usr/bin/env bash
# Behavior that exists only in debug builds.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
#[cfg(debug_assertions)]
pub fn shortcut() {}
RUST
