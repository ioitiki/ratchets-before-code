#!/usr/bin/env bash
# The core reads the clock instead of being told the time.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
pub fn stamp() -> std::time::SystemTime {
    std::time::SystemTime::now()
}
RUST
