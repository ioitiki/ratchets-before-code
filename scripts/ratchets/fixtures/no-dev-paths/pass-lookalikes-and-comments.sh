#!/usr/bin/env bash
# Names that only contain the forbidden words inside a larger word, a
# debug_assert! (which is not a gate), and the rule named in a comment.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
// Never gate on cfg(debug_assertions) or read LEDGER_SKIP_AUTH.
#[cfg(feature = "localization")]
pub fn greet() {}
pub fn device() -> Option<String> {
    debug_assert!(true);
    std::env::var("LEDGER_DEVICE_ID").ok()
}
RUST
