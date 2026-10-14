#!/usr/bin/env bash
# Skipping the derive and writing the impl by hand is the same violation.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
pub struct Leaked;
impl<'de> serde::Deserialize<'de> for Leaked {
    fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        Ok(Leaked)
    }
}
RUST
