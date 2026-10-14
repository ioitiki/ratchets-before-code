#!/usr/bin/env bash
# ledger-api is where wire types belong.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-api src/planted.rs <<'RUST'
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Wire;
RUST
