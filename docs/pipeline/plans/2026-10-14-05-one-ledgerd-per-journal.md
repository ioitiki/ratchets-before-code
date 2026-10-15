# Plan 05: one `ledgerd` per journal

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Brief:** [`../briefs/2026-10-14-05-do-leases-need-a-distributed-lock.md`](../briefs/2026-10-14-05-do-leases-need-a-distributed-lock.md). Its report is [`../reports/2026-10-14-05-do-leases-need-a-distributed-lock.md`](../reports/2026-10-14-05-do-leases-need-a-distributed-lock.md); this plan builds finding 1 (and so closes finding 2).
**Decision:** [ADR 0005](../../decisions/0005-one-ledgerd-per-journal.md), decided by Andy.
**Executed by:** a worker agent in its own Orca worktree. It ends with a PR that the coordinator verifies and merges.

**Goal:** a second `ledgerd` started on a journal that is already in use exits non-zero having written nothing, and the first process's work is undisturbed.

**Architecture:**
- **`Store::open` takes an exclusive, non-blocking lock first:** `std::fs::File::try_lock` on a sidecar file, `<journal>.lock`.
  - It happens before the SQLite connection is opened, so before replay, recovery and binding a port.
  - `Store` holds the file for its whole life. The kernel releases the lock when the process dies, including on `kill -9`, so crash recovery is unchanged.
- **A refused open is `StoreError::JournalInUse { lock }`.** `ledgerd` already propagates store errors out of `main`, so it exits non-zero and prints the error.
- **`open_in_memory` takes no lock.** No other process can reach that journal.
- **Nothing in `ledger-core` changes,** and neither does its line budget.

**Tech stack:** std only. `File::try_lock` and `TryLockError` have been stable since Rust 1.89, and the toolchain is 1.98.1. No new dependency.

**Rules for the worker:** the same as plans 02–04, and binding:
- clean-room
- never weaken a ratchet; if one blocks you, stop and report it
- the noreply commit identity, and a trailer on every commit
- `nix develop -c`
- gate captures under `target/`
- `bash -o pipefail`
- a report that quotes real output and keeps Measured separate from Inferred

**Do not edit** `docs/pipeline/reports/2026-10-14-05-evidence/measure.sh` or its `run-*.txt` files. They are brief 05's record. Task 4 runs the harness as it is. If it fails or misreports against the new behavior, say exactly how in the report; don't fix it.

---

## File map

| Path | Change |
|---|---|
| `crates/ledger-store/src/lib.rs` | `StoreError::JournalInUse` and `StoreError::Lock`; `Store` holds the lock; `lock_journal` |
| `crates/ledger-store/tests/lock.rs` | new: one holder per journal, in-process |
| `crates/ledgerd/src/main.rs` | doc comment only |
| `crates/ledgerd/tests/one_per_journal.rs` | new: a second real `ledgerd` refuses and leaves the first alone |
| `docs/pipeline/reports/2026-10-14-05-one-ledgerd-per-journal.md` | live proof |

---

### Task 1: The store refuses a journal that is already open

- [ ] **Step 1: Write the failing tests**

<!-- file: crates/ledger-store/tests/lock.rs -->
```rust
//! One journal, one holder (ADR 0005).

use ledger_store::{Store, StoreError};

#[test]
fn a_journal_already_open_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.db");
    let _first = Store::open(&path).unwrap();
    match Store::open(&path) {
        Err(StoreError::JournalInUse { lock }) => {
            assert_eq!(lock, dir.path().join("ledger.db.lock"));
        }
        Err(other) => panic!("expected JournalInUse, got: {other}"),
        Ok(_) => panic!("a second store opened a journal that is already in use"),
    }
}

#[test]
fn the_journal_opens_again_once_its_holder_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.db");
    drop(Store::open(&path).unwrap());
    Store::open(&path).unwrap();
}

#[test]
fn in_memory_journals_do_not_contend() {
    let _first = Store::open_in_memory().unwrap();
    let _second = Store::open_in_memory().unwrap();
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `nix develop -c bash -o pipefail -c 'cargo test -p ledger-store --test lock 2>&1 | tail -20'`
Expected: a compile error, because `StoreError::JournalInUse` does not exist yet.

- [ ] **Step 3: Implement the lock in `crates/ledger-store/src/lib.rs`**

Replace the two `use std::…` lines at the top with:

```rust
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
```

Add two variants to `StoreError`, after `NoSuchIntent`:

```rust
    /// Another holder has the journal's lock. One `ledgerd` per journal (ADR 0005).
    JournalInUse { lock: PathBuf },
    /// The journal's lock file could not be opened or locked.
    Lock(std::io::Error),
```

Add their arms to `impl fmt::Display for StoreError`, after the `NoSuchIntent` arm:

```rust
            StoreError::JournalInUse { lock } => write!(
                f,
                "journal in use by another process (lock held on {})",
                lock.display()
            ),
            StoreError::Lock(e) => write!(f, "journal lock: {e}"),
```

Replace `pub struct Store` and the start of `impl Store`, up to and including `with_schema`, with:

```rust
pub struct Store {
    conn: Connection,
    /// The journal's lock, held for as long as the store lives. The kernel
    /// releases it when the process dies, however it dies.
    _lock: Option<File>,
}

impl Store {
    /// Open (or create) the journal at `path`.
    ///
    /// It takes the journal's exclusive lock first (ADR 0005), before reading
    /// anything. If another store already holds that lock, in this process or
    /// any other, this fails with [`StoreError::JournalInUse`] and touches
    /// nothing.
    ///
    /// WAL mode with `synchronous = FULL`: a committed append survives the
    /// process being killed at any instant, which is the failure the
    /// crash-recovery test produces.
    pub fn open(path: &Path) -> Result<Store> {
        let lock = lock_journal(path)?;
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        Store::with_schema(conn, Some(lock))
    }

    /// A journal that lives only as long as this value. No lock: nothing else can reach it.
    pub fn open_in_memory() -> Result<Store> {
        Store::with_schema(Connection::open_in_memory()?, None)
    }

    fn with_schema(conn: Connection, lock: Option<File>) -> Result<Store> {
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn, _lock: lock })
    }
```

Add this free function after the `impl Store` block:

```rust
/// Take the journal's exclusive lock without waiting, or refuse.
///
/// The lock lives on a sidecar file, `<journal>.lock`, never on the journal
/// itself. SQLite holds POSIX locks on the journal, and those are dropped when
/// any other descriptor for the same file is closed.
fn lock_journal(path: &Path) -> Result<File> {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    let lock = PathBuf::from(name);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock)
        .map_err(StoreError::Lock)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(StoreError::JournalInUse { lock }),
        Err(TryLockError::Error(e)) => Err(StoreError::Lock(e)),
    }
}
```

- [ ] **Step 4: Run the store's tests**

Run: `nix develop -c bash -o pipefail -c 'cargo test -p ledger-store 2>&1 | grep -E "^(test |test result)"'`
Expected: the three new tests pass, and so do the existing `journal` and `effects` tests.

- [ ] **Step 5: Commit:** `ledger-store: one holder per journal (ADR 0005)`, with a trailer.

---

### Task 2: A second `ledgerd` refuses, and the first is untouched

- [ ] **Step 1: Write the test**

<!-- file: crates/ledgerd/tests/one_per_journal.rs -->
```rust
//! A second `ledgerd` on a journal that is already in use exits without
//! writing, and the first one's job finishes on its first attempt
//! (ADR 0005; report 05, findings 1 and 2).

mod common;

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{http, json, start, wait_for};

#[test]
fn a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.db");

    // A is running a job whose effect takes 2 seconds.
    let a = start(&db);
    let body = r#"{"idempotency_key":"held","kind":{"SleepThenRecord":{"ms":2000,"token":"held"}},"max_attempts":3}"#;
    let (code, reply) = http(&a.addr, "POST", "/jobs", Some(body));
    assert_eq!(code, 201, "{reply}");
    let job = json(&reply)["job"].as_u64().unwrap();
    wait_for(&a.addr, job, Duration::from_secs(10), |s| {
        s["effect"]["InFlight"].is_object()
    });

    // B is pointed at the same journal while A's effect is running.
    let mut b = Command::new(env!("CARGO_BIN_EXE_ledgerd"))
        .env("LEDGER_DB", &db)
        .env("LEDGER_ADDR", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ledgerd starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    let exit = loop {
        if let Some(exit) = b.try_wait().unwrap() {
            break exit;
        }
        if Instant::now() > deadline {
            let _ = b.kill();
            let _ = b.wait();
            panic!("B is still running: it started on a journal that was already in use");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    b.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
    b.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    println!("B exited with {exit}; stderr: {stderr}");
    assert!(!exit.success(), "B must exit non-zero");
    assert!(stderr.contains("JournalInUse"), "B names the cause: {stderr}");
    assert!(stdout.is_empty(), "B never got as far as listening: {stdout}");

    // A's job finishes on its first attempt: B never expired A's lease.
    let status = wait_for(&a.addr, job, Duration::from_secs(10), |s| {
        s["job"]["state"]["Succeeded"].is_object()
    });
    assert_eq!(status["job"]["attempts"], 1, "{status}");
    assert_eq!(status["effect"]["Finished"]["attempt"], 1, "{status}");
}
```

- [ ] **Step 2: Prove the test catches the old behavior**

Before Task 1's change reached `ledgerd`, this test must fail. Show that it would:

```bash
git worktree add --detach target/plan05-before origin/main
cp crates/ledgerd/tests/one_per_journal.rs target/plan05-before/crates/ledgerd/tests/
nix develop -c bash -o pipefail -c 'cd target/plan05-before && cargo test -p ledgerd --test one_per_journal -- --nocapture 2>&1 | tail -15' | tee target/plan05-red.txt
git worktree remove --force target/plan05-before
```

Expected: FAIL with `B is still running: it started on a journal that was already in use`. Quote this in the report under Measured as the red run.

- [ ] **Step 3: Run it against this branch**

Run: `nix develop -c bash -o pipefail -c 'cargo test -p ledgerd --test one_per_journal -- --nocapture 2>&1 | tail -15'`
Expected: PASS. The `println!` shows B's non-zero exit and an error naming `JournalInUse`.

- [ ] **Step 4: Update `main.rs`'s doc comment**

In `crates/ledgerd/src/main.rs`, replace the first four lines with:

```rust
//! `ledgerd`: serve the ledger over HTTP and run its jobs.
//!
//! LEDGER_DB    path to the journal file (required). One `ledgerd` per journal:
//!              a second process on the same file exits with `JournalInUse` (ADR 0005).
//! LEDGER_ADDR  address to listen on (default 127.0.0.1:8080; port 0 picks a free port)
```

- [ ] **Step 5: Commit:** `ledgerd: a second process on a journal in use refuses to start`, with a trailer.

---

### Task 3: Gates, in CI's order, plus the crash test

- [ ] **Step 1: Run every gate**

```bash
mkdir -p target && nix develop -c bash -o pipefail -c '
  echo "\$ ./scripts/ratchets/all.sh"; ./scripts/ratchets/all.sh; echo "exit $?"
  echo; echo "\$ ./scripts/ratchets/test-ratchets.sh"; ./scripts/ratchets/test-ratchets.sh; echo "exit $?"
  echo; echo "\$ cargo fmt --all -- --check"; cargo fmt --all -- --check && echo "(formatted)"; echo "exit $?"
  echo; echo "\$ cargo clippy --workspace --all-targets -- -D warnings"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3; echo "exit $?"
  echo; echo "\$ cargo test --workspace"; cargo test --workspace 2>&1 | grep -E "^(test result|running|test )"; echo "exit $?"
  echo; echo "\$ cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture"; cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture 2>&1 | grep -vE "^\s+(Compiling|Finished|Running)"; echo "exit $?"
' 2>&1 | grep -v "^warning: Git tree" > target/plan05-gates.txt; cat target/plan05-gates.txt
```

Expected:
- all 5 ratchets hold
- the ratchet self-tests pass
- the code is formatted and clippy is clean
- every test passes, including the 3 new store tests and the new `ledgerd` test
- the crash test still passes: `kill -9` releases the lock, so the restarted process takes it

---

### Task 4: Brief 05's harness, as the regression check

- [ ] **Step 1: Run case 3 three times, and the same-addr variant once**

```bash
nix develop -c bash -o pipefail -c '
  H=docs/pipeline/reports/2026-10-14-05-evidence/measure.sh
  for run in 1 2 3; do echo "### two-processes $run"; bash "$H" two-processes "$run"; echo "exit $?"; done
  echo "### two-processes 4 same-addr"; bash "$H" two-processes 4 same-addr; echo "exit $?"
' 2>&1 | grep -v "^warning: Git tree" > target/plan05-case3.txt; cat target/plan05-case3.txt
```

Expected: in every run, B exits at once with `JournalInUse`. The job ends with 1 lease, 1 intent, 1 outcome and 1 `Succeeded` event. In the same-addr run, nothing is appended for A's job and no attempt is marked `abandoned`. Brief 05 measured the opposite in 3 runs out of 3.

The harness was written for a B that starts. If it waits, times out, or prints something misleading now that B refuses, quote what it actually printed and explain it. Do not edit the harness.

---

### Task 5: Report, PR

- [ ] **Step 1: Write `docs/pipeline/reports/2026-10-14-05-one-ledgerd-per-journal.md`**

Shape it like reports 01–04.

**Measured:**
- the red run from Task 2, Step 2
- the verbatim gate output from Task 3
- the harness output from Task 4, set beside brief 05's numbers for the same cases

**Inferred:**
- The lock is host-local, and not reliable on a network filesystem.
- Report 05's findings 3 to 6 are not addressed. Say which of them a single writer turns from a live bug into a known limit.
- Anything else this does not prove.

**Deviations from the plan.**

- [ ] **Step 2: Commit, push and open the PR**

```bash
git add docs/pipeline/reports/2026-10-14-05-one-ledgerd-per-journal.md
git commit -m "Plan 05: live-proof report

Co-Authored-By: <your model> <noreply@anthropic.com>"
git push -u origin HEAD
gh pr create --base main --title "Plan 05: one ledgerd per journal" --body "Executes docs/pipeline/plans/2026-10-14-05-one-ledgerd-per-journal.md (ADR 0005; report 05, findings 1 and 2). Live proof: docs/pipeline/reports/2026-10-14-05-one-ledgerd-per-journal.md."
```

Do not merge.
