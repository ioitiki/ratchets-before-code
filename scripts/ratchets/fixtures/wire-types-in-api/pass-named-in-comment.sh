#!/usr/bin/env bash
# The rule named in a comment.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
// Request types derive Serialize and Deserialize in ledger-api, never here.
pub struct Handler;
RUST
