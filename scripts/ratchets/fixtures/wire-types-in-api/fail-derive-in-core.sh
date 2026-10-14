#!/usr/bin/env bash
# The core declares its own wire type.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
#[derive(Debug, serde::Serialize)]
pub struct Leaked;
RUST
