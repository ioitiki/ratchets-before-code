#!/usr/bin/env bash
# all: run every ratchet against one tree. Fails if any ratchet is violated or
# could not run. Every *.sh in this directory is a ratchet except the helpers
# named below, so adding a ratchet means adding its file.
#
# Usage: all.sh [repo-root]
set -uo pipefail
shopt -s nullglob

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "${1:-$(git -C "$HERE" rev-parse --show-toplevel)}" && pwd)

ratchets=0
failed=()
for script in "$HERE"/*.sh; do
  name=$(basename "$script" .sh)
  case "$name" in lib | all | test-ratchets) continue ;; esac
  ratchets=$((ratchets + 1))
  if ! bash "$script" "$ROOT"; then
    failed+=("$name")
  fi
done

if (( ratchets == 0 )); then
  echo "no ratchets found in $HERE"
  exit 1
fi
if (( ${#failed[@]} > 0 )); then
  echo "ratchets failed: ${failed[*]}"
  exit 1
fi
echo "all $ratchets ratchets hold"
