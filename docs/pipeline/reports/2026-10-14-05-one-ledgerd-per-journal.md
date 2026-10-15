# Report 05: one `ledgerd` per journal

**Plan:** [`../plans/2026-10-14-05-one-ledgerd-per-journal.md`](../plans/2026-10-14-05-one-ledgerd-per-journal.md)
**Decision:** [ADR 0005](../../decisions/0005-one-ledgerd-per-journal.md). It builds finding 1 of [report 05](2026-10-14-05-do-leases-need-a-distributed-lock.md), and so closes finding 2.
**Executed by:** a worker (Claude Code, model Claude Opus 5.5) in its own Orca worktree, on branch `ioitiki/plan-05-journal-lock`.
**Date:** 2026-10-14

This is plan 05's live proof. Below, "report 05" means brief 05's report, as in the plan and the ADR.

**Result:**
- **The goal holds when A and B use the same path.** A second `ledgerd` started on that path while A's effect runs exits with status 1 and `JournalInUse`, and A's job ends with 1 lease, 1 intent, 1 outcome and 1 `Succeeded` event. Brief 05's harness measured that in 4 runs out of 4. On the same harness and the same steps, brief 05 measured 2 of each in 3 runs out of 3.
- **Every gate holds, and so does the crash test.** The plan's code needed `cargo fmt` first (deviation 1).
- **A symlink to the journal file gets past the lock.** I measured this as a check the plan didn't ask for. The lock's name is built from the path string, so a second process given a symlink to the journal starts on it. See finding 1 below.

## Measured

Every block below is verbatim tool output captured during execution. Every cargo, rustfmt, shellcheck, sqlite3 and ratchet command ran inside the dev shell as `nix develop -c bash -o pipefail -c '…'`, and so did brief 05's harness and the `awk` that extracted the plan's file blocks. Commands outside the dev shell are listed in deviation 3. The only edits to the output:
- the worktree's absolute path is shortened to `…`
- Nix's "Git tree is dirty" warning lines are removed
- ANSI color codes are removed from `cargo fmt`'s diff
- in excerpts, `[…]` marks lines left out; every line shown is verbatim

The raw captures are under `target/` in the worktree:
- `plan05-red.txt`, `plan05-green.txt`
- `plan05-gates-first.txt`, `plan05-gates.txt`, `plan05-shellcheck.txt`
- `plan05-case3.txt`, `plan05-alias.txt`
- `plan05-tools-render.txt`, `plan05-render.txt`, `plan05-commits.txt`
- `plan05/`, which holds the render, the alias script and this report's draft

### The dev shell

```
$ cargo --version; rustc --version; rustfmt --version; cargo clippy --version; sqlite3 --version; shellcheck --version | sed -n 2p
cargo 1.98.0 (797e8a9bc 2026-08-05)
rustc 1.98.1 (48a229cea 2026-09-01) (built from a source tarball)
rustfmt 1.9.0
clippy 0.1.98
3.53.3 2026-06-26 20:14:12 d4c0e51e4aeb96955b99185ab9cde75c339e2c29c3f3f12428d364a10d78alt1 (64-bit)
version: 0.11.0
```

### Red, then green

**Task 1, Step 2:** the store's new tests, before the change. The first part of the output is cargo compiling dependencies:

```
$ nix develop -c bash -o pipefail -c 'cargo test -p ledger-store --test lock 2>&1 | tail -20'
[…]
   Compiling ledger-store v0.1.0 (…/crates/ledger-store)
error[E0599]: no variant named `JournalInUse` found for enum `StoreError`
  --> crates/ledger-store/tests/lock.rs:11:25
   |
11 |         Err(StoreError::JournalInUse { lock }) => {
   |                         ^^^^^^^^^^^^ variant not found in `StoreError`

For more information about this error, try `rustc --explain E0599`.
error: could not compile `ledger-store` (test "lock") due to 1 previous error
```

**Task 1, Step 4:** after the change, before `cargo fmt`. The 3 new tests pass, and so do the 9 existing store tests:

```
$ nix develop -c bash -o pipefail -c 'cargo test -p ledger-store 2>&1 | grep -E "^(test |test result)"'
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test an_attempt_records_its_intent_once ... ok
test an_intent_without_an_outcome_is_unfinished ... ok
test an_outcome_finishes_its_intent ... ok
test an_outcome_without_an_intent_is_refused ... ok
test unfinished_effects_survive_a_reopen_oldest_first ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test events_replay_in_append_order ... ok
test an_empty_journal_replays_nothing ... ok
test the_journal_survives_a_reopen ... ok
test any_batching_replays_the_same_sequence ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test in_memory_journals_do_not_contend ... ok
test the_journal_opens_again_once_its_holder_is_dropped ... ok
test a_journal_already_open_is_refused ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Task 2, Step 2: the red run.** The new `ledgerd` test, copied into a detached worktree of `origin/main` at `37e97d5`, before the lock existed. The worktree was built from scratch under `target/plan05-before` and removed afterwards, as the plan says:

```
$ git worktree add --detach target/plan05-before origin/main
$ cp crates/ledgerd/tests/one_per_journal.rs target/plan05-before/crates/ledgerd/tests/
$ nix develop -c bash -o pipefail -c 'cd target/plan05-before && cargo test -p ledgerd --test one_per_journal -- --nocapture 2>&1 | tail -15' | tee target/plan05-red.txt
running 1 test

thread 'a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing' (803573) panicked at crates/ledgerd/tests/one_per_journal.rs:44:13:
B is still running: it started on a journal that was already in use
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing ... FAILED

failures:

failures:
    a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.16s

error: test failed, to rerun pass `-p ledgerd --test one_per_journal`
$ git worktree remove --force target/plan05-before
```

B was still running 10 s after it started, on a journal A was using. The test failed with the message the plan expected.

**Task 2, Step 3: the green run,** on this branch, before `cargo fmt`:

```
$ nix develop -c bash -o pipefail -c 'cargo test -p ledgerd --test one_per_journal -- --nocapture 2>&1 | tail -15'
   Compiling hyper v1.12.0
   Compiling tower v0.5.3
   Compiling hyper-util v0.1.21
   Compiling axum v0.8.9
   Compiling ledgerd v0.1.0 (…/crates/ledgerd)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 12.11s
     Running tests/one_per_journal.rs (target/debug/deps/one_per_journal-89981ef6ac2357ce)

running 1 test
B exited with exit status: 1; stderr: Error: Store(JournalInUse { lock: "/tmp/nix-shell.r5dGMJ/.tmpR8e23e/ledger.db.lock" })

test a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.17s

```

B exited with status 1 and named `JournalInUse`. A's job then succeeded on attempt 1, with `effect Finished(1)`.

### The gates, in CI's order, plus the crash test (Task 3)

**First run: `cargo fmt` failed.** I ran Task 3's command exactly as written. Every other gate in it passed, as `exit 0` after each one shows in `target/plan05-gates-first.txt`. The `cargo fmt` section:

```
$ cargo fmt --all -- --check
Diff in …/crates/ledger-store/src/lib.rs:32:
         attempt: u32,
     },
     /// Another holder has the journal's lock. One `ledgerd` per journal (ADR 0005).
-    JournalInUse { lock: PathBuf },
+    JournalInUse {
+        lock: PathBuf,
+    },
     /// The journal's lock file could not be opened or locked.
     Lock(std::io::Error),
 }
Diff in …/crates/ledgerd/tests/one_per_journal.rs:47:
     };
     let mut stdout = String::new();
     let mut stderr = String::new();
-    b.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
-    b.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
+    b.stdout
+        .take()
+        .unwrap()
+        .read_to_string(&mut stdout)
+        .unwrap();
+    b.stderr
+        .take()
+        .unwrap()
+        .read_to_string(&mut stderr)
+        .unwrap();
     println!("B exited with {exit}; stderr: {stderr}");
     assert!(!exit.success(), "B must exit non-zero");
-    assert!(stderr.contains("JournalInUse"), "B names the cause: {stderr}");
-    assert!(stdout.is_empty(), "B never got as far as listening: {stdout}");
+    assert!(
+        stderr.contains("JournalInUse"),
+        "B names the cause: {stderr}"
+    );
+    assert!(
+        stdout.is_empty(),
+        "B never got as far as listening: {stdout}"
+    );
 
     // A's job finishes on its first attempt: B never expired A's lease.
     let status = wait_for(&a.addr, job, Duration::from_secs(10), |s| {
exit 1
```

The plan's blocks for `StoreError::JournalInUse` and for the new test are not in rustfmt's layout. I ran `cargo fmt --all` and committed the result separately, as `3bcf462` (deviation 1). Its whole diff is the layout above, which includes one added trailing comma after `lock: PathBuf`.

**Second run, after `3bcf462`,** complete:

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
    Checking ledger-store v0.1.0 (…/crates/ledger-store)
    Checking ledgerd v0.1.0 (…/crates/ledgerd)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s
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
test a_completed_job_records_its_result ... ok
test a_duplicate_submission_records_nothing ... ok
test a_failure_retries_with_backoff_until_attempts_run_out ... ok
test an_expired_lease_is_retried_and_its_holder_is_refused ... ok
test nothing_due_means_nothing_leased ... ok
test an_unknown_job_is_refused ... ok
test zero_attempts_is_refused ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test attempts_never_exceed_max_attempts ... ok
test the_same_commands_give_the_same_events ... ok
test replaying_the_events_rebuilds_the_state ... ok
test a_job_succeeds_at_most_once ... ok
test a_stale_lease_changes_nothing ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
running 6 tests
test an_unfinished_non_idempotent_effect_is_killed_not_repeated ... ok
test a_lease_whose_effect_never_started_is_retried_whatever_its_kind ... ok
test a_finished_effect_completes_without_running_again ... ok
test jobs_that_are_not_leased_are_left_alone ... ok
test an_unfinished_idempotent_effect_is_re_driven ... ok
test the_last_attempt_dies_instead_of_retrying ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test an_attempt_records_its_intent_once ... ok
test an_outcome_without_an_intent_is_refused ... ok
test an_intent_without_an_outcome_is_unfinished ... ok
test an_outcome_finishes_its_intent ... ok
test unfinished_effects_survive_a_reopen_oldest_first ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
running 4 tests
test an_empty_journal_replays_nothing ... ok
test events_replay_in_append_order ... ok
test the_journal_survives_a_reopen ... ok
test any_batching_replays_the_same_sequence ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
running 3 tests
test in_memory_journals_do_not_contend ... ok
test a_journal_already_open_is_refused ... ok
test the_journal_opens_again_once_its_holder_is_dropped ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test a_job_killed_mid_effect_completes_exactly_once_after_restart ... ignored, spawns and kills the real server; run by CI's crash-recovery job
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 3 tests
test an_unknown_job_is_not_found_and_zero_attempts_is_refused ... ok
test a_duplicate_submission_returns_the_same_job ... ok
test a_submitted_job_runs_to_success ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
running 1 test
test a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.16s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0

$ cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture

running 1 test
killed mid-effect: signal: 9 (SIGKILL)
ledgerd: recovered 1 leased job(s) after a restart
succeeded 3.067933341s after the kill: {"effect":{"Finished":{"attempt":2}},"job":{"attempts":2,"id":0,"idempotency_key":"crash","kind":{"SleepThenRecord":{"ms":2000,"token":"crash-test"}},"max_attempts":3,"state":{"Succeeded":{"result":"crash-test"}}}}
test a_job_killed_mid_effect_completes_exactly_once_after_restart ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.21s

exit 0
```

- All 5 ratchets hold.
- The ratchet self-tests pass.
- The code is formatted, and clippy is clean.
- Every test passes: 38 run and 0 fail, including the 3 new store tests and the new `ledgerd` test. The crash test is the 1 ignored test, and it runs next.
- The crash test passes after `kill -9`: the restarted process takes the lock and recovers.
- `core-line-budget` reads 234 of 250, the same as at the end of plan 04: nothing under `crates/ledger-core/` changed.

**CI's shellcheck step** is not in the plan's Task 3. No shell script changed, but I ran it anyway:

```
$ shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh
exit 0
```

### Brief 05's harness, as the regression check (Task 4)

I ran `measure.sh` as committed, without editing it:

```
$ nix develop -c bash -o pipefail -c '
  H=docs/pipeline/reports/2026-10-14-05-evidence/measure.sh
  for run in 1 2 3; do echo "### two-processes $run"; bash "$H" two-processes "$run"; echo "exit $?"; done
  echo "### two-processes 4 same-addr"; bash "$H" two-processes 4 same-addr; echo "exit $?"
' 2>&1 | grep -v "^warning: Git tree" > target/plan05-case3.txt
```

Run 1, complete. Its `cargo build` compiled four crates; the later runs found the build fresh:

```
### two-processes 1
journal: target/brief05/two-processes-different-addr-1.cEHIgM/ledger.db
$ cargo build -p ledgerd
   Compiling bitflags v2.13.2
   Compiling rusqlite v0.37.0
   Compiling ledger-store v0.1.0 (…/crates/ledger-store)
   Compiling ledgerd v0.1.0 (…/crates/ledgerd)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.13s
+0.0s A started: pid 827565, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:37941
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29972 ms left; starting B on the same LEDGER_DB
+0.2s B did not start: it exited with status 1 before printing an address
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1)
  effects: (0,1)=none
+5.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1) Succeeded(0)
  effects: (0,1)=run-1
+8.4s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: no
== A's stderr
(empty)
== B's stderr
Error: Store(JournalInUse { lock: "target/brief05/two-processes-different-addr-1.cEHIgM/ledger.db.lock" })
== events
+-----+-----------+-----+--------------------+
| seq |   kind    | job |       detail       |
+-----+-----------+-----+--------------------+
|   1 | Submitted |   0 | key long           |
|   2 | Leased    |   0 | lease 0, attempt 1 |
|   3 | Succeeded |   0 | result run-1       |
+-----+-----------+-----+--------------------+
== effects
+-----+---------+-----------------------+---------+
| job | attempt | intent_ms_after_first | outcome |
+-----+---------+-----------------------+---------+
|   0 |       1 |                     0 | run-1   |
+-----+---------+-----------------------+---------+
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      1 |       1 |        1 |         1 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      3 |         1 |        3 | ok              |
+--------+-----------+----------+-----------------+
exit 0
```

Runs 2 and 3 are shown without their `events` and `effects` tables. Those match run 1's, apart from the token (`run-2`, `run-3`):

```
### two-processes 2
journal: target/brief05/two-processes-different-addr-2.unIFNp/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
+0.0s A started: pid 829060, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:34477
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29964 ms left; starting B on the same LEDGER_DB
+0.2s B did not start: it exited with status 1 before printing an address
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1)
  effects: (0,1)=none
+5.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1) Succeeded(0)
  effects: (0,1)=run-2
+8.4s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: no
== A's stderr
(empty)
== B's stderr
Error: Store(JournalInUse { lock: "target/brief05/two-processes-different-addr-2.unIFNp/ledger.db.lock" })
[…]
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      1 |       1 |        1 |         1 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      3 |         1 |        3 | ok              |
+--------+-----------+----------+-----------------+
exit 0
### two-processes 3
journal: target/brief05/two-processes-different-addr-3.gyHdQe/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
+0.0s A started: pid 830586, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:34099
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29967 ms left; starting B on the same LEDGER_DB
+0.2s B did not start: it exited with status 1 before printing an address
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1)
  effects: (0,1)=none
+5.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1) Succeeded(0)
  effects: (0,1)=run-3
+8.4s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: no
== A's stderr
(empty)
== B's stderr
Error: Store(JournalInUse { lock: "target/brief05/two-processes-different-addr-3.gyHdQe/ledger.db.lock" })
[…]
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      1 |       1 |        1 |         1 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      3 |         1 |        3 | ok              |
+--------+-----------+----------+-----------------+
exit 0
```

The same-addr run, where B is given A's own bound address. It is complete:

```
### two-processes 4 same-addr
journal: target/brief05/two-processes-same-addr-4.3IGX0e/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.0s A started: pid 832089, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:46433
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29970 ms left; starting B on the same LEDGER_DB
+0.2s B did not start: it exited with status 1 before printing an address
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1)
  effects: (0,1)=none
+5.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1) Succeeded(0)
  effects: (0,1)=run-4
+8.4s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: no
== A's stderr
(empty)
== B's stderr
Error: Store(JournalInUse { lock: "target/brief05/two-processes-same-addr-4.3IGX0e/ledger.db.lock" })
== events
+-----+-----------+-----+--------------------+
| seq |   kind    | job |       detail       |
+-----+-----------+-----+--------------------+
|   1 | Submitted |   0 | key long           |
|   2 | Leased    |   0 | lease 0, attempt 1 |
|   3 | Succeeded |   0 | result run-4       |
+-----+-----------+-----+--------------------+
== effects
+-----+---------+-----------------------+---------+
| job | attempt | intent_ms_after_first | outcome |
+-----+---------+-----------------------+---------+
|   0 |       1 |                     0 | run-4   |
+-----+---------+-----------------------+---------+
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      1 |       1 |        1 |         1 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      3 |         1 |        3 | ok              |
+--------+-----------+----------+-----------------+
exit 0
```

**Beside brief 05's numbers.** Both columns come from the same harness and the same steps. Brief 05's numbers are from `run-3-two-processes-{1,2,3}.txt`, as quoted in report 05. Times are sample times, counted from A's start:

| | brief 05, runs 1 / 2 / 3 | plan 05, runs 1 / 2 / 3 |
|---|---|---|
| Does B start? | yes / yes / yes | no / no / no: "exited with status 1 before printing an address", at +0.2 s |
| B's stderr | `ledgerd: recovered 1 leased job(s) after a restart` | `Error: Store(JournalInUse { lock: "target/brief05/…/ledger.db.lock" })` |
| Time left on A's lease when B started | 29,961 / 29,968 / 29,968 ms | 29,972 / 29,964 / 29,967 ms |
| A's attempt 1 marked `abandoned`, and a `Retrying` event | by +0.2 s, every run | never |
| B re-drives the effect (`Leased(0,1,2)`, intent `(0,2)`) | +1.4 / +1.1 / +1.4 s | never |
| A's `Succeeded` | +5.1 / +5.1 / +5.3 s, for a lease the journal had already retried | +5.3 s, every run, for its live lease |
| A's view of the effect while it ran | `Finished(1)` from +0.2 s: B had marked it `abandoned` | `InFlight(1)` until it finished |
| Events in the journal | 6: `Submitted Leased Retrying Leased Succeeded Succeeded` | 3: `Submitted Leased Succeeded` |
| Job 0's leases, intents, outcomes, `Succeeded` events | 2, 2, 2, 2 | 1, 1, 1, 1 |
| `integrity_check`; event `seq` | ok; 1 to 6 | ok; 1 to 3 |
| Quiet at | +9.3 / +9.4 / +9.3 s | +8.4 s, every run |
| Still running at the end | A yes, B yes | A yes, B no |

| same-addr | brief 05 | plan 05 |
|---|---|---|
| B | "did not start: it exited with status 1 before printing an address", at +0.2 s | the same line, at +0.2 s |
| B's stderr | `ledgerd: recovered 1 leased job(s) after a restart`, then `Error: Os { code: 98, kind: AddrInUse, … }` | `Error: Store(JournalInUse { lock: "target/brief05/…/ledger.db.lock" })` |
| A's attempt 1 marked `abandoned`, and a `Retrying` event | yes, at the +0.2 s sample | no |
| Events in the journal | 4: `Submitted Leased Retrying Succeeded` | 3: `Submitted Leased Succeeded` |
| Job 0's leases, intents, outcomes, `Succeeded` events | 1, 1, 1, 1 | 1, 1, 1, 1 |

The plan expected this result. B exits at once with `JournalInUse`, and the job ends with 1 lease, 1 intent, 1 outcome and 1 `Succeeded` event. In the same-addr run nothing is appended for A's job, and no attempt is marked `abandoned`.

**How the harness copes with a B that refuses.** It didn't wait or time out. Its `start` treats an early exit as "did not start", and `watch` counts a down process as settled. Three things in its output could still mislead:
- **"B did not start" is not evidence by itself.** The line is word for word what brief 05's same-addr run printed, where B had already written `Retrying` and `abandoned` to the journal. Only B's stderr and the journal tell the two apart.
- **The same-addr variant no longer tests the bind.** B now exits at the lock, before it reaches the bind. The variant measures the same refusal as the default one, and finding 2's order (replay, then bind) is no longer reached by a second process on the same path.
- **`exit 0` after each run is the harness's own exit status, not B's.** B's status is the 1 in the "did not start" line.

### A symlink to the journal is not refused (a check the plan did not ask for)

The lock's name is the journal's path string with `.lock` appended. `target/plan05/alias.sh` starts A on `ledger.db`. It then starts B on the same path, and C on `alias.db`, a symlink to `ledger.db` in the same directory. It submits no job, so nothing re-runs, and it deletes nothing:

```
$ nix develop -c bash -o pipefail -c 'bash target/plan05/alias.sh'
dir: target/plan05/alias.AlCdrc
a stdout: ledgerd listening on 127.0.0.1:34259
$ LEDGER_DB=$DIR/ledger.db ledgerd   # B, same path as A
b exited with status 1; stderr: Error: Store(JournalInUse { lock: "target/plan05/alias.AlCdrc/ledger.db.lock" })
$ LEDGER_DB=$DIR/alias.db ledgerd    # C, a symlink to A's journal
c stdout: ledgerd listening on 127.0.0.1:41575
still running? A: yes, B: no, C: yes
$ ls -1 $DIR | grep -v '\.\(out\|err\)$'
alias.db
alias.db.lock
ledger.db
ledger.db.lock
ledger.db-shm
ledger.db-wal
target/plan05/alias.sh: line 31: 846465 Killed                     LEDGER_DB=$DIR/alias.db LEDGER_ADDR=127.0.0.1:0 "$BIN" > "$DIR/c.out" 2> "$DIR/c.err"
```

- B, on A's own path, was refused.
- C, on the symlink, started and listened, and took a second lock file, `alias.db.lock`.
- No `alias.db-wal` or `alias.db-shm` appeared. That is consistent with SQLite following the symlink to `ledger.db`, the same journal A holds. I didn't read C's journal back.
- The last line is bash reporting the script's own `kill -KILL` of C.

## Inferred (not proven by anything above)

- **The guarantee is host-local.** Advisory locks are not reliable on network filesystems, and multiple nodes stay out of scope (spec §10; ADR 0005). Nothing here ran on a network filesystem or on two hosts.
- **B never opens the SQLite file.** I infer this from the code's order, not from a trace:
  - `Store::open` calls `lock_journal` before `Connection::open`.
  - `Ledger::open` replays and recovers only after `Store::open` returns.
  - `main.rs` binds only after `Ledger::open` returns.

  What is measured is the journal's contents and B's output:
  - no event appended for A's job, and no outcome rewritten
  - no "recovered" line on B's stderr
  - nothing on B's stdout
- **B does touch one file.** `lock_journal` opens `<journal>.lock` for writing, with `create(true)` and `truncate(false)`. When A is running, that file already exists, so B creates nothing and changes no content. I didn't check its mtime.
- **The lock lasts as long as the open file, not the `Store` value alone.** `kill -9` releases it: the crash test's restarted process took it. The kernel releasing the lock when the file is closed is what that relies on. A child process that inherited the descriptor would keep the lock. `ledgerd` starts no child processes, and std opens files close-on-exec, so an exec'd child would not inherit it. I didn't test that.
- **Other spellings of the path.** These are inferred from how the lock's name is built, and not measured:
  - A relative path, a path through `..` or one through a symlinked *directory* ends at the same directory entry, so it gets the same lock file. The harness used relative paths and the tests used absolute ones, and both were refused. No run mixed the two.
  - A symlink to the journal *file* gets a different lock file, as measured above.
  - So would a hard link. SQLite names its `-wal` and `-shm` files after the path it was given, so a hard-linked journal would get a second WAL. That is unsafe for SQLite whether or not this lock exists.
- **The operator sees the Debug form of the error.** B's stderr reads `Error: Store(JournalInUse { lock: "…" })`, as measured above. `StoreError`'s `Display` text, "journal in use by another process (lock held on …)", is never printed. `main` returns the error, and Rust prints a returned error's Debug form. The plan's test asserts the variant name, which is in that form.
- **Finding 2, more exactly.** A second process on the same path is now refused before replay. The order in `main.rs` hasn't changed: replay and recovery still come before the bind. A process that holds the lock and then fails to bind, say because some other program has the port, has still run recovery. In that case it was the only holder, so the leases it expired really did belong to a dead process.
- **Report 05's findings 3 to 6 are not addressed.** Which ones a single writer turns from a live bug into a known limit:
  - **Finding 3 (`record_outcome` overwrites an outcome): becomes a known limit.** It needs a second writer to the same attempt. Inside one process, each attempt records one outcome, and recovery marks only unfinished intents (report 05). The store still has no refusal, so a second writer would rewrite history silently.
  - **Finding 4 (each process checks leases against its own memory): becomes a known limit.** It needs a second process. Report 05 itself proposed no step "while finding 1's lock holds".
  - **Finding 5 (an expired lease is retried without consulting its effect record): still live.** It happens inside one process, after `finish_effect` fails in the store. A single writer doesn't change it.
  - **Finding 6 (one slow effect stops the whole queue): still live.** It is a liveness limit of the one executor loop in one process. A single writer doesn't change it.

  So ADR 0005's line "with one writer per journal, each one becomes a known limit" holds for findings 3 and 4, not for 5 and 6. Through finding 1 below, findings 3 and 4 can also still be reached today.
- **What no run here covers:**
  - a network filesystem, or more than one host
  - power loss (report 04's caveat stands)
  - `StoreError::Lock`: a lock file that can't be opened, such as in a read-only directory, and the `TryLockError::Error` arm. No test reaches either.
  - two stores racing to open the same journal at the same instant. The tests open one after the other.
  - this branch's CI run. The report is committed before the push, so CI is not quoted here. The coordinator verifies it before merging.

## Findings for the operator

1. **A symlink to the journal file gets past the lock.**
   - *Measured:* `target/plan05-alias.txt`, above. C started on `alias.db`, a symlink to A's live `ledger.db`, with its own `alias.db.lock`.
   - Running C against a job in flight was not measured. From report 05's case 3, it would re-run the effect, as B did there.
   - *Proposed next step:* for the operator to decide; this plan didn't change the mechanism. One option: build the lock's name from the journal's resolved path, so every spelling of one file meets at one lock file. The journal may not exist yet on the first open, so that means resolving its directory and then appending the file name. Another option: document "one path per journal" as a limit, beside "host-local".
2. **The refusal prints the Debug form.**
   - *Measured:* B's stderr, above.
   - *Proposed next step:* none needed for correctness. If operators should see the `Display` text, `main` would have to print the error itself.

## Code, commits and ratchets

### The code is the plan

I extracted the plan's two `<!-- file: … -->` blocks into `target/plan05/render` with `target/plan05/render.awk`. Then I ran `rustfmt --edition 2024` on them, with `crates/ledgerd/tests/common/` copied beside them, and compared each one with the committed file:

```
rendered crates/ledger-store/tests/lock.rs
rendered crates/ledgerd/tests/one_per_journal.rs
identical  crates/ledger-store/tests/lock.rs
identical  crates/ledgerd/tests/one_per_journal.rs
```

The edits the plan gives as instructions rather than as whole files:
- **`crates/ledger-store/src/lib.rs`:** Task 1 Step 3's five replacements, made by one host `python3` call. Each replacement checked that its target text appeared exactly once. After that, only `cargo fmt` changed the file, by splitting `JournalInUse { lock: PathBuf }` across three lines.
- **`crates/ledgerd/src/main.rs`:** Task 2 Step 4's first lines. The plan says "replace the first four lines". Its replacement keeps lines 1, 2 and 4 as they were and turns line 3 into two lines, so the diff is one line removed and two added.

### Commits and the ratchets

```
$ git config user.email
26385844+ioitiki@users.noreply.github.com

$ git log --reverse --format='%h %an <%ae>%n    %s%n    %(trailers:key=Co-Authored-By)' origin/main..HEAD
bd86530 Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    Plan 05 and ADR 0005: one ledgerd per journal
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

23965fd Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    ledger-store: one holder per journal (ADR 0005)
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

aae9aa8 Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    ledgerd: a second process on a journal in use refuses to start
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

3bcf462 Andrew Malkin <26385844+ioitiki@users.noreply.github.com>
    cargo fmt: plan 05's blocks in rustfmt's layout
    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

$ git diff --stat origin/main
 crates/ledger-store/src/lib.rs                     |  56 +++-
 crates/ledger-store/tests/lock.rs                  |  31 ++
 crates/ledgerd/src/main.rs                         |   3 +-
 crates/ledgerd/tests/one_per_journal.rs            |  77 +++++
 docs/decisions/0005-one-ledgerd-per-journal.md     |  42 +++
 .../plans/2026-10-14-05-one-ledgerd-per-journal.md | 371 +++++++++++++++++++++
 .../2026-10-14-ratchets-before-code-design.md      |   2 +-
 7 files changed, 574 insertions(+), 8 deletions(-)

$ git diff --stat origin/main -- scripts/ crates/ledger-core/ docs/pipeline/reports/2026-10-14-05-evidence/ Cargo.toml Cargo.lock
(no output)
```

- `bd86530` is the plan, the ADR and a one-line spec edit. It was already on the branch when this worker started.
- This report is committed after the log above, with the same author and trailer.
- Nothing changed under `scripts/` or `crates/ledger-core/`, and `BUDGET` is untouched. Neither did `Cargo.toml`, `Cargo.lock` or brief 05's evidence directory.
- No ratchet blocked this plan.

## Deviations from the plan

1. **Formatting.** The plan's code is not in rustfmt's layout. Plans 02 and 03 ran `cargo fmt --all` before their commits, and plan 04's worker did so unasked (report 04, deviation 1). This plan doesn't, so I committed Tasks 1 and 2 as written.
   - The Task 1 and Task 2 commits (`23965fd`, `aae9aa8`) are the plan's text as written.
   - The first Task 3 run failed `cargo fmt --all -- --check`. I ran `cargo fmt --all` and committed the result as `3bcf462`, then ran Task 3 again from the top.
   - The change is layout, plus one trailing comma. "The code is the plan" shows the committed files are the plan's blocks after rustfmt.
2. **Checks the plan did not ask for.** None of them changed committed code:
   - CI's shellcheck step
   - the symlink check, `target/plan05/alias.sh`
   - the render comparison
   - the tool versions
3. **Not every command ran inside the dev shell.** As in plans 02 to 04:
   - **`git` and `gh` ran on the host.** That includes the `git worktree add` and `remove` for the red run, as the plan writes them.
   - **The plan's `cp` ran on the host,** copying the new test into that worktree.
   - **One `python3` call on the host** applied Task 1 Step 3's replacements to `lib.rs`. It read a script from stdin and ran without `-I`. It read and wrote only that file.
   - **My first check that `cargo fmt` changed only layout** used host `grep`, `sed`, `tr` and `cmp`. They wrote three scratch files under `target/`, which I then deleted with `rm -f`. That check reported a difference, which turned out to be the trailing comma, so I read the diff itself instead. That diff is the first-run `cargo fmt` output quoted above.
   - **Other host commands:**
     - `cat`, `head`, `sed -n`, `grep`, `ls` and `wc`, to read files in the worktree
     - `mkdir`, and heredocs that wrote `render.awk`, `alias.sh` and this report's draft, all under `target/plan05/`
     - `cp`, to keep the first gate run as `plan05-gates-first.txt`
     - `awk` and `grep`, to find line ranges in the captures
     - one `python3 -I` call, which made five text replacements in this report's draft; later corrections to the draft were made with Claude Code's file editor
     - one `awk` script, `target/plan05/splice.awk`, which spliced the captures into this report from the draft

   Every cargo, rustfmt, shellcheck, sqlite3 and ratchet command ran in `nix develop`. So did the harness, the alias script and the `awk` that rendered the plan's blocks.
4. **The first render failed.** rustfmt couldn't resolve `mod common;` in the rendered `one_per_journal.rs`. I copied `crates/ledgerd/tests/common/` into the render directory and ran the comparison again; that second run is the one quoted above.
5. **What was deleted, all of it under `target/`:**
   - the red run's worktree, which the plan removes with `git worktree remove --force`
   - the three scratch files from deviation 3
   - the first render directory, `target/plan05/render`, with `rm -rf` before the second render

   The harness's run directories stay under `target/brief05/`, and the alias run's under `target/plan05/`.
6. **Expected output that matched, noted for completeness:**
   - Task 1 Step 2 failed to compile on `StoreError::JournalInUse`, as the plan expected.
   - The red run failed with "B is still running: it started on a journal that was already in use".
   - The harness printed no error and did not time out.

**Clean-room:** every file this worker read or wrote is inside this worktree. The tools went outside it in their usual way:
- For the red run, `git worktree add` and `git worktree remove` wrote to the repository's shared `.git`, in the main checkout.
- `test-ratchets.sh` made scratch copies with `mktemp -d`.
- `tempfile::tempdir()` made the tests' journals under the dev shell's temporary directory (`/tmp/nix-shell.*`).
