# Report 03: ledger-store, the journal

**Plan:** [`../plans/2026-10-14-03-ledger-store.md`](../plans/2026-10-14-03-ledger-store.md)
**Executed by:** a worker (Claude Code, model Claude Opus 5.5) in its own Orca worktree, on branch `ioitiki/plan-03-ledger-store`.
**Date:** 2026-10-14

## Measured

Every block below is verbatim tool output captured during execution. Every cargo, rustfmt and ratchet command ran inside the dev shell as `nix develop -c bash -o pipefail -c '…'`. `gh`, `git` and one `python3` helper ran outside it (deviation 2). The only edits:
- the worktree's absolute path is shortened to `…`
- Nix's "Git tree is dirty" warning lines are removed
- in the per-task excerpts, `[…]` marks lines left out; every line shown is verbatim

### The dev shell

```
$ cargo --version; rustc --version; rustfmt --version; cargo clippy --version; cc --version | head -1; command -v cc
cargo 1.98.0 (797e8a9bc 2026-08-05)
rustc 1.98.1 (48a229cea 2026-09-01) (built from a source tarball)
rustfmt 1.9.0
clippy 0.1.98
gcc (GCC) 16.2.0
/nix/store/j1938gsbvbhkyz4kbmvjk56r479mkw3s-gcc-wrapper-16.2.0/bin/cc
```

`rusqlite`'s `bundled` feature compiles SQLite's C source, so it needs a C compiler. `flake.nix` does not list one; `mkShell`'s standard environment supplies the `gcc` wrapper above.

### Dependencies

`rusqlite = "0.37"` resolved on the first try, so `cargo search` was not needed:

```
$ grep -A1 -E 'name = "(rusqlite|libsqlite3-sys|tempfile)"' Cargo.lock
name = "libsqlite3-sys"
version = "0.35.0"
--
name = "rusqlite"
version = "0.37.0"
--
name = "tempfile"
version = "3.27.0"
```

### Red, then green, task by task

**Task 1**, the journal tests before `crates/ledger-store/src/lib.rs` existed. Cargo locked 13 new packages and compiled `libsqlite3-sys` and `rusqlite` in this run, before the test target failed:

```
$ cargo test -p ledger-store --test journal; echo "exit=$?"
[…]
     Locking 13 packages to latest Rust 1.98 compatible versions
[…]
   Compiling libsqlite3-sys v0.35.0
[…]
   Compiling rusqlite v0.37.0
[…]
error[E0432]: unresolved import `ledger_store`
 --> crates/ledger-store/tests/journal.rs:4:5
  |
4 | use ledger_store::Store;
  |     ^^^^^^^^^^^^ use of unresolved module or unlinked crate `ledger_store`
  |
  = help: if you wanted to use a crate named `ledger_store`, use `cargo add ledger_store` to add it to your `Cargo.toml`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `ledger-store` (test "journal") due to 1 previous error
exit=101
```

**Task 2**, after writing `schema.sql` and `lib.rs`, with no `^warning` line other than Nix's:

```
$ cargo test -p ledger-store --test journal; echo "exit=$?"
[…]
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.35s
     Running tests/journal.rs (target/debug/deps/journal-765c9796eef9910e)

running 4 tests
test an_empty_journal_replays_nothing ... ok
test events_replay_in_append_order ... ok
test the_journal_survives_a_reopen ... ok
test any_batching_replays_the_same_sequence ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

exit=0
```

The ratchets with real SQL in the tree for the first time (Task 2 Step 3):

```
$ ./scripts/ratchets/all.sh; echo "exit=$?"
core-line-budget: ok (201 of 250 lines)
core-purity: ok
no-dev-paths: ok
sql-containment: ok
wire-types-in-api: ok
all 5 ratchets hold
exit=0
```

**Task 3**, the effect tests. The behavior already existed, so this run proves it; it is not a red/green cycle:

```
$ cargo test -p ledger-store --test effects; echo "exit=$?"
[…]
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.83s
     Running tests/effects.rs (target/debug/deps/effects-58208abc836d2da7)

running 5 tests
test an_outcome_without_an_intent_is_refused ... ok
test an_attempt_records_its_intent_once ... ok
test an_outcome_finishes_its_intent ... ok
test an_intent_without_an_outcome_is_unfinished ... ok
test unfinished_effects_survive_a_reopen_oldest_first ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

exit=0
```

### The gates, in CI's order (Task 5 Step 1)

This is the plan's script, run as written under `bash -o pipefail` and captured to `target/plan03-gates.txt`. The file's contents, verbatim:

```
$ ./scripts/ratchets/all.sh
core-line-budget: ok (201 of 250 lines)
core-purity: ok
no-dev-paths: ok
sql-containment: ok
wire-types-in-api: ok
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

$ cargo fmt --all -- --check
(formatted)
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
    Checking ledger-store v0.1.0 (…/crates/ledger-store)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
exit 0

$ cargo test --workspace
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 3 tests
test the_demo_job_declares_itself_idempotent ... ok
test commands_and_refusals_round_trip ... ok
test events_round_trip ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test backoff::tests::doubles_then_caps ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 7 tests
test a_duplicate_submission_records_nothing ... ok
test a_completed_job_records_its_result ... ok
test a_failure_retries_with_backoff_until_attempts_run_out ... ok
test an_expired_lease_is_retried_and_its_holder_is_refused ... ok
test an_unknown_job_is_refused ... ok
test nothing_due_means_nothing_leased ... ok
test zero_attempts_is_refused ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test attempts_never_exceed_max_attempts ... ok
test a_job_succeeds_at_most_once ... ok
test replaying_the_events_rebuilds_the_state ... ok
test the_same_commands_give_the_same_events ... ok
test a_stale_lease_changes_nothing ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test an_intent_without_an_outcome_is_unfinished ... ok
test an_attempt_records_its_intent_once ... ok
test an_outcome_finishes_its_intent ... ok
test an_outcome_without_an_intent_is_refused ... ok
test unfinished_effects_survive_a_reopen_oldest_first ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
running 4 tests
test an_empty_journal_replays_nothing ... ok
test events_replay_in_append_order ... ok
test the_journal_survives_a_reopen ... ok
test any_batching_replays_the_same_sequence ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0
```

The 9 new tests are the last two non-empty groups: 5 in `effects` and 4 in `journal`. The core is unchanged at 201 lines against the `BUDGET` of 250 that ADR 0004 set.

### The code is the plan

I extracted the plan's six `<!-- file: … -->` blocks into `target/plan03/render` with a small script (`target/plan03/render.py`), ran `rustfmt --edition 2024` on the three Rust files, and compared each file with the committed tree:

```
identical  crates/ledger-store/Cargo.toml
identical  crates/ledger-store/src/lib.rs
identical  crates/ledger-store/src/schema.sql
identical  crates/ledger-store/tests/effects.rs
identical  crates/ledger-store/tests/journal.rs
identical  docs/decisions/0003-the-journal-records-intent-before-outcome.md
```

The root `Cargo.toml` change is the plan's two lines and nothing else:

```
$ git diff main --stat -- Cargo.toml
 Cargo.toml | 2 ++
 1 file changed, 2 insertions(+)
```

`cargo fmt` changed only layout, as it did in plan 02. For example, it expands `NoSuchIntent { job: JobId, attempt: u32 }` across four lines. I also ran `cargo clippy --workspace --all-targets -- -D warnings` before the Task 2 commit, because plan 02's code had failed that gate. It printed `clippy exit=0`.

### The first CI run on real crates: PR #1

```
$ gh run view 38654361590 --json … --jq …    # the run, then each job and its steps
run 38654361590 on ioitiki/plan-02-ledger-core @ 4189d56 (pull_request): completed/success, created 2026-10-14T23:29:11Z
  title: Plan 02: ledger-api and ledger-core
  job ratchets: success
    1. Set up job: success
    2. Run actions/checkout@v4: success
    3. shellcheck the ratchets: success
    4. the ratchets hold: success
    5. every ratchet can fail: success
    10. Post Run actions/checkout@v4: success
    11. Complete job: success
  job build: success
    1. Set up job: success
    2. Run actions/checkout@v4: success
    3. Run if [ -f Cargo.toml ]; then: success
    4. install the toolchain pinned in rust-toolchain.toml: success
    5. Run cargo fmt --all -- --check: success
    6. Run cargo clippy --workspace --all-targets -- -D warnings: success
    7. Run cargo test --workspace: success
    14. Post Run actions/checkout@v4: success
    15. Complete job: success
  job crash-recovery: success
    1. Set up job: success
    2. Run actions/checkout@v4: success
    3. Run if [ -f crates/ledgerd/tests/crash_recovery.rs ]; then: success
    4. install the toolchain pinned in rust-toolchain.toml: skipped
    5. Run cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture: skipped
    10. Post Run actions/checkout@v4: success
    11. Complete job: success
```

Lines from that run's log (`gh run view 38654361590 --log`), shown as job | step | line with timestamps removed:

```
ratchets | the ratchets hold |   RUSTUP_TOOLCHAIN: stable
ratchets | the ratchets hold | core-line-budget: ok (201 of 800 lines)
ratchets | the ratchets hold | all 5 ratchets hold
ratchets | every ratchet can fail | all 5 ratchets can fail, and no comment trips one
build | install the toolchain pinned in rust-toolchain.toml | info: syncing channel updates for 1.98.1-x86_64-unknown-linux-gnu
build | install the toolchain pinned in rust-toolchain.toml | info: latest update on 2026-09-03 for version 1.98.1 (48a229cea 2026-09-01)
build | install the toolchain pinned in rust-toolchain.toml | info: downloading 5 components
build | install the toolchain pinned in rust-toolchain.toml |   1.98.1-x86_64-unknown-linux-gnu installed - rustc 1.98.1 (48a229cea 2026-09-01)
build | Run cargo test --workspace | test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
build | Run cargo test --workspace | test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
build | Run cargo test --workspace | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
build | Run cargo test --workspace | test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
build | Run cargo test --workspace | test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.09s
build | Run cargo test --workspace | test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
build | Run cargo test --workspace | test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Each line from the `every ratchet can fail` step of that run, for `core-purity`:

```
core-purity
  ok    clean tree                                     exit 0
  ok    fail-ambient-time                              exit 1
  ok    fail-dependency-outside-allowlist              exit 1
  ok    fail-network-in-source                         exit 1
  ok    pass-dev-dependency                            exit 0
  ok    pass-named-in-comment                          exit 0
```

This run settles three items that report 02 listed as inferred:
- **rustup installed the pinned 1.98.1 on `ubuntu-latest`.** It reports the same commit hash, `48a229cea`, as the Nix-built rustc in the dev shell.
- **The `ratchets` job read an edition-2024 workspace with stable cargo.** It ran with `RUSTUP_TOOLCHAIN: stable`, and `the ratchets hold` printed `all 5 ratchets hold`. One of those five is `core-purity`, which exits 2, not 0, when `cargo metadata` fails on the workspace (`scripts/ratchets/core-purity.sh`). Its `fail-dependency-outside-allowlist` fixture also exited 1, which likewise requires `cargo metadata` to have run.
- **The edited workflow parsed.** All three jobs ran with the steps report 02 wrote.

The ratchets job printed `201 of 800 lines` because PR #2 (ADR 0004) had not yet lowered `BUDGET` to 250.

### Commits and the ratchets

```
$ git config user.email
26385844+ioitiki@users.noreply.github.com

$ git log --reverse --format='%h %an <%ae>%n    %s%n    %(trailers:key=Co-Authored-By)' 7ff0a27..HEAD
5da735a Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    ledger-store: the event journal, with replay across reopen
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

4723901 Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    ledger-store: effect intents and outcomes, across reopen
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

ce90de4 Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    ADR 0003: the journal records intent before outcome
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

$ git diff --stat main -- scripts/
(no output)
```

`7ff0a27`, the plan itself, was already on the branch when this worker started. This report is committed after the log above, with the same author and trailer. Nothing under `scripts/ratchets/` changed, including `BUDGET`. No ratchet blocked this plan.

## Inferred (not proven by anything above)

- **Durability under a real `kill -9`.** `the_journal_survives_a_reopen` and `unfinished_effects_survive_a_reopen_oldest_first` drop the `Store` cleanly and open the file again. SQLite closes its connection and checkpoints on that drop. Nothing here kills a process mid-transaction, or between an intent and its outcome. That is plan 04's crash-recovery test.
- **`synchronous = FULL` against power loss.** Nothing here exercises an OS crash or a power cut, and nothing checks that the pragmas took effect. `open` sets `journal_mode = WAL` and `synchronous = FULL` but does not read either back, and no test inspects them. On some filesystems (tmpfs, for example), SQLite may not be able to use WAL; that was not checked for the directories `tempfile` chose.
- **Atomicity of a failed append.** `append` runs in one transaction, so a batch is all or nothing by SQLite's guarantee. No test makes an append fail partway and then checks that nothing was recorded.
- **The `i64` boundary.** `to_sql` and `from_sql` refuse values above `i64::MAX` instead of wrapping. No test feeds them such a value.
- **The tests' power to fail.** Task 3's tests went straight to green, by the plan's design, and the store was not mutated to show that each test catches a broken store. The journal tests failed only because the crate did not exist yet; that does not show that they detect wrong behavior.
- **Property coverage.** `any_batching_replays_the_same_sequence` runs proptest's default number of cases, and proptest prints no case count when a run passes. The generator uses three of the five `Event` variants (`Succeeded`, `Retrying`, `Died`). `Submitted` and `Leased` round-trip only through the example tests, and `JobKind` only through `sample()`.
- **This branch's CI run.** The report is committed before the push. `bundled` needs a C compiler on `ubuntu-latest`, which the runner image normally provides, but this branch's `build` job has not been observed.

## Deviations from the plan

1. **The Task 4 commit message is my wording.** The plan says to commit ADR 0003 "with a trailer" and gives no subject. I used `ADR 0003: the journal records intent before outcome`.
2. **Not every command ran inside the dev shell.** As in plan 02, `git` and `gh` ran on the host. The `python3` script that extracted the plan's file blocks (see "The code is the plan") also ran on the host, with `-I`. Every cargo, rustfmt and ratchet command ran in `nix develop`.
3. **One helper file was written outside the worktree.** I wrote that extraction script first in the session's scratchpad directory, outside this worktree, and ran it from there. It read only this worktree's plan file and wrote only under `target/plan03/render`. A copy is at `target/plan03/render.py`. Nothing outside this repository was read.
4. **Expected output that matched, noted for completeness:**
   - Task 1 failed on `unresolved import ledger_store`, exactly as the plan expected.
   - The plan's code passed every gate as written. Unlike plan 02, no fix was needed.

**Clean-room:** apart from deviation 3, every file this worker read or wrote is inside this worktree. The tools went outside it in their usual way:
- Nix used its store, and cargo used the crates.io index and its registry cache.
- `test-ratchets.sh` made scratch copies with `mktemp -d`, and `tempfile::tempdir()` made the reopen tests' database directories under the system temp directory.
- `gh` read this repository's Actions run.
