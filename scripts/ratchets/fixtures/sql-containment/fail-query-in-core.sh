#!/usr/bin/env bash
# A duplicate check written as a query, inside the core.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
pub const FIND_BY_KEY: &str = "SELECT id FROM jobs WHERE idempotency_key = ?1";
RUST
