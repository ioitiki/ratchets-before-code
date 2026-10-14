#!/usr/bin/env bash
# The store is where SQL belongs.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-store src/planted.rs <<'RUST'
use rusqlite::Connection;
pub const FIND_BY_KEY: &str = "SELECT id FROM jobs WHERE idempotency_key = ?1";
pub fn open() -> rusqlite::Result<Connection> {
    Connection::open_in_memory()
}
RUST
