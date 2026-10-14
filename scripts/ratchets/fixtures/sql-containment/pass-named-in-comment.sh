#!/usr/bin/env bash
# The rule named in a comment, and Rust identifiers that only look like SQL.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
// Never SELECT ... FROM a table here: all SQL lives in ledger-store (rusqlite).
pub fn update_set(selector: u32) -> u32 {
    selector.saturating_add(1)
}
RUST
