#!/usr/bin/env bash
# Shared helpers for the ratchets. Sourced by each ratchet, never run directly.
#
# Exit-code contract for every ratchet:
#   0  the invariant holds
#   1  the invariant is violated; the violations are printed
#   2  the ratchet could not run (a missing tool, an unreadable workspace)
# "Could not run" is never reported as "holds".

set -euo pipefail
shopt -s nullglob

# ratchet_root [DIR]: the tree to check. Defaults to the enclosing git checkout.
ratchet_root() {
  if [[ -n "${1:-}" ]]; then
    (cd "$1" && pwd)
  else
    git rev-parse --show-toplevel
  fi
}

# need TOOL...: exit 2 unless every tool is on PATH.
need() {
  local tool
  for tool in "$@"; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      echo "ratchet error: '$tool' is required but not on PATH" >&2
      exit 2
    fi
  done
}

# rust_files DIR...: every .rs file under the given directories, sorted.
rust_files() {
  local dir
  for dir in "$@"; do
    if [[ -d "$dir" ]]; then
      find "$dir" -type f -name '*.rs'
    fi
  done | sort
}

# scan_lines COMMENT PATTERN FILE...: print FILE:LINE:TEXT for every line that
# matches the extended regex PATTERN, case-insensitively, and is not a
# whole-line comment (first non-blank text starts with COMMENT). Returns 1 when
# nothing matches, including when no files are given.
scan_lines() {
  local comment="$1" pattern="$2" hits
  shift 2
  (( $# > 0 )) || return 1
  hits=$(grep -EinH -- "$pattern" "$@" | grep -Ev "^[^:]+:[0-9]+:[[:space:]]*${comment}" || true)
  [[ -n "$hits" ]] || return 1
  printf '%s\n' "$hits"
}

# scan PATTERN FILE...: scan_lines for Rust sources, where comments start with //.
scan() { scan_lines '//' "$@"; }

# scan_toml PATTERN FILE...: scan_lines for TOML manifests, where comments start with #.
scan_toml() { scan_lines '#' "$@"; }

# report: indent violation lines and make their paths relative to ROOT.
report() { sed "s|^${ROOT}/||; s/^/  /"; }
