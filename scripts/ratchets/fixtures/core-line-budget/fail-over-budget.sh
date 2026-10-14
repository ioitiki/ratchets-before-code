#!/usr/bin/env bash
# 900 implementation lines in one file: over budget whatever else exists.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
for i in $(seq 1 900); do
  printf 'pub const PLANTED_%d: u32 = %d;\n' "$i" "$i"
done | plant ledger-core src/planted.rs
