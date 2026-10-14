#!/usr/bin/env bash
# A tests.rs file is test code, and test code does not count.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
for i in $(seq 1 900); do
  printf 'pub const T_%d: u32 = %d;\n' "$i" "$i"
done | plant ledger-core src/tests.rs
