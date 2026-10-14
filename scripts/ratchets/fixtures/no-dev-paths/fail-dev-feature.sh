#!/usr/bin/env bash
# A feature flag that turns on a development-only path.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
#[cfg(feature = "dev-mode")]
pub fn seed_demo_data() {}
RUST
