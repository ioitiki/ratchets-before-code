#!/usr/bin/env bash
# no-dev-paths: there is one program, and development runs it.
#
# The decay this prevents: a code path that exists only on a laptop. It starts
# as a skipped check to unblock a demo, becomes a debug-only shortcut, and ends
# as a second program sharing a name with production, where the bugs that
# matter never reproduce.
#
# Fails on any non-comment Rust line under crates/ that:
#   - gates code on debug_assertions (cfg, cfg!, cfg_attr)
#   - gates code on a feature whose name contains the word dev, debug, local,
#     insecure or skip
#   - reads an environment variable whose name contains the word SKIP,
#     INSECURE, BYPASS or DEV
# "Contains the word" means a whole segment of the name: LEDGER_DEV_MODE and
# "dev-tools" trip it; LEDGER_DEVICE_ID and "localization" do not.
#
# Usage: no-dev-paths.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
OPEN='"([^"]*[^[:alnum:]])?'
CLOSE='([^[:alnum:]][^"]*)?"'
DEBUG_CFG='cfg(_attr)?!?[[:space:]]*\([^)]*debug_assertions'
DEV_FEATURE="feature[[:space:]]*=[[:space:]]*${OPEN}(dev|debug|local|insecure|skip)${CLOSE}"
BYPASS_ENV="(env::var(_os)?|env!|option_env!)[[:space:]]*\\([[:space:]]*${OPEN}(skip|insecure|bypass|dev)${CLOSE}"

if [[ ! -d "$ROOT/crates" ]]; then
  echo "no-dev-paths: no crates yet; nothing to protect"
  exit 0
fi

files=()
while IFS= read -r file; do files+=("$file"); done < <(rust_files "$ROOT/crates")
if hits=$(scan "${DEBUG_CFG}|${DEV_FEATURE}|${BYPASS_ENV}" "${files[@]}"); then
  echo "no-dev-paths: code that only exists in development:"
  report <<<"$hits"
  exit 1
fi
echo "no-dev-paths: ok"
