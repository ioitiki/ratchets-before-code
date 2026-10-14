#!/usr/bin/env bash
# Helpers for ratchet fixtures. A fixture receives a scratch copy of the repo
# and plants exactly one thing in it. When the crate a fixture plants into does
# not exist yet, these helpers create a minimal one, so the same fixture works
# on the empty tree of commit #1 and on the finished workspace.
set -euo pipefail

TREE="${1:?usage: <fixture>.sh <scratch-tree>}"

# A target that never builds. Planted dependencies are declared but inert, and
# a table header of their own keeps every append valid TOML.
# shellcheck disable=SC2034  # used by the fixtures that source this file
NEVER="'cfg(target_os = \"redox\")'"

ensure_crate() {   # ensure_crate NAME
  local dir="$TREE/crates/$1"
  if [[ ! -f "$TREE/Cargo.toml" ]]; then
    printf '[workspace]\nmembers = ["crates/*"]\nresolver = "2"\n' >"$TREE/Cargo.toml"
  fi
  if [[ ! -f "$dir/Cargo.toml" ]]; then
    mkdir -p "$dir/src"
    printf '[package]\nname = "%s"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n' "$1" >"$dir/Cargo.toml"
    : >"$dir/src/lib.rs"
  fi
}

plant() {   # plant CRATE PATH < content: write a new file inside the crate
  ensure_crate "$1"
  mkdir -p "$(dirname "$TREE/crates/$1/$2")"
  cat >"$TREE/crates/$1/$2"
}

append_manifest() {   # append_manifest CRATE < toml: append to the crate's Cargo.toml
  ensure_crate "$1"
  cat >>"$TREE/crates/$1/Cargo.toml"
}
