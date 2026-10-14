# Plan 01: Commit #1, the ratchets before the code

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`../specs/2026-10-14-ratchets-before-code-design.md`](../specs/2026-10-14-ratchets-before-code-design.md), build step 1 (§7).
**Executed by:** the coordinator, directly. Commit #1 is the only commit that does not go through a PR (spec §7).

**Goal:** the repository's first commit contains the five ratchets, a self-test proving each can fail, the CI workflow that runs them first, ADR 0001, the spec and this plan. No crate exists yet.

**Architecture:**
- Each ratchet is a standalone bash script in `scripts/ratchets/`. It takes an optional repo-root argument and exits 0 (holds), 1 (violated) or 2 (could not run).
- `all.sh` runs every ratchet.
- `test-ratchets.sh` runs each ratchet against scratch copies of the tree after a fixture has planted something: `fail-*` fixtures must turn the ratchet red, and `pass-*` fixtures (comments, lookalikes) must not.
- With no crates present, every ratchet passes trivially, and the fixtures create the crate they plant into.

**Tech stack:**
- bash, plus GNU or BSD `grep`/`sed`/`awk`/`tar`
- `jq`, and `cargo metadata --no-deps --offline` (verified to need neither a registry nor a lockfile)
- `shellcheck`
- GitHub Actions
- a Nix flake dev shell for local work

**Conventions for every file in this plan:**
- **Confidentiality:** the spec's §3 rules apply. No file may contain any term from the operator's private forbidden-terms list.
- **Format:** each code block below is the complete content of the file named directly above it, in the form `<!-- file: path -->`.

---

## File map

| Path | Responsibility |
|---|---|
| `.gitignore`, `.envrc`, `.shellcheckrc`, `flake.nix`, `flake.lock` | dev environment |
| `README.md` | pointer to the record until the essay exists |
| `scripts/ratchets/lib.sh` | shared helpers: root, tool check, file listing, comment-aware scan, report |
| `scripts/ratchets/all.sh` | run every ratchet |
| `scripts/ratchets/test-ratchets.sh` | prove every ratchet can fail and that comments cannot trip one |
| `scripts/ratchets/fixtures/lib.sh` | fixture helpers: `ensure_crate`, `plant`, `append_manifest` |
| `scripts/ratchets/core-purity.sh` + `fixtures/core-purity/*` | core dependency allowlist, plus core source free of I/O and ambient time |
| `scripts/ratchets/core-line-budget.sh` + `fixtures/core-line-budget/*` | core implementation-line budget |
| `scripts/ratchets/sql-containment.sh` + `fixtures/sql-containment/*` | SQL and rusqlite only in ledger-store |
| `scripts/ratchets/no-dev-paths.sh` + `fixtures/no-dev-paths/*` | no development-only code paths |
| `scripts/ratchets/wire-types-in-api.sh` + `fixtures/wire-types-in-api/*` | serde only in ledger-api |
| `.github/workflows/ci.yml` | ratchets → build → crash-recovery |
| `docs/decisions/0001-ratchets-land-before-code.md` | ADR 0001 |
| `docs/pipeline/reports/2026-10-14-01-ratchets-first-commit.md` | live proof for this plan |

---

### Task 1: Dev environment

**Files:** create `.gitignore`, `.envrc`, `.shellcheckrc`, `flake.nix`, `README.md`

- [ ] **Step 1: Write the files**

<!-- file: .gitignore -->
```
/target
/.direnv
```

<!-- file: .envrc -->
```
use flake
```

<!-- file: .shellcheckrc -->
```
# Resolve `source` paths relative to the script that sources them.
source-path=SCRIPTDIR
```

<!-- file: flake.nix -->
```nix
{
  description = "ratchets-before-code: development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy jq shellcheck sqlite ];
        };
      });
    };
}
```

<!-- file: README.md -->
```markdown
# ratchets-before-code

This repository is being built by the method it will describe: architectural
invariants enforced as failing builds and landed before the code they protect,
plus a brief → spec → plan → live-proof pipeline run with AI coding agents.

The essay replaces this page when the build is done. Until then, the record is
the point:

- [The design](docs/pipeline/specs/2026-10-14-ratchets-before-code-design.md)
- [The first decision](docs/decisions/0001-ratchets-land-before-code.md)
- [The ratchets](scripts/ratchets/), which exist before any crate does
```

- [ ] **Step 2: Lock the flake and verify the shell**

Run: `git add -A && nix flake lock && nix develop -c sh -c 'cargo --version && jq --version && shellcheck --version | head -2'`
Expected: `flake.lock` is created, and a cargo version, a jq version and a shellcheck version are printed.

---

### Task 2: The harness, failing on purpose

**Files:** create `scripts/ratchets/lib.sh`, `scripts/ratchets/all.sh`, `scripts/ratchets/test-ratchets.sh`, `scripts/ratchets/fixtures/lib.sh`

- [ ] **Step 1: Write the harness**

<!-- file: scripts/ratchets/lib.sh -->
```bash
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
```

<!-- file: scripts/ratchets/all.sh -->
```bash
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
```

<!-- file: scripts/ratchets/test-ratchets.sh -->
```bash
#!/usr/bin/env bash
# test-ratchets: proves every ratchet can fail, and that a comment cannot trip one.
#
# A ratchet that has never failed is decoration. For each ratchet R in this
# directory:
#   - R must exit 0 on a clean copy of the tree.
#   - fixtures/R/ must hold at least one fail-*.sh. Each plants one violation in
#     a fresh copy, and R must then exit exactly 1. Exit 2 means R could not
#     run, which is not the same as R catching the violation.
#   - every fixtures/R/pass-*.sh plants something that must NOT trip R (the
#     forbidden thing named in a comment, or an allowed lookalike), and R must
#     still exit 0.
# A fixtures/ directory that names no ratchet is also a failure.
#
# Usage: test-ratchets.sh [repo-root]
# Exit:  0 everything proven, 1 otherwise.
set -uo pipefail
shopt -s nullglob

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "${1:-$(git -C "$HERE" rev-parse --show-toplevel)}" && pwd)
failures=0
ratchets=0

copy_tree() {
  local dst
  dst=$(mktemp -d)
  tar -C "$ROOT" --exclude=./target --exclude=./.git -cf - . | tar -C "$dst" -xf -
  echo "$dst"
}

expect() {   # expect LABEL WANT RATCHET TREE
  local label=$1 want=$2 ratchet=$3 tree=$4 out got line
  out=$(bash "$HERE/$ratchet.sh" "$tree" 2>&1)
  got=$?
  if (( got == want )); then
    printf '  ok    %-46s exit %d\n' "$label" "$got"
  else
    printf '  FAIL  %-46s wanted exit %d, got %d\n' "$label" "$want" "$got"
    while IFS= read -r line; do printf '        %s\n' "$line"; done <<<"$out"
    failures=$((failures + 1))
  fi
}

run_fixture() {   # run_fixture FIXTURE WANT RATCHET
  local fixture=$1 want=$2 ratchet=$3 tree
  tree=$(copy_tree)
  if bash "$fixture" "$tree"; then
    expect "$(basename "$fixture" .sh)" "$want" "$ratchet" "$tree"
  else
    printf '  FAIL  %-46s the fixture itself failed\n' "$(basename "$fixture" .sh)"
    failures=$((failures + 1))
  fi
  rm -rf "$tree"
}

for script in "$HERE"/*.sh; do
  ratchet=$(basename "$script" .sh)
  case "$ratchet" in lib | all | test-ratchets) continue ;; esac
  ratchets=$((ratchets + 1))
  echo "$ratchet"

  tree=$(copy_tree)
  expect "clean tree" 0 "$ratchet" "$tree"
  rm -rf "$tree"

  fails=("$HERE/fixtures/$ratchet"/fail-*.sh)
  if (( ${#fails[@]} == 0 )); then
    echo "  FAIL  no fail-* fixture: nothing proves this ratchet can fail"
    failures=$((failures + 1))
  fi
  for fixture in "${fails[@]}"; do run_fixture "$fixture" 1 "$ratchet"; done
  for fixture in "$HERE/fixtures/$ratchet"/pass-*.sh; do run_fixture "$fixture" 0 "$ratchet"; done
done

for dir in "$HERE"/fixtures/*/; do
  name=$(basename "$dir")
  if [[ ! -f "$HERE/$name.sh" ]]; then
    echo "$name"
    echo "  FAIL  fixtures/$name exists, but there is no $name.sh ratchet"
    failures=$((failures + 1))
  fi
done

if (( ratchets == 0 )); then
  echo "no ratchets found in $HERE"
  exit 1
fi
if (( failures > 0 )); then
  echo "$failures ratchet self-test(s) failed"
  exit 1
fi
echo "all $ratchets ratchets can fail, and no comment trips one"
```

<!-- file: scripts/ratchets/fixtures/lib.sh -->
```bash
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
```

- [ ] **Step 2: Run the self-test and watch it fail**

Run: `chmod +x scripts/ratchets/*.sh && nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: `no ratchets found in …/scripts/ratchets` and `exit=1`. A harness with nothing to prove must not pass.

---

### Task 3: core-purity

**Files:**
- Create: `scripts/ratchets/fixtures/core-purity/{fail-dependency-outside-allowlist,fail-network-in-source,fail-ambient-time,pass-named-in-comment,pass-dev-dependency}.sh`
- Create: `scripts/ratchets/core-purity.sh`

- [ ] **Step 1: Write the fixtures**

<!-- file: scripts/ratchets/fixtures/core-purity/fail-dependency-outside-allowlist.sh -->
```bash
#!/usr/bin/env bash
# A normal dependency outside the allowlist, hidden in a target-specific table.
# The allowlist has to see those too.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
printf '\n[target.%s.dependencies]\nserde_json = "1"\n' "$NEVER" | append_manifest ledger-core
```

<!-- file: scripts/ratchets/fixtures/core-purity/fail-network-in-source.sh -->
```bash
#!/usr/bin/env bash
# The core opens a socket.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
pub fn leak() -> bool {
    std::net::TcpStream::connect("127.0.0.1:9").is_ok()
}
RUST
```

<!-- file: scripts/ratchets/fixtures/core-purity/fail-ambient-time.sh -->
```bash
#!/usr/bin/env bash
# The core reads the clock instead of being told the time.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
pub fn stamp() -> std::time::SystemTime {
    std::time::SystemTime::now()
}
RUST
```

<!-- file: scripts/ratchets/fixtures/core-purity/pass-named-in-comment.sh -->
```bash
#!/usr/bin/env bash
# Explaining the rule in a comment must not break it.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
// The core never touches std::net, std::fs or SystemTime::now, and never
// depends on tokio or rusqlite. Time is an argument.
pub fn add(a: u64, b: u64) -> u64 {
    a + b
}
RUST
```

<!-- file: scripts/ratchets/fixtures/core-purity/pass-dev-dependency.sh -->
```bash
#!/usr/bin/env bash
# Dev-dependencies are test tooling, not part of the core, so the allowlist
# ignores them.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
printf '\n[target.%s.dev-dependencies]\nproptest = "1"\n' "$NEVER" | append_manifest ledger-core
```

- [ ] **Step 2: Run the self-test and watch it fail**

Run: `nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: `FAIL  fixtures/core-purity exists, but there is no core-purity.sh ratchet` and `exit=1`.

- [ ] **Step 3: Write the ratchet**

<!-- file: scripts/ratchets/core-purity.sh -->
```bash
#!/usr/bin/env bash
# core-purity: ledger-core decides; it never does.
#
# The decay this prevents: the pure core picks up I/O one reasonable exception
# at a time. A file read to load a setting, a clock call to stamp an event, a
# query to check for a duplicate. Each is small. Together they mean the
# decision logic can no longer be tested exhaustively, because testing it now
# needs a disk, a clock and a database.
#
# Two checks:
#   1. ledger-core's normal dependencies are an ALLOWLIST, read from
#      `cargo metadata` rather than grepped out of Cargo.toml, so
#      target-specific and renamed dependencies are seen too. A denylist only
#      stops the crate someone already thought of.
#   2. No non-comment source line may reach for I/O, ambient time or entropy.
#      Time is an argument to the core (ADR 0002).
#
# Usage: core-purity.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
CORE="$ROOT/crates/ledger-core"
ALLOWED='["ledger-api"]'
FORBIDDEN='std::(net|fs|process|env)|(SystemTime|Instant)::now|(^|[^[:alnum:]_])(tokio|rusqlite|reqwest|rand)([^[:alnum:]_]|$)'

if [[ ! -d "$CORE" ]]; then
  echo "core-purity: no crates/ledger-core yet; nothing to protect"
  exit 0
fi
need cargo jq

errors=$(mktemp)
trap 'rm -f "$errors"' EXIT
if ! metadata=$(cd "$ROOT" && cargo metadata --format-version 1 --no-deps --offline 2>"$errors"); then
  echo "ratchet error: cargo metadata failed:" >&2
  cat "$errors" >&2
  exit 2
fi
if ! jq -e 'any(.packages[]; .name == "ledger-core")' >/dev/null <<<"$metadata"; then
  echo "ratchet error: crates/ledger-core exists but is not a workspace member, so its dependencies cannot be checked" >&2
  exit 2
fi

status=0

# shellcheck disable=SC2016  # $allowed is a jq variable, not a shell one
outside=$(jq -r --argjson allowed "$ALLOWED" '
  .packages[]
  | select(.name == "ledger-core")
  | .dependencies[]
  | select(.kind == null)
  | select(.name | IN($allowed[]) | not)
  | .name + (if .target then " (target " + .target + ")" else "" end)
' <<<"$metadata")
if [[ -n "$outside" ]]; then
  echo "core-purity: ledger-core has normal dependencies outside its allowlist $ALLOWED:"
  report <<<"$outside"
  status=1
fi

files=()
while IFS= read -r file; do files+=("$file"); done < <(rust_files "$CORE/src")
if hits=$(scan "$FORBIDDEN" "${files[@]}"); then
  echo "core-purity: ledger-core source reaches for I/O, ambient time or entropy:"
  report <<<"$hits"
  status=1
fi

if (( status == 0 )); then
  echo "core-purity: ok"
fi
exit "$status"
```

- [ ] **Step 4: Run the self-test and watch it pass**

Run: `chmod +x scripts/ratchets/*.sh && nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: under `core-purity`, the clean tree and both `pass-*` fixtures show `exit 0`, the three `fail-*` fixtures show `exit 1`, and `exit=0`.

---

### Task 4: core-line-budget

**Files:**
- Create: `scripts/ratchets/fixtures/core-line-budget/{fail-over-budget,pass-tests-and-comments-do-not-count,pass-tests-file-does-not-count}.sh`
- Create: `scripts/ratchets/core-line-budget.sh`

- [ ] **Step 1: Write the fixtures**

<!-- file: scripts/ratchets/fixtures/core-line-budget/fail-over-budget.sh -->
```bash
#!/usr/bin/env bash
# 900 implementation lines in one file: over budget whatever else exists.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
for i in $(seq 1 900); do
  printf 'pub const PLANTED_%d: u32 = %d;\n' "$i" "$i"
done | plant ledger-core src/planted.rs
```

<!-- file: scripts/ratchets/fixtures/core-line-budget/pass-tests-and-comments-do-not-count.sh -->
```bash
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
```

<!-- file: scripts/ratchets/fixtures/core-line-budget/pass-tests-file-does-not-count.sh -->
```bash
#!/usr/bin/env bash
# A tests.rs file is test code, and test code does not count.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
for i in $(seq 1 900); do
  printf 'pub const T_%d: u32 = %d;\n' "$i" "$i"
done | plant ledger-core src/tests.rs
```

- [ ] **Step 2: Run the self-test and watch it fail**

Run: `nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: `FAIL  fixtures/core-line-budget exists, but there is no core-line-budget.sh ratchet` and `exit=1`.

- [ ] **Step 3: Write the ratchet**

<!-- file: scripts/ratchets/core-line-budget.sh -->
```bash
#!/usr/bin/env bash
# core-line-budget: the core stays small enough to read in one sitting.
#
# The decay this prevents: the "pure core" grows by accretion until the phrase
# describes a directory, not a constraint. A budget only constrains if it starts
# below the size the code would drift to on its own. Set at today's size plus
# slack, it measures nothing.
#
# Raising BUDGET is allowed, and it should cost something: it means editing the
# number below in the same diff as the code that needs it, where a reviewer can
# see the trade.
#
# What counts: non-blank, non-comment lines in crates/ledger-core/src/**/*.rs,
# excluding files named tests.rs and everything from a `#[cfg(test)]` line to
# the end of its file. Test modules go at the bottom of their file.
#
# Usage: core-line-budget.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

BUDGET=800
ROOT=$(ratchet_root "${1:-}")
SRC="$ROOT/crates/ledger-core/src"

if [[ ! -d "$SRC" ]]; then
  echo "core-line-budget: no crates/ledger-core yet; nothing to measure"
  exit 0
fi

files=()
while IFS= read -r file; do
  if [[ "$(basename "$file")" != tests.rs ]]; then
    files+=("$file")
  fi
done < <(rust_files "$SRC")

lines=0
if (( ${#files[@]} > 0 )); then
  lines=$(awk '
    FNR == 1 { in_tests = 0 }
    /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/ { in_tests = 1 }
    in_tests { next }
    /^[[:space:]]*$/ { next }
    /^[[:space:]]*\/\// { next }
    { n++ }
    END { print n + 0 }
  ' "${files[@]}")
fi

if (( lines > BUDGET )); then
  echo "core-line-budget: ledger-core has $lines implementation lines; the budget is $BUDGET"
  echo "  Shrink the core, or raise BUDGET in scripts/ratchets/core-line-budget.sh in this same diff."
  exit 1
fi
echo "core-line-budget: ok ($lines of $BUDGET lines)"
```

- [ ] **Step 4: Run the self-test and watch it pass**

Run: `chmod +x scripts/ratchets/*.sh && nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: under `core-line-budget`, the clean tree and both `pass-*` fixtures show `exit 0`, `fail-over-budget` shows `exit 1`, and `exit=0`.

---

### Task 5: sql-containment

**Files:**
- Create: `scripts/ratchets/fixtures/sql-containment/{fail-query-in-core,fail-rusqlite-in-server-manifest,pass-query-in-store,pass-named-in-comment}.sh`
- Create: `scripts/ratchets/sql-containment.sh`

- [ ] **Step 1: Write the fixtures**

<!-- file: scripts/ratchets/fixtures/sql-containment/fail-query-in-core.sh -->
```bash
#!/usr/bin/env bash
# A duplicate check written as a query, inside the core.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
pub const FIND_BY_KEY: &str = "SELECT id FROM jobs WHERE idempotency_key = ?1";
RUST
```

<!-- file: scripts/ratchets/fixtures/sql-containment/fail-rusqlite-in-server-manifest.sh -->
```bash
#!/usr/bin/env bash
# The server takes its own database dependency.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
printf '\n[target.%s.dependencies]\nrusqlite = "0.32"\n' "$NEVER" | append_manifest ledgerd
```

<!-- file: scripts/ratchets/fixtures/sql-containment/pass-query-in-store.sh -->
```bash
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
```

<!-- file: scripts/ratchets/fixtures/sql-containment/pass-named-in-comment.sh -->
```bash
#!/usr/bin/env bash
# The rule named in a comment, and Rust identifiers that only look like SQL.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
// Never SELECT ... FROM a table here: all SQL lives in ledger-store (rusqlite).
pub fn update_set(selector: u32) -> u32 {
    selector.saturating_add(1)
}
RUST
```

- [ ] **Step 2: Run the self-test and watch it fail**

Run: `nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: `FAIL  fixtures/sql-containment exists, but there is no sql-containment.sh ratchet` and `exit=1`.

- [ ] **Step 3: Write the ratchet**

<!-- file: scripts/ratchets/sql-containment.sh -->
```bash
#!/usr/bin/env bash
# sql-containment: the storage engine is a one-crate decision.
#
# The decay this prevents: a query written wherever it was convenient. One in a
# handler, one in a migration helper, one in the core to check a duplicate.
# After that the schema is known in five places, every schema change becomes a
# search, and replacing SQLite is a rewrite instead of a crate swap.
#
# Fails when, outside crates/ledger-store:
#   - a non-comment source line contains a SQL statement or names rusqlite
#   - a Cargo.toml names rusqlite on a non-comment line
#
# Usage: sql-containment.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
STORE=ledger-store
START='(^|[^[:alnum:]_])'
END='([^[:alnum:]_]|$)'
STATEMENT="select[[:space:]].+[[:space:]]from|insert[[:space:]]+into|update[[:space:]]+[[:alnum:]_\"]+[[:space:]]+set|delete[[:space:]]+from|(create|alter|drop)[[:space:]]+table"
SQL="${START}(${STATEMENT})${END}|${START}rusqlite${END}"

if [[ ! -d "$ROOT/crates" ]]; then
  echo "sql-containment: no crates yet; nothing to protect"
  exit 0
fi

status=0
for crate in "$ROOT"/crates/*/; do
  crate=${crate%/}
  name=$(basename "$crate")
  if [[ "$name" == "$STORE" ]]; then
    continue
  fi
  files=()
  while IFS= read -r file; do files+=("$file"); done < <(rust_files "$crate")
  if hits=$(scan "$SQL" "${files[@]}"); then
    echo "sql-containment: SQL outside $STORE, in $name:"
    report <<<"$hits"
    status=1
  fi
  if [[ -f "$crate/Cargo.toml" ]] && hits=$(scan_toml "${START}rusqlite${END}" "$crate/Cargo.toml"); then
    echo "sql-containment: $name's manifest depends on rusqlite:"
    report <<<"$hits"
    status=1
  fi
done

if (( status == 0 )); then
  echo "sql-containment: ok"
fi
exit "$status"
```

- [ ] **Step 4: Run the self-test and watch it pass**

Run: `chmod +x scripts/ratchets/*.sh && nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: under `sql-containment`, the clean tree and both `pass-*` fixtures show `exit 0`, both `fail-*` fixtures show `exit 1`, and `exit=0`.

---

### Task 6: no-dev-paths

**Files:**
- Create: `scripts/ratchets/fixtures/no-dev-paths/{fail-debug-assertions,fail-dev-feature,fail-bypass-env,pass-lookalikes-and-comments}.sh`
- Create: `scripts/ratchets/no-dev-paths.sh`

- [ ] **Step 1: Write the fixtures**

<!-- file: scripts/ratchets/fixtures/no-dev-paths/fail-debug-assertions.sh -->
```bash
#!/usr/bin/env bash
# Behavior that exists only in debug builds.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
#[cfg(debug_assertions)]
pub fn shortcut() {}
RUST
```

<!-- file: scripts/ratchets/fixtures/no-dev-paths/fail-dev-feature.sh -->
```bash
#!/usr/bin/env bash
# A feature flag that turns on a development-only path.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
#[cfg(feature = "dev-mode")]
pub fn seed_demo_data() {}
RUST
```

<!-- file: scripts/ratchets/fixtures/no-dev-paths/fail-bypass-env.sh -->
```bash
#!/usr/bin/env bash
# An environment variable that switches a check off.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
pub fn auth_disabled() -> bool {
    std::env::var("LEDGER_SKIP_AUTH").is_ok()
}
RUST
```

<!-- file: scripts/ratchets/fixtures/no-dev-paths/pass-lookalikes-and-comments.sh -->
```bash
#!/usr/bin/env bash
# Names that only contain the forbidden words inside a larger word, a
# debug_assert! (which is not a gate), and the rule named in a comment.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
// Never gate on cfg(debug_assertions) or read LEDGER_SKIP_AUTH.
#[cfg(feature = "localization")]
pub fn greet() {}
pub fn device() -> Option<String> {
    debug_assert!(true);
    std::env::var("LEDGER_DEVICE_ID").ok()
}
RUST
```

- [ ] **Step 2: Run the self-test and watch it fail**

Run: `nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: `FAIL  fixtures/no-dev-paths exists, but there is no no-dev-paths.sh ratchet` and `exit=1`.

- [ ] **Step 3: Write the ratchet**

<!-- file: scripts/ratchets/no-dev-paths.sh -->
```bash
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
```

- [ ] **Step 4: Run the self-test and watch it pass**

Run: `chmod +x scripts/ratchets/*.sh && nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: under `no-dev-paths`, the clean tree and `pass-lookalikes-and-comments` show `exit 0`, the three `fail-*` fixtures show `exit 1`, and `exit=0`.

---

### Task 7: wire-types-in-api

**Files:**
- Create: `scripts/ratchets/fixtures/wire-types-in-api/{fail-derive-in-core,fail-hand-impl-in-server,pass-derive-in-api,pass-named-in-comment}.sh`
- Create: `scripts/ratchets/wire-types-in-api.sh`

- [ ] **Step 1: Write the fixtures**

<!-- file: scripts/ratchets/fixtures/wire-types-in-api/fail-derive-in-core.sh -->
```bash
#!/usr/bin/env bash
# The core declares its own wire type.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-core src/planted.rs <<'RUST'
#[derive(Debug, serde::Serialize)]
pub struct Leaked;
RUST
```

<!-- file: scripts/ratchets/fixtures/wire-types-in-api/fail-hand-impl-in-server.sh -->
```bash
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
```

<!-- file: scripts/ratchets/fixtures/wire-types-in-api/pass-derive-in-api.sh -->
```bash
#!/usr/bin/env bash
# ledger-api is where wire types belong.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledger-api src/planted.rs <<'RUST'
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Wire;
RUST
```

<!-- file: scripts/ratchets/fixtures/wire-types-in-api/pass-named-in-comment.sh -->
```bash
#!/usr/bin/env bash
# The rule named in a comment.
# shellcheck source=../lib.sh
source "$(dirname "$0")/../lib.sh"
plant ledgerd src/planted.rs <<'RUST'
// Request types derive Serialize and Deserialize in ledger-api, never here.
pub struct Handler;
RUST
```

- [ ] **Step 2: Run the self-test and watch it fail**

Run: `nix develop -c ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"`
Expected: `FAIL  fixtures/wire-types-in-api exists, but there is no wire-types-in-api.sh ratchet` and `exit=1`.

- [ ] **Step 3: Write the ratchet**

<!-- file: scripts/ratchets/wire-types-in-api.sh -->
```bash
#!/usr/bin/env bash
# wire-types-in-api: a type that crosses a process boundary is declared once.
#
# The decay this prevents: the same request shape defined twice, once by the
# server and once by a client, drifting one renamed field apart until a deploy
# breaks only in production. Serialization is how a type crosses a boundary,
# so serialization lives in ledger-api and nowhere else.
#
# Fails when a crate other than ledger-api derives or hand-implements serde's
# Serialize or Deserialize on a non-comment line.
#
# Usage: wire-types-in-api.sh [repo-root]
# Exit:  0 holds, 1 violated, 2 could not run.

# shellcheck source=lib.sh
source "$(dirname "$0")/lib.sh"

ROOT=$(ratchet_root "${1:-}")
API=ledger-api
PATTERN='derive[[:space:]]*\([^)]*(Serialize|Deserialize)|impl[^{]*(Serialize|Deserialize)[^{]*[[:space:]]for[[:space:]]'

if [[ ! -d "$ROOT/crates" ]]; then
  echo "wire-types-in-api: no crates yet; nothing to protect"
  exit 0
fi

status=0
for crate in "$ROOT"/crates/*/; do
  crate=${crate%/}
  name=$(basename "$crate")
  if [[ "$name" == "$API" ]]; then
    continue
  fi
  files=()
  while IFS= read -r file; do files+=("$file"); done < <(rust_files "$crate")
  if hits=$(scan "$PATTERN" "${files[@]}"); then
    echo "wire-types-in-api: $name serializes types; wire types belong in $API:"
    report <<<"$hits"
    status=1
  fi
done

if (( status == 0 )); then
  echo "wire-types-in-api: ok"
fi
exit "$status"
```

- [ ] **Step 4: Run the full self-test and the ratchets**

Run: `chmod +x scripts/ratchets/*.sh && nix develop -c sh -c './scripts/ratchets/test-ratchets.sh && ./scripts/ratchets/all.sh'; echo "exit=$?"`
Expected: `all 5 ratchets can fail, and no comment trips one`, then each ratchet printing `nothing to protect` (or `nothing to measure`), then `all 5 ratchets hold`, and `exit=0`.

---

### Task 8: CI and shellcheck

**Files:** create `.github/workflows/ci.yml`

- [ ] **Step 1: Write the workflow**

<!-- file: .github/workflows/ci.yml -->
```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  # First and cheapest. There is no toolchain install step: the runner already
  # has bash, jq, cargo and shellcheck, and a broken invariant should fail
  # before anyone waits on a compiler.
  ratchets:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: shellcheck the ratchets
        run: shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh
      - name: the ratchets hold
        run: ./scripts/ratchets/all.sh
      - name: every ratchet can fail
        run: ./scripts/ratchets/test-ratchets.sh

  build:
    needs: ratchets
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # The workspace arrives with plan 02. Until then this job says so in the
      # run summary instead of looking like a build that passed.
      - id: gate
        run: |
          if [ -f Cargo.toml ]; then
            echo "ready=true" >> "$GITHUB_OUTPUT"
          else
            echo "::notice::No Cargo workspace yet. The build turns on when plan 02 adds one."
          fi
      - if: steps.gate.outputs.ready == 'true'
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - if: steps.gate.outputs.ready == 'true'
        run: cargo fmt --all -- --check
      - if: steps.gate.outputs.ready == 'true'
        run: cargo clippy --workspace --all-targets -- -D warnings
      - if: steps.gate.outputs.ready == 'true'
        run: cargo test --workspace

  crash-recovery:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # Present from the first commit, so the crash test has a place waiting
      # for it. It turns on when plan 04 adds the test.
      - id: gate
        run: |
          if [ -f crates/ledgerd/tests/crash_recovery.rs ]; then
            echo "ready=true" >> "$GITHUB_OUTPUT"
          else
            echo "::notice::No crash test yet. This job turns on when plan 04 adds it."
          fi
      - if: steps.gate.outputs.ready == 'true'
        uses: dtolnay/rust-toolchain@stable
      - if: steps.gate.outputs.ready == 'true'
        run: cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture
```

- [ ] **Step 2: shellcheck every script**

Run: `nix develop -c shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh; echo "exit=$?"`
Expected: no findings and `exit=0`. Fix any finding in the script itself; never silence one without a comment saying why.

---

### Task 9: ADR 0001 and spec alignment

**Files:**
- Create: `docs/decisions/0001-ratchets-land-before-code.md`
- Modify: `docs/pipeline/specs/2026-10-14-ratchets-before-code-design.md` (§5 and §7 wording, to match what this plan builds)

- [ ] **Step 1: Write ADR 0001**

<!-- file: docs/decisions/0001-ratchets-land-before-code.md -->
```markdown
# ADR 0001: The ratchets land before the code they protect

**Status:** accepted
**Date:** 2026-10-14
**Decided by:** Andy

## Context

This repository is built mostly by AI coding agents working from written
plans. Agents are fast, and they are obliging. Asked to make a test pass by the
end of a task, a good agent finds the shortest path, and the shortest path
through an architecture usually runs through one of its boundaries. A person
under a deadline does the same, just less often.

Each such exception is defensible on the day it is made. The damage is in the
sum, and nobody ever approves the sum. A design document cannot stop it,
because a document has no way to fail a build. Review does not reliably stop
it either: the reviewer who should catch the fifth exception is the one who
waved through the first four.

## Decision

Every structural rule this repository depends on is a script that fails the
build, and those scripts are in the first commit, before any crate exists.

So the first commit holds:
- this decision
- the design spec
- five ratchets under `scripts/ratchets/`
- a self-test that proves each ratchet can fail
- the CI workflow that runs them before anything else

## Why first

- **Today they cost nothing.** Against an empty tree, every ratchet passes on
  its first run. A ratchet introduced later arrives with every violation that
  accumulated before it, and its first conversation is about which of those
  really count.
- **A rule that runs is a different thing from a rule that is written down.**
  "The core is pure" is a sentence in the spec. `core-purity.sh` is the same
  sentence, checked on every pull request, including the ones an agent opens at
  3 a.m.
- **A budget binds only when it starts below need.** `core-line-budget.sh`
  allows 800 lines for a core expected to settle at 400-600. Starting at
  whatever size the core reaches, plus some slack, would record the code's size
  rather than limit it.

## Consequences

- **Dependency rules are allowlists.** A denylist blocks only the crate
  someone thought of in advance.
- **Source scans skip whole-line comments.** Otherwise the explanation of a
  rule becomes its first violation, and people learn to silence the check.
- **Loosening a rule is a visible diff.** Raising a budget or widening an
  allowlist is allowed, and it is done by editing the ratchet in the same diff
  as the code that needs it. The trade is then in front of the reviewer, not in
  a setting nobody reads.
- **Every ratchet ships with fixtures that prove it can fail**
  (`test-ratchets.sh`). That also stops a ratchet from being weakened quietly:
  an agent that "fixes" a red build by loosening the check turns the self-test
  red instead.
- **They are a floor, not a proof.** The ratchets are text scans and metadata
  checks, and someone set on evading them can. What they catch is the
  ordinary, reasonable shortcut.
```

- [ ] **Step 2: Align the spec with what was built**

Run:

```bash
node - <<'EOF'
const fs = require('fs');
const p = 'docs/pipeline/specs/2026-10-14-ratchets-before-code-design.md';
let s = fs.readFileSync(p, 'utf8');
const swap = (a, b) => { if (!s.includes(a)) throw new Error('missing: ' + a.slice(0, 60)); s = s.replace(a, b); };
swap("| `wire-types-in-api.sh` | `Serialize` or `Deserialize` derives appear in any crate other than `ledger-api`. |",
     "| `wire-types-in-api.sh` | `Serialize` or `Deserialize` derives, or hand-written impls of them, appear in any crate other than `ledger-api`. |");
swap("makes a temporary copy of the repo, plants one known violation for each ratchet (from `scripts/ratchets/fixtures/`), and asserts that the ratchet exits non-zero. It also asserts that every ratchet exits zero on a clean tree. Any ratchet without a fixture fails this test.",
     "runs each ratchet against scratch copies of the repo. Every ratchet must exit 0 on a clean tree. Each `fixtures/<ratchet>/fail-*.sh` plants one violation, and the ratchet must exit exactly 1 (exit 2, \"could not run\", does not count). Each `fixtures/<ratchet>/pass-*.sh` plants something that must not trip it, such as the forbidden thing named in a comment or an allowed lookalike, and the ratchet must exit 0. A ratchet without a `fail-*` fixture, or a fixtures directory without a ratchet, fails the test.");
swap("Commit #1 contains this spec, ADR 0001, the five ratchets with their self-tests, and the CI workflow.",
     "Commit #1 contains this spec, plan 01 and its report, ADR 0001, the five ratchets with their self-tests, the CI workflow, and the dev shell.");
swap("1. `ratchets`: `all.sh` and `test-ratchets.sh`. No Rust toolchain needed.",
     "1. `ratchets`: shellcheck, `all.sh`, then `test-ratchets.sh`. There is no toolchain install step; the job uses the bash, jq, cargo and shellcheck already on the runner.");
swap("1. **Commit #1:** this spec, ADR 0001, the ratchets, their self-tests and the CI workflow.",
     "1. **Commit #1:** this spec, plan 01 and its report, ADR 0001, the ratchets, their self-tests, the CI workflow and the dev shell.");
fs.writeFileSync(p, s);
EOF
```

Expected: no output and exit 0. If a `missing:` error appears, the spec text has drifted; fix the plan's quoted text, not the spec.

---

### Task 10: Live proof, commit #1, private remote, CI

**Files:** create `docs/pipeline/reports/2026-10-14-01-ratchets-first-commit.md`

- [ ] **Step 1: Run every gate and capture the real output**

Run:

```bash
mkdir -p docs/pipeline/reports
nix develop -c sh -c '
  echo "\$ shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh"
  shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh && echo "(no findings)"; echo "exit $?"
  echo; echo "\$ ./scripts/ratchets/all.sh"; ./scripts/ratchets/all.sh; echo "exit $?"
  echo; echo "\$ ./scripts/ratchets/test-ratchets.sh"; ./scripts/ratchets/test-ratchets.sh; echo "exit $?"
' > /tmp/plan01-gates.txt 2>&1; cat /tmp/plan01-gates.txt
```

Expected: shellcheck reports no findings, `all 5 ratchets hold`, and `all 5 ratchets can fail, and no comment trips one`, each followed by `exit 0`.

- [ ] **Step 2: Write the report from that output**

The report has these sections:
- **Measured:** the verbatim contents of `/tmp/plan01-gates.txt` in a fenced block, with the date and the `nix develop` toolchain versions.
- **Inferred:** whatever the local run does not prove. At minimum: CI on GitHub's runner has not run yet, its result is recorded in the next report, and the runner's tools are assumed preinstalled.
- **Deviations from the plan,** if any.

Nothing in the report may be paraphrased output.

- [ ] **Step 3: Confidentiality scan before the first commit**

Run: `grep -rniF -f ~/.config/ratchets-before-code/forbidden-terms.txt --exclude-dir=.git . ; echo "matches-exit=$?"`
Expected: no lines, and `matches-exit=1` (grep found nothing). Any match blocks the commit.

- [ ] **Step 4: Commit #1**

```bash
git add -A
git status --short
git commit -m "Ratchets before code: commit #1

Five ratchets, a self-test proving each can fail, and the CI that runs them
first, landed before any crate exists. Includes the design spec, plan 01, its
live-proof report and ADR 0001.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git log --format='%h %an <%ae>%n%B' -1
```

Expected: one commit authored by `Andrew Malkin <26385844+ioitiki@users.noreply.github.com>`, carrying the trailer.

- [ ] **Step 5: Create the private remote and push**

Run: `gh repo create ioitiki/ratchets-before-code --private --source . --remote origin --push --description "Architectural invariants as failing builds, landed before the code they protect: built by orchestrated AI agents, with the record to prove it."`
Expected: the repository is created as private and `main` is pushed.

- [ ] **Step 6: Watch CI**

Run: `gh run watch --exit-status $(gh run list --limit 1 --json databaseId --jq '.[0].databaseId')`
Expected:
- `ratchets` passes
- `build` and `crash-recovery` pass, each with a notice that it turns on later

A red run is not fixed by editing this plan's history. It gets a new report and a follow-up commit through a PR.
