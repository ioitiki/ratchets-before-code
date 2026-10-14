#!/usr/bin/env bash
# Explaining the rule in a comment must not break it.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
// The core never touches std::net, std::fs or SystemTime::now, and never
// depends on tokio or rusqlite. Time is an argument.
pub fn add(a: u64, b: u64) -> u64 {
    a + b
}
RUST
