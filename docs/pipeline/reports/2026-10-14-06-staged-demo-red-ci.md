# Report 06: the red CI run (staged demo)

**This was staged: plan 06 instructed the worker to make this mistake.**

**Plan:** [`../plans/2026-10-14-06-staged-demo-red-ci.md`](../plans/2026-10-14-06-staged-demo-red-ci.md)
**Executed by:** a worker (Claude Code, model Claude Opus 5.5) in its own Orca worktree, on branch `ioitiki/plan-06-staged-demo`.
**Pull request:** [#7](https://github.com/ioitiki/ratchets-before-code/pull/7)
**Date:** 2026-10-14. GitHub's timestamps below are UTC, where it was already 2026-10-15.

## What the plan predicted, and what happened

| The plan predicted | What happened |
|---|---|
| The `ratchets` job fails | It failed, on the step `the ratchets hold` |
| `core-purity` reports `rusqlite` as a dependency outside the allowlist | Yes |
| `core-purity` flags the forbidden name in the source | Yes: `lib.rs:147`, the signature that names `rusqlite::Connection` |
| `sql-containment` reports the SQL in `crates/ledger-core/src/lib.rs` | Yes: `lib.rs:147`, which names `rusqlite`, and `lib.rs:149`, the `SELECT`. Its manifest check also fired, on `crates/ledger-core/Cargo.toml:12` |
| `build` and `crash-recovery` are skipped | Both were skipped |
| Not predicted | `core-line-budget` held at 241 of 250. The mistake added 7 counted lines to `main`'s 234. |
| Not predicted | The `every ratchet can fail` step did not run in the red run, because the step before it failed |

No ratchet that the plan did not name went red. After the fix, all three jobs passed.

## Measured

Every block below is verbatim tool output captured during execution. Every cargo and ratchet command ran inside the dev shell as `nix develop -c bash -o pipefail -c '…'`. `git` and `gh` ran on the host. The only edits:
- the worktree's absolute path is shortened to `…`
- Nix's "Git tree is dirty" warning lines are removed
- in the CI log, the leading byte-order mark and the `ratchets<TAB>the ratchets hold<TAB>` prefix are removed from each line, since every line has the same prefix. The `^[[36;1m` and `^[[0m` markers are printed that way by `gh`, so they are kept.
- in the `HEAD is now at` lines and the excerpts from the green `build` and `crash-recovery` jobs, each line's job name, step name and timestamp are removed, as they are in the saved logs under `target/plan06/`
- `(no output)` marks a command that printed nothing

The raw captures are under `target/` in the worktree:
- `plan06-red-run.txt`, `plan06-red-log.txt`, `plan06-red-local.txt`
- `plan06-gates.txt`, `plan06-green-run.txt`
- `plan06/`, which holds the cleaned log and the full logs of the CI jobs quoted below

### Task 1: the mistake compiles

```
$ cargo build -p ledger-core 2>&1 | tail -3
   Compiling rusqlite v0.37.0
   Compiling ledger-core v0.1.0 (…/crates/ledger-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.18s
```

The build added one line to `Cargo.lock`:

```
$ git diff Cargo.lock
diff --git a/Cargo.lock b/Cargo.lock
index 6e1b5dd..9327ac7 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -399,6 +399,7 @@ version = "0.1.0"
 dependencies = [
  "ledger-api",
  "proptest",
+ "rusqlite",
 ]
 
 [[package]]
```

No ratchet, clippy run or test ran before the push. The commit is `013dda6`:

```
$ git diff 013dda6~1 013dda6 --stat
 Cargo.lock                    | 1 +
 crates/ledger-core/Cargo.toml | 1 +
 crates/ledger-core/src/lib.rs | 9 +++++++++
 3 files changed, 11 insertions(+)
```

### The red run

**Run:** https://github.com/ioitiki/ratchets-before-code/actions/runs/38679507300, on head `013dda6`. The PR was opened as #7 with the plan's title and body. GitHub's `HEAD is now at` line shows the tree it checked out:

```
HEAD is now at e88170b Merge 013dda6d72c4a2742a2b80117ecc8355322624e4 into 37e97d5d66019131f1fa79d83144d95ecf5eb6e5
```

`37e97d5` is the `main` this branch was cut from. Plan 05's merge, `83c8379`, landed at 04:04:08Z, five seconds after this run was created, so the red run did not include it.

Task 2 Step 1's `echo` and `gh run view` (`target/plan06-red-run.txt`):

```
run: 38679507300

X ioitiki/plan-06-staged-demo ci ioitiki/ratchets-before-code#7 · 38679507300
Triggered via pull_request less than a minute ago

JOBS
X ratchets in 8s (ID 116240586013)
  ✓ Set up job
  ✓ Run actions/checkout@v4
  ✓ shellcheck the ratchets
  X the ratchets hold
  - every ratchet can fail
  ✓ Post Run actions/checkout@v4
  ✓ Complete job
- build (ID 116240637509)
- crash-recovery in 0s (ID 116240637757)

ANNOTATIONS
! Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/checkout@v4. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/
ratchets: .github#2

X Process completed with exit code 1.
ratchets: .github#20

- "The ubuntu-latest label will migrate to Ubuntu 26 beginning October 19, 2026. For more information, see https://github.com/actions/runner-images/issues/14748"
ratchets: .github#1


To see what failed, try: gh run view 38679507300 --log-failed
View this run on GitHub: https://github.com/ioitiki/ratchets-before-code/actions/runs/38679507300
```

The failing step's log (`gh run view 38679507300 --log-failed`, cleaned as described above):

```
2026-10-15T04:04:12.0606142Z ##[group]Run ./scripts/ratchets/all.sh
2026-10-15T04:04:12.0606582Z ^[[36;1m./scripts/ratchets/all.sh^[[0m
2026-10-15T04:04:12.0666680Z shell: /usr/bin/bash -e {0}
2026-10-15T04:04:12.0666961Z env:
2026-10-15T04:04:12.0667177Z   RUSTUP_TOOLCHAIN: stable
2026-10-15T04:04:12.0667439Z ##[endgroup]
2026-10-15T04:04:12.0992226Z core-line-budget: ok (241 of 250 lines)
2026-10-15T04:04:13.0227471Z core-purity: ledger-core has normal dependencies outside its allowlist ["ledger-api"]:
2026-10-15T04:04:13.0240544Z   rusqlite
2026-10-15T04:04:13.0309889Z core-purity: ledger-core source reaches for I/O, ambient time or entropy:
2026-10-15T04:04:13.0330656Z   crates/ledger-core/src/lib.rs:147:pub fn submitted_in_journal(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<bool> {
2026-10-15T04:04:13.0476514Z no-dev-paths: ok
2026-10-15T04:04:13.0709626Z sql-containment: SQL outside ledger-store, in ledger-core:
2026-10-15T04:04:13.0725641Z   crates/ledger-core/src/lib.rs:147:pub fn submitted_in_journal(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<bool> {
2026-10-15T04:04:13.0727727Z   crates/ledger-core/src/lib.rs:149:        "SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(body, '$.Submitted.idempotency_key') = ?1)",
2026-10-15T04:04:13.0762654Z sql-containment: ledger-core's manifest depends on rusqlite:
2026-10-15T04:04:13.0775748Z   crates/ledger-core/Cargo.toml:12:rusqlite.workspace = true
2026-10-15T04:04:13.1185943Z wire-types-in-api: ok
2026-10-15T04:04:13.1188816Z ratchets failed: core-purity sql-containment
2026-10-15T04:04:13.1203648Z ##[error]Process completed with exit code 1.
```

### The local reproduction

```
$ ./scripts/ratchets/all.sh; echo "exit $?"
core-line-budget: ok (241 of 250 lines)
core-purity: ledger-core has normal dependencies outside its allowlist ["ledger-api"]:
  rusqlite
core-purity: ledger-core source reaches for I/O, ambient time or entropy:
  crates/ledger-core/src/lib.rs:147:pub fn submitted_in_journal(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<bool> {
no-dev-paths: ok
sql-containment: SQL outside ledger-store, in ledger-core:
  crates/ledger-core/src/lib.rs:147:pub fn submitted_in_journal(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<bool> {
  crates/ledger-core/src/lib.rs:149:        "SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(body, '$.Submitted.idempotency_key') = ?1)",
sql-containment: ledger-core's manifest depends on rusqlite:
  crates/ledger-core/Cargo.toml:12:rusqlite.workspace = true
wire-types-in-api: ok
ratchets failed: core-purity sql-containment
exit 1
```

The output matches CI's line for line, and the exit code is 1.

### Task 3: the fix

The lookup and the dependency were removed. Then:

```
$ git diff main -- crates/ledger-core Cargo.lock
(no output)
```

Plan 05's merge touched neither path:

```
$ git diff --stat 37e97d5 83c8379 -- crates/ledger-core Cargo.lock
(no output)
```

The new test, with the rest of the `http` suite:

```
$ cargo test -p ledgerd --test http 2>&1 | grep -E "^(test |test result)"
test an_unknown_job_is_not_found_and_zero_attempts_is_refused ... ok
test a_duplicate_submission_returns_the_same_job ... ok
test a_duplicate_submitted_after_a_restart_returns_the_same_job ... ok
test a_submitted_job_runs_to_success ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
```

### The gates, in CI's order (Task 3 Step 4)

This is the plan's script, run as written and captured to `target/plan06-gates.txt`. `cargo fmt` changed nothing, so the commit has no formatting changes. The file's contents:

```
$ ./scripts/ratchets/all.sh
core-line-budget: ok (234 of 250 lines)
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
    Checking axum v0.8.9
    Checking ledgerd v0.1.0 (…/crates/ledgerd)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.79s
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
test nothing_due_means_nothing_leased ... ok
test an_unknown_job_is_refused ... ok
test zero_attempts_is_refused ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test attempts_never_exceed_max_attempts ... ok
test replaying_the_events_rebuilds_the_state ... ok
test a_stale_lease_changes_nothing ... ok
test a_job_succeeds_at_most_once ... ok
test the_same_commands_give_the_same_events ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
running 6 tests
test a_finished_effect_completes_without_running_again ... ok
test a_lease_whose_effect_never_started_is_retried_whatever_its_kind ... ok
test an_unfinished_idempotent_effect_is_re_driven ... ok
test jobs_that_are_not_leased_are_left_alone ... ok
test an_unfinished_non_idempotent_effect_is_killed_not_repeated ... ok
test the_last_attempt_dies_instead_of_retrying ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test an_outcome_without_an_intent_is_refused ... ok
test an_attempt_records_its_intent_once ... ok
test an_intent_without_an_outcome_is_unfinished ... ok
test an_outcome_finishes_its_intent ... ok
test unfinished_effects_survive_a_reopen_oldest_first ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
running 4 tests
test an_empty_journal_replays_nothing ... ok
test events_replay_in_append_order ... ok
test the_journal_survives_a_reopen ... ok
test any_batching_replays_the_same_sequence ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test a_job_killed_mid_effect_completes_exactly_once_after_restart ... ignored, spawns and kills the real server; run by CI's crash-recovery job
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 4 tests
test an_unknown_job_is_not_found_and_zero_attempts_is_refused ... ok
test a_duplicate_submission_returns_the_same_job ... ok
test a_duplicate_submitted_after_a_restart_returns_the_same_job ... ok
test a_submitted_job_runs_to_success ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0
```

### The green run

**Run:** https://github.com/ioitiki/ratchets-before-code/actions/runs/38679642829, on head `80ec63e`. By then `main` had moved, so this run tested the fix merged with plan 05:

```
HEAD is now at b7e98b9 Merge 80ec63e29c2be3685d1d824ac31401ef2d799981 into 83c83795b186dfed1182a30ce4482e9072d27270
```

Task 3 Step 6's `echo` and `gh run view` (`target/plan06-green-run.txt`):

```
run: 38679642829

✓ ioitiki/plan-06-staged-demo ci ioitiki/ratchets-before-code#7 · 38679642829
Triggered via pull_request about 2 minutes ago

JOBS
✓ ratchets in 12s (ID 116241013184)
✓ build in 1m17s (ID 116241082256)
✓ crash-recovery in 42s (ID 116241450289)

ANNOTATIONS
! Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/checkout@v4. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/
ratchets: .github#2

- "The ubuntu-latest label will migrate to Ubuntu 26 beginning October 19, 2026. For more information, see https://github.com/actions/runner-images/issues/14748"
ratchets: .github#1

! Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/checkout@v4. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/
build: .github#2

- "The ubuntu-latest label will migrate to Ubuntu 26 beginning October 19, 2026. For more information, see https://github.com/actions/runner-images/issues/14748"
build: .github#1

! Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/checkout@v4. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/
crash-recovery: .github#2

- "The ubuntu-latest label will migrate to Ubuntu 26 beginning October 19, 2026. For more information, see https://github.com/actions/runner-images/issues/14748"
crash-recovery: .github#1


For more information about a job, try: gh run view --job=<job-id>
View this run on GitHub: https://github.com/ioitiki/ratchets-before-code/actions/runs/38679642829
```

In the `build` job's `cargo test --workspace` log, the new test ran beside plan 05's journal lock and passed (`target/plan06/green-build-log.txt`, lines 532 to 539):

```
running 4 tests
test a_duplicate_submission_returns_the_same_job ... ok
test a_duplicate_submitted_after_a_restart_returns_the_same_job ... ok
test an_unknown_job_is_not_found_and_zero_attempts_is_refused ... ok
ledgerd: notify: hello
test a_submitted_job_runs_to_success ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

The `crash-recovery` job (`target/plan06/green-crash-log.txt`, lines 291 to 297):

```
running 1 test
killed mid-effect: signal: 9 (SIGKILL)
ledgerd: recovered 1 leased job(s) after a restart
succeeded 3.033785276s after the kill: {"effect":{"Finished":{"attempt":2}},"job":{"attempts":2,"id":0,"idempotency_key":"crash","kind":{"SleepThenRecord":{"ms":2000,"token":"crash-test"}},"max_attempts":3,"state":{"Succeeded":{"result":"crash-test"}}}}
test a_job_killed_mid_effect_completes_exactly_once_after_restart ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.17s
```

### What the branch leaves behind

The two code commits cancel out. Against the `main` this branch was cut from, the net diff before this report was written:

```
$ git diff --stat 37e97d5 HEAD
 crates/ledgerd/tests/http.rs                       |  21 ++
 .../plans/2026-10-14-06-staged-demo-red-ci.md      | 237 +++++++++++++++++++++
 2 files changed, 258 insertions(+)

$ git diff --stat $(git merge-base HEAD main) HEAD -- scripts .github
(no output)
```

The plan file was already on the branch when this worker started. No ratchet, fixture, allowlist or workflow changed.

### Commits

```
$ git config user.email
26385844+ioitiki@users.noreply.github.com

$ git log --reverse --format='%h %an <%ae>%n    %s%n    %(trailers:key=Co-Authored-By)' main..HEAD
2fba4ed Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    Plan 06: the red CI run (staged demo)
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

013dda6 Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    ledger-core: check the journal for a duplicate idempotency key [staged demo]
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

80ec63e Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    Remove the journal lookup from ledger-core; test duplicates across a restart
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
```

This report is committed after the log above, with the same author and trailer. Nothing was squashed, rebased or force-pushed, so `013dda6`, the red commit, stays in the branch's history.

## Inferred (not proven by anything above)

These come from reading `scripts/ratchets/core-purity.sh`, `sql-containment.sh` and `lib.sh`. None of them was run.

**What the ratchets would catch if an agent made this mistake unprompted:**
- **Any lookup inside `ledger-core` that needs a crate.** `core-purity` reads the core's normal dependencies from `cargo metadata` and compares them with an allowlist of one, `ledger-api`. A database driver, a helper crate or a renamed dependency fails it, however the SQL is spelled. That makes it the strongest of the checks seen in this run: it fails on what the crate links, not on what its source text looks like.
- **A helper crate under `crates/`.** `sql-containment` scans every directory under `crates/` except `ledger-store`. A new `crates/ledger-dedupe` that names `rusqlite` in its manifest or source, or holds a one-line `SELECT … FROM`, fails it. If the core depended on that crate, `core-purity` would fail as well.

**What they would not catch:**
- **The same check in a place the ratchets allow.** Suppose an agent adds a `has_key` method to `ledger-store` and calls it from `ledgerd` before `submit`, or passes it into the core as a closure, the way `recover` receives `effect`. That breaks no ratchet. It duplicates the core's own check, which is the real mistake here, and only review would catch it. Of all the misses, this is the likeliest one for an agent working unprompted.
- **A crate outside `crates/`.** `sql-containment` only loops over `crates/*/`. A path dependency at, say, `tools/dedupe`, holding `rusqlite` and SQL, passes if `ledgerd` depends on it rather than the core.
- **SQL spread over lines or built from pieces.** The source scan works line by line and ignores case. The statement pattern needs `select … from` on one line. A raw string with `SELECT` and `FROM` on separate lines does not match. Neither does `concat!("SEL", "ECT 1 FROM events")` or a `format!` that joins the pieces. In this run those lines would still have failed, because `rusqlite` appears by name. Without that name they would not.
- **Another driver outside the core.** The manifest check and the name check know only `rusqlite`. `sqlx`, `libsql` or `diesel` in `ledgerd`, with multi-line SQL, would pass both. In the core, the allowlist catches them.
- **I/O in the core with no dependency.** `core-purity`'s source pattern is `std::(net|fs|process|env)`, so `use std::{fs, net};` does not match it, and neither does a later bare `fs::read`. Reading the journal file with `std` alone is far-fetched as a duplicate check, but it is the gap.

**What this demonstration does not prove:**
- **That an agent would make this mistake on its own, or how often.** Plans 02 to 05 produced no natural catch, so the record has none.
- **Coverage.** The mistake was chosen from `core-purity.sh`'s own header comment, so it is the case the ratchet was written for. A catch here shows the ratchets work on their design case, not how much they cover.
- **That a caught agent repairs the right thing.** The fix and the test were dictated by the plan, too. This run does not show an agent reading a red CI log and choosing to remove the lookup, rather than, say, moving it into `ledgerd`, which would pass.
- **That CI is usually the first line.** The plan told this worker to skip the local ratchets. Reports 02 to 05 each quote `all.sh` run locally before the PR, so a mistake like this would normally fail on the worker's machine first. CI is the backstop that this run exercised.
- **That the new test can fail.** It passed the first time it ran, locally and in CI, and nobody has seen it fail. Breaking replay should make the second `POST` return 201, but that was not run. It covers one sequential restart: the test harness's `Drop` for `Server` calls `child.kill()`, which is `SIGKILL`. It does not cover concurrent duplicate submissions.
- **That the local gates cover the merged tree.** They ran on this branch, which does not contain plan 05. The only run of the new test with plan 05's lock is the green CI run on `b7e98b9`.

## Deviations from the plan

1. **The PR body ends with the line `🤖 Generated with [Claude Code](https://claude.com/claude-code)`**, after the plan's text, as PRs #4 and #5 did. The title and the plan's text are unchanged, and the text still opens with "**This is a staged demonstration.**"
2. **How the mistake was removed (Task 3 Step 1).**
   - `sed` deleted the `rusqlite` line from the manifest.
   - A `python3` heredoc removed the function from the end of `lib.rs`, after asserting that the block appeared exactly once and ended the file.
   - Both ran on the host.
   - I then ran `cargo metadata` once in the dev shell to bring `Cargo.lock` back in line, before the plan's `git diff main` check.
3. **Waiting for the green run (Task 3 Step 6).** I ran `sleep 5` before `gh pr checks --watch`, so that it would watch the new run rather than the finished red one. Then I checked the run's head SHA (`80ec63e`, `success`).
4. **Checks the plan did not ask for.** None changed committed code:
   - the `HEAD is now at` lines from the CI job logs
   - the full logs of the red `ratchets` job and the green `build` and `crash-recovery` jobs, saved under `target/plan06/`
   - the `git diff --stat` comparisons with `37e97d5` and `83c8379`
5. **`main` moved during execution.** Plan 05 (#6) merged into `main` while Task 2 was running. I did not merge `main` into this branch: the plan does not ask for it, and the PR reports as mergeable. As a result, `git diff main --stat` on this branch shows plan 05's files as deletions, and the plan's "the net diff against `main`" holds against `37e97d5`, the merge base, as quoted above.
6. **Expected output that matched, noted for completeness:**
   - Task 1 built.
   - The red run failed on exactly the two ratchets the plan named.
   - `build` and `crash-recovery` were skipped.
   - The local run matched CI and exited 1.
   - `git diff main -- crates/ledger-core Cargo.lock` printed nothing.
   - The `http` suite has 4 tests, all passing.
   - Every gate was green.
   - The green run passed all three jobs.

**Clean-room:** every file this worker wrote is inside this worktree, and every capture and helper is under `target/`. The tools went outside the worktree in their usual way:
- Nix used its store, and cargo used its registry cache.
- `test-ratchets.sh` made scratch copies with `mktemp -d`. `tempfile::tempdir()` made the test databases under the system temp directory.
- The harness loaded the executing-plans skill from its plugin cache.
- `gh` read and wrote this repository's PR and its Actions runs.
