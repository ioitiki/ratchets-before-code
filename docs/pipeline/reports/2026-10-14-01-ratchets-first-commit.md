# Report 01: commit #1, the ratchets before the code

**Plan:** [`../plans/2026-10-14-01-ratchets-first-commit.md`](../plans/2026-10-14-01-ratchets-first-commit.md)
**Executed by:** the coordinator (Claude Code), directly on `main`, as spec §7 requires for commit #1.
**Date:** 2026-10-14

## Measured

Every block below is verbatim tool output captured during execution. The only edits:
- one absolute local path in a red-step message is shortened to `…/scripts/ratchets`
- Nix's "Git tree is dirty" warning lines are removed

### The red steps, written test-first

Each ratchet's fixtures were written before the ratchet itself, and the self-test was run to confirm it failed. Task 2 has no ratchets at all, so the harness must refuse to pass:

```
$ ./scripts/ratchets/test-ratchets.sh; echo "exit=$?"
no ratchets found in …/scripts/ratchets
exit=1
```

Tasks 3–7: fixtures present, ratchet held back. Shown here is the FAIL line from each run:

```
core-purity
  FAIL  fixtures/core-purity exists, but there is no core-purity.sh ratchet
exit=1
  FAIL  fixtures/core-line-budget exists, but there is no core-line-budget.sh ratchet
exit=1
  FAIL  fixtures/sql-containment exists, but there is no sql-containment.sh ratchet
exit=1
  FAIL  fixtures/no-dev-paths exists, but there is no no-dev-paths.sh ratchet
exit=1
  FAIL  fixtures/wire-types-in-api exists, but there is no wire-types-in-api.sh ratchet
exit=1
```

### The gates, on the tree as committed

```
$ date -u
2026-10-14T22:57Z

$ cargo --version; jq --version; shellcheck --version | sed -n 2p; bash --version | head -1
cargo 1.98.0 (797e8a9bc 2026-08-05)
jq-1.8.2
version: 0.11.0
GNU bash, version 5.3.15(1)-release (x86_64-pc-linux-gnu)

$ shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh
(no findings)
exit 0

$ ./scripts/ratchets/all.sh
core-line-budget: no crates/ledger-core yet; nothing to measure
core-purity: no crates/ledger-core yet; nothing to protect
no-dev-paths: no crates yet; nothing to protect
sql-containment: no crates yet; nothing to protect
wire-types-in-api: no crates yet; nothing to protect
all 5 ratchets hold
exit 0

$ ./scripts/ratchets/test-ratchets.sh
core-line-budget
  ok    clean tree                                     exit 0
  ok    fail-over-budget                               exit 1
  ok    pass-tests-and-comments-do-not-count           exit 0
  ok    pass-tests-file-does-not-count                 exit 0
core-purity
  ok    clean tree                                     exit 0
  ok    fail-ambient-time                              exit 1
  ok    fail-dependency-outside-allowlist              exit 1
  ok    fail-network-in-source                         exit 1
  ok    pass-dev-dependency                            exit 0
  ok    pass-named-in-comment                          exit 0
no-dev-paths
  ok    clean tree                                     exit 0
  ok    fail-bypass-env                                exit 1
  ok    fail-debug-assertions                          exit 1
  ok    fail-dev-feature                               exit 1
  ok    pass-lookalikes-and-comments                   exit 0
sql-containment
  ok    clean tree                                     exit 0
  ok    fail-query-in-core                             exit 1
  ok    fail-rusqlite-in-server-manifest               exit 1
  ok    pass-named-in-comment                          exit 0
  ok    pass-query-in-store                            exit 0
wire-types-in-api
  ok    clean tree                                     exit 0
  ok    fail-derive-in-core                            exit 1
  ok    fail-hand-impl-in-server                       exit 1
  ok    pass-derive-in-api                             exit 0
  ok    pass-named-in-comment                          exit 0
all 5 ratchets can fail, and no comment trips one
exit 0
```

### Each failing fixture trips its ratchet for the right reason

Exit 1 only says that *a* violation was found. This run plants each `fail-*` fixture and prints the first lines of the ratchet's own report, to show it found *the planted* violation:

```
--- core-line-budget / fail-over-budget
core-line-budget: ledger-core has 900 implementation lines; the budget is 800
  Shrink the core, or raise BUDGET in scripts/ratchets/core-line-budget.sh in this same diff.
--- core-purity / fail-ambient-time
core-purity: ledger-core source reaches for I/O, ambient time or entropy:
  crates/ledger-core/src/planted.rs:2:    std::time::SystemTime::now()
--- core-purity / fail-dependency-outside-allowlist
core-purity: ledger-core has normal dependencies outside its allowlist ["ledger-api"]:
  serde_json (target cfg(target_os = "redox"))
--- core-purity / fail-network-in-source
core-purity: ledger-core source reaches for I/O, ambient time or entropy:
  crates/ledger-core/src/planted.rs:2:    std::net::TcpStream::connect("127.0.0.1:9").is_ok()
--- no-dev-paths / fail-bypass-env
no-dev-paths: code that only exists in development:
  crates/ledgerd/src/planted.rs:2:    std::env::var("LEDGER_SKIP_AUTH").is_ok()
--- no-dev-paths / fail-debug-assertions
no-dev-paths: code that only exists in development:
  crates/ledger-core/src/planted.rs:1:#[cfg(debug_assertions)]
--- no-dev-paths / fail-dev-feature
no-dev-paths: code that only exists in development:
  crates/ledgerd/src/planted.rs:1:#[cfg(feature = "dev-mode")]
--- sql-containment / fail-query-in-core
sql-containment: SQL outside ledger-store, in ledger-core:
  crates/ledger-core/src/planted.rs:1:pub const FIND_BY_KEY: &str = "SELECT id FROM jobs WHERE idempotency_key = ?1";
--- sql-containment / fail-rusqlite-in-server-manifest
sql-containment: ledgerd's manifest depends on rusqlite:
  crates/ledgerd/Cargo.toml:8:rusqlite = "0.32"
--- wire-types-in-api / fail-derive-in-core
wire-types-in-api: ledger-core serializes types; wire types belong in ledger-api:
  crates/ledger-core/src/planted.rs:1:#[derive(Debug, serde::Serialize)]
--- wire-types-in-api / fail-hand-impl-in-server
wire-types-in-api: ledgerd serializes types; wire types belong in ledger-api:
  crates/ledgerd/src/planted.rs:2:impl<'de> serde::Deserialize<'de> for Leaked {
```

### The code is the plan

Every file block in plan 01 (Tasks 1–9) was rendered into an empty directory and compared byte for byte with the committed tree: **36 files, 0 differences.**

## Inferred (not proven by anything above)

- **GitHub's runner** has not run this yet. The `ratchets` CI job assumes bash, jq, cargo and shellcheck are preinstalled on `ubuntu-latest`, which is true of the runner image as documented but not yet observed here. The first CI run is recorded in report 02.
- **Planted fixtures only.** The ratchets have been exercised against planted fixtures, not against real crate code. The first real test comes with plan 02.
- **Portability.** The scans are written against POSIX classes for portability, and have only been run with GNU grep/awk/sed (Nix) here. BSD tools on macOS have not been exercised.

## Deviations from the plan

All of these were found while executing. In each case the plan was amended before commit #1, so the plan and the code agree:

1. **Trailing-slash paths.** The `crates/*/` glob left a trailing slash, so `sql-containment` reported `crates/ledgerd//Cargo.toml`. Fixed with `crate=${crate%/}` in the two per-crate loops (`sql-containment`, `wire-types-in-api`).
2. **shellcheck SC2001 (style), found twice.** `sed` was used to indent a variable in `core-purity` and `test-ratchets`. Fixed rather than silenced: `core-purity` now uses the shared `report` helper, and `test-ratchets` uses a `read` loop.
3. **shellcheck SC2016,** anticipated during plan self-review. `$allowed` inside core-purity's jq program is a jq variable, so the finding is disabled on that one line, with the reason in the comment.
4. **Execution slip.** The coordinator's first attempt to run Tasks 4–7 in a loop used zsh word-splitting. Every task extraction errored before writing anything, and the tree was unchanged. The loop was re-run under bash; the red and green output above comes from that run.
