#!/usr/bin/env bash
# The core opens a socket.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
pub fn leak() -> bool {
    std::net::TcpStream::connect("127.0.0.1:9").is_ok()
}
RUST
