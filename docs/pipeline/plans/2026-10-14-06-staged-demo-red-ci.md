# Plan 06: the red CI run (staged demo)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`../specs/2026-10-14-ratchets-before-code-design.md`](../specs/2026-10-14-ratchets-before-code-design.md), §6 (required evidence 2) and §7 step 6.
**Executed by:** a worker agent in its own Orca worktree. It ends with a PR that the coordinator verifies and merges.

## This is a staged demonstration

Spec §6 requires a red CI run that catches an agent. Through plans 02 to 05, no worker broke a ratchet on its own:
- Plan 04's core came in under its budget.
- Plan 05's lock went into the store, where it belongs.

So, as §6 provides, this plan **instructs** the worker to make the mistake. The pull request's title starts with `[staged demo]`, and its description says, before anything else, that the violation was ordered by this plan, not made by accident. The essay will say the same.

**The mistake** is the one `scripts/ratchets/core-purity.sh` names in its header: "a query to check for a duplicate". The worker adds a SQL lookup for an existing idempotency key to `ledger-core`, as a belt-and-braces check beside the core's own. CI should go red on `core-purity` (a dependency outside the allowlist, and a forbidden name in the source) and on `sql-containment` (SQL outside `ledger-store`).

**The fix** removes the lookup. It was never needed: the core already refuses a duplicate key in `decide`, against state that replay rebuilds from the journal.

The fix also adds the test that proves this across a restart, which no test covers yet. So the merged PR leaves one real test behind. Its history keeps the red commit, because nothing is squashed or rewritten (spec §3, honesty).

**Rules for the worker:** the same as plans 02–05, with one deliberate exception, Task 1.
- clean-room
- the noreply commit identity, and a trailer on every commit
- `nix develop -c`
- gate captures under `target/`
- `bash -o pipefail`
- a report that quotes real output and keeps Measured separate from Inferred

**The exception:** Task 1 breaks a ratchet on purpose, and pushes without running the ratchets first, because the point is to watch CI catch it.

**Never edit a ratchet, its allowlist, its fixtures or the CI workflow.** If CI does not go red as predicted, stop and report exactly what happened. Do not adjust the mistake until it fails. An unexpected green is a finding about the ratchets.

---

## File map

| Path | Change |
|---|---|
| `crates/ledger-core/Cargo.toml` | Task 1 adds `rusqlite`; Task 3 removes it |
| `crates/ledger-core/src/lib.rs` | Task 1 adds `submitted_in_journal`; Task 3 removes it |
| `crates/ledgerd/tests/http.rs` | Task 3 adds the across-restart duplicate test |
| `docs/pipeline/reports/2026-10-14-06-staged-demo-red-ci.md` | live proof |

At the end, the net diff against `main` is the new test and the report. The two code commits cancel out.

---

### Task 1: The staged mistake

- [ ] **Step 1: Add the dependency to `crates/ledger-core/Cargo.toml`**

Under `[dependencies]`, after `ledger-api.workspace = true`, add:

```toml
rusqlite.workspace = true
```

- [ ] **Step 2: Add the lookup to `crates/ledger-core/src/lib.rs`**

Append at the end of the file:

```rust
/// Belt and braces: ask the journal itself whether this key was already submitted.
pub fn submitted_in_journal(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(body, '$.Submitted.idempotency_key') = ?1)",
        [key],
        |row| row.get(0),
    )
}
```

- [ ] **Step 3: Check that it compiles, and nothing more**

Run: `nix develop -c bash -o pipefail -c 'cargo build -p ledger-core 2>&1 | tail -3'`
Expected: it builds. Do **not** run the ratchets, clippy or the tests before pushing.

- [ ] **Step 4: Commit and push**

```bash
git add crates/ledger-core/Cargo.toml crates/ledger-core/src/lib.rs Cargo.lock
git commit -m "ledger-core: check the journal for a duplicate idempotency key [staged demo]

Staged by plan 06: this commit breaks core-purity and sql-containment on
purpose, so CI's ratchets can be seen catching an agent (spec §6).

Co-Authored-By: <your model> <noreply@anthropic.com>"
git push -u origin HEAD
```

- [ ] **Step 5: Open the PR, labeled staged**

```bash
gh pr create --base main --title "[staged demo] A SQL duplicate check in ledger-core, caught by the ratchets" --body "**This is a staged demonstration.** Plan 06 (docs/pipeline/plans/2026-10-14-06-staged-demo-red-ci.md) ordered the worker to put a SQL lookup for duplicate idempotency keys into ledger-core, because no worker broke a ratchet on its own during plans 02-05 (spec §6, required evidence 2). The first commit is that mistake, and CI should go red on core-purity and sql-containment. A follow-up commit removes it and adds the test that shows the core's own check holds across a restart. Live proof: docs/pipeline/reports/2026-10-14-06-staged-demo-red-ci.md."
```

---

### Task 2: Watch CI go red, and keep the record

- [ ] **Step 1: Wait for the run, then capture it**

```bash
mkdir -p target
gh pr checks --watch --interval 20 || true
RUN=$(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
echo "run: $RUN" | tee target/plan06-red-run.txt
gh run view "$RUN" | tee -a target/plan06-red-run.txt
gh run view "$RUN" --log-failed | tee target/plan06-red-log.txt
```

Expected:
- The `ratchets` job fails.
- `core-purity` reports `rusqlite` as a dependency outside the allowlist, and flags the forbidden name in the source.
- `sql-containment` reports the SQL in `crates/ledger-core/src/lib.rs`.
- `build` and `crash-recovery` are skipped, because they need `ratchets`.

Whatever actually happened goes into the report verbatim, including any ratchet you did not expect, such as `core-line-budget`.

- [ ] **Step 2: Reproduce it locally, for the record**

Run: `nix develop -c bash -o pipefail -c './scripts/ratchets/all.sh; echo "exit $?"' 2>&1 | grep -v "^warning: Git tree" | tee target/plan06-red-local.txt`
Expected: the same failures, and exit 1.

---

### Task 3: The fix, and the test that makes the lookup unnecessary

- [ ] **Step 1: Remove the mistake**

- Delete the `rusqlite.workspace = true` line from `crates/ledger-core/Cargo.toml`.
- Delete the `submitted_in_journal` function and its doc comment from `crates/ledger-core/src/lib.rs`.
- Run `git diff main -- crates/ledger-core Cargo.lock`. Expected: no output.

- [ ] **Step 2: Add the across-restart test to `crates/ledgerd/tests/http.rs`**

Append:

```rust
#[test]
fn a_duplicate_submitted_after_a_restart_returns_the_same_job() {
    // The core refuses a duplicate key against state that replay rebuilds
    // from the journal, so no lookup in the journal is needed (plan 06).
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.db");
    let body = r#"{"idempotency_key":"once","kind":{"SleepThenRecord":{"ms":1,"token":"t"}},"max_attempts":1}"#;

    let first = start(&db);
    let (code, reply) = http(&first.addr, "POST", "/jobs", Some(body));
    assert_eq!(code, 201, "{reply}");
    let job = json(&reply)["job"].clone();
    drop(first);

    let second = start(&db);
    let (code, reply) = http(&second.addr, "POST", "/jobs", Some(body));
    assert_eq!(code, 200, "{reply}");
    assert_eq!(json(&reply)["job"], job);
    assert_eq!(json(&reply)["created"], false);
}
```

- [ ] **Step 3: Run it**

Run: `nix develop -c bash -o pipefail -c 'cargo test -p ledgerd --test http 2>&1 | grep -E "^(test |test result)"'`
Expected: 4 tests pass, including the new one.

- [ ] **Step 4: Every gate, in CI's order**

```bash
nix develop -c bash -o pipefail -c '
  echo "\$ ./scripts/ratchets/all.sh"; ./scripts/ratchets/all.sh; echo "exit $?"
  echo; echo "\$ ./scripts/ratchets/test-ratchets.sh"; ./scripts/ratchets/test-ratchets.sh; echo "exit $?"
  echo; echo "\$ cargo fmt --all -- --check"; cargo fmt --all -- --check && echo "(formatted)"; echo "exit $?"
  echo; echo "\$ cargo clippy --workspace --all-targets -- -D warnings"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3; echo "exit $?"
  echo; echo "\$ cargo test --workspace"; cargo test --workspace 2>&1 | grep -E "^(test result|running|test )"; echo "exit $?"
' 2>&1 | grep -v "^warning: Git tree" > target/plan06-gates.txt; cat target/plan06-gates.txt
```

Expected: all green. If `cargo fmt` reformats the new test, run `cargo fmt --all` and include the result in this commit.

- [ ] **Step 5: Commit and push**

```bash
git add crates/ledger-core/Cargo.toml crates/ledger-core/src/lib.rs Cargo.lock crates/ledgerd/tests/http.rs
git commit -m "Remove the journal lookup from ledger-core; test duplicates across a restart

The core already refuses a duplicate key in decide, against state that
replay rebuilds from the journal. The new test proves that across a
restart, so the lookup the ratchets caught was never needed.

Co-Authored-By: <your model> <noreply@anthropic.com>"
git push
```

- [ ] **Step 6: Watch CI go green, and capture it**

```bash
gh pr checks --watch --interval 20 || true
RUN=$(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
echo "run: $RUN" | tee target/plan06-green-run.txt
gh run view "$RUN" | tee -a target/plan06-green-run.txt
```

Expected: `ratchets`, `build` and `crash-recovery` all pass.

---

### Task 4: Report

- [ ] **Step 1: Write `docs/pipeline/reports/2026-10-14-06-staged-demo-red-ci.md`**

Shape it like reports 01–05. It opens with one sentence: **"This was staged: plan 06 instructed the worker to make this mistake."**

**Measured:**
- the red run's URL and its failing log, verbatim
- the local reproduction
- the gate output after the fix
- the green run's URL and its job list

**Inferred:**
- what the ratchets would and would not catch if an agent made this mistake unprompted. For example: a lookup through a helper crate that is on no list, or SQL built from string pieces.
- what this demonstration does not prove

**Deviations from the plan.**

- [ ] **Step 2: Commit and push**

```bash
git add docs/pipeline/reports/2026-10-14-06-staged-demo-red-ci.md
git commit -m "Plan 06: live-proof report

Co-Authored-By: <your model> <noreply@anthropic.com>"
git push
```

Do not merge, squash or force-push. The red commit stays in the branch's history.
