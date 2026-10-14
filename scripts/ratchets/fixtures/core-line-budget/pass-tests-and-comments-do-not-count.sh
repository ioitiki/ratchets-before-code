#!/usr/bin/env bash
# Comments, blank lines and an inline #[cfg(test)] module add nothing to the count.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
{
  for i in $(seq 1 900); do printf '// comment line %d\n\n' "$i"; done
  printf '#[cfg(test)]\nmod planted_tests {\n'
  for i in $(seq 1 900); do printf '    const T_%d: u32 = %d;\n' "$i" "$i"; done
  printf '}\n'
} | plant ledger-core src/planted.rs
