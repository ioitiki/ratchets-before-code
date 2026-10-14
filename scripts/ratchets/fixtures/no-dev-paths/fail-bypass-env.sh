#!/usr/bin/env bash
# An environment variable that switches a check off.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
pub fn auth_disabled() -> bool {
    std::env::var("LEDGER_SKIP_AUTH").is_ok()
}
RUST
