# Plan 03: ledger-store, the journal

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`../specs/2026-10-14-ratchets-before-code-design.md`](../specs/2026-10-14-ratchets-before-code-design.md), build step 3 (§7), and §4.2.
**Executed by:** a worker agent in its own Orca worktree. It ends with a PR that the coordinator verifies and merges.

**Goal:** `ledger-store` persists the ledger in SQLite. It keeps:
- an append-only event journal, with replay in order
- effect records: an **intent** written before a job runs, and an **outcome** written after

It is the only crate that may contain SQL.

**Architecture:**
- `Store` wraps one `rusqlite::Connection`. Events are serialized as JSON, using `ledger-api`'s serde derives; the store itself derives nothing.
- A batch of events is appended in one transaction.
- `ledger-store` does **not** depend on `ledger-core`. It hands back `Vec<Event>`, and the caller folds them with `ledger_core::apply`. The store holds rows; it makes no decisions.

**Tech stack:**
- `rusqlite` with the `bundled` feature, so SQLite builds from source with no system library
- `serde_json`
- `tempfile` and `proptest` as dev-dependencies

**Rules for the worker:** the same as plan 02, and binding:
- clean-room: only this repository
- never weaken a ratchet; if one blocks you, stop and report it
- check the noreply commit identity
- add a `Co-Authored-By` trailer to every commit
- run every command in `nix develop -c …`
- the report quotes real output, keeps Measured separate from Inferred, and lists every deviation
- write gate captures under `target/`, which is gitignored and inside the worktree; never `/tmp`
- **run gate scripts under `bash -o pipefail`,** so a piped command reports its own exit status (report 02, deviation 3)

---

## File map

| Path | Responsibility |
|---|---|
| `Cargo.toml` | add `rusqlite` and `tempfile` to `[workspace.dependencies]` |
| `crates/ledger-store/Cargo.toml` | manifest |
| `crates/ledger-store/src/lib.rs` | `Store`, `StoreError`, `Intent`; open, append, replay, intents and outcomes |
| `crates/ledger-store/src/schema.sql` | the schema, applied on open |
| `crates/ledger-store/tests/journal.rs` | append/replay, persistence across reopen, and a property: any batching replays identically |
| `crates/ledger-store/tests/effects.rs` | intent/outcome bookkeeping and persistence across reopen |
| `docs/decisions/0003-the-journal-records-intent-before-outcome.md` | ADR 0003 |
| `docs/pipeline/reports/2026-10-14-03-ledger-store.md` | live proof |

---

### Task 1: Manifest and failing journal tests

- [ ] **Step 1: Add the workspace dependencies**

In the root `Cargo.toml`, add these lines under `[workspace.dependencies]`, keeping the existing entries:

```toml
rusqlite = { version = "0.37", features = ["bundled"] }
tempfile = "3"
```

If `0.37` does not resolve, use the newest `rusqlite` release `cargo search rusqlite` shows, and record the version you used as a deviation.

- [ ] **Step 2: Write the manifest**

<!-- file: crates/ledger-store/Cargo.toml -->
```toml
[package]
name = "ledger-store"
version = "0.1.0"
description = "The ledger's journal in SQLite: the only crate that contains SQL."
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
ledger-api.workspace = true
rusqlite.workspace = true
serde_json.workspace = true

[dev-dependencies]
proptest.workspace = true
tempfile.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: Write the failing journal tests**

<!-- file: crates/ledger-store/tests/journal.rs -->
```rust
//! The journal gives back exactly what was appended, in order, across restarts.

use ledger_api::{Event, JobId, JobKind, LeaseId, Millis};
use ledger_store::Store;
use proptest::prelude::*;

fn sample() -> Vec<Event> {
    vec![
        Event::Submitted {
            job: JobId(0),
            idempotency_key: "k".into(),
            kind: JobKind::SleepThenRecord { ms: 5, token: "t".into() },
            max_attempts: 3,
        },
        Event::Leased { job: JobId(0), lease: LeaseId(0), attempt: 1, expires: Millis(1_000) },
        Event::Succeeded { job: JobId(0), result: "t".into() },
    ]
}

#[test]
fn an_empty_journal_replays_nothing() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.events().unwrap(), vec![]);
}

#[test]
fn events_replay_in_append_order() {
    let mut store = Store::open_in_memory().unwrap();
    let events = sample();
    store.append(&events[..1]).unwrap();
    store.append(&events[1..]).unwrap();
    assert_eq!(store.events().unwrap(), events);
}

#[test]
fn the_journal_survives_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.db");
    {
        let mut store = Store::open(&path).unwrap();
        store.append(&sample()).unwrap();
    }
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.events().unwrap(), sample());
}

fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        (0u64..50, "[a-z]{0,8}").prop_map(|(job, result)| Event::Succeeded { job: JobId(job), result }),
        (0u64..50, 0u64..1_000_000, "[a-z ]{0,12}").prop_map(|(job, at, reason)| Event::Retrying {
            job: JobId(job),
            not_before: Millis(at),
            reason,
        }),
        (0u64..50, "[a-z]{0,8}").prop_map(|(job, reason)| Event::Died { job: JobId(job), reason }),
    ]
}

proptest! {
    #[test]
    fn any_batching_replays_the_same_sequence(
        events in prop::collection::vec(event(), 0..40),
        cuts in prop::collection::vec(0usize..40, 0..6),
    ) {
        let mut store = Store::open_in_memory().unwrap();
        let mut cuts: Vec<usize> = cuts.into_iter().map(|c| c.min(events.len())).collect();
        cuts.push(0);
        cuts.push(events.len());
        cuts.sort_unstable();
        for pair in cuts.windows(2) {
            store.append(&events[pair[0]..pair[1]]).unwrap();
        }
        prop_assert_eq!(store.events().unwrap(), events);
    }
}
```

- [ ] **Step 4: Run them and watch them fail**

Run: `nix develop -c cargo test -p ledger-store --test journal`
Expected: FAIL, with an unresolved import of `ledger_store` (there is no library yet).

---

### Task 2: The store

- [ ] **Step 1: Write the schema and the crate**

<!-- file: crates/ledger-store/src/schema.sql -->
```sql
-- The ledger's journal. Applied on every open; every statement is idempotent.

-- Every event the ledger has recorded, in order. Rows are never updated or
-- deleted: the journal is the history, and state is rebuilt from it.
CREATE TABLE IF NOT EXISTS events (
    seq  INTEGER PRIMARY KEY AUTOINCREMENT,
    body TEXT NOT NULL
);

-- One row per job attempt that started an effect. `intent_at` is written
-- before the effect runs; `outcome` is written after it finishes. A row with
-- no outcome is an effect whose fate is unknown (ADR 0003).
CREATE TABLE IF NOT EXISTS effects (
    job       INTEGER NOT NULL,
    attempt   INTEGER NOT NULL,
    intent_at INTEGER NOT NULL,
    outcome   TEXT,
    PRIMARY KEY (job, attempt)
);
```

<!-- file: crates/ledger-store/src/lib.rs -->
```rust
//! The ledger's journal in SQLite. This is the only crate that contains SQL;
//! `scripts/ratchets/sql-containment.sh` fails the build if any leaks out.
//!
//! The store keeps rows and makes no decisions. It hands events back in order,
//! and the caller folds them with `ledger_core::apply`.

use std::fmt;
use std::path::Path;

use ledger_api::{Event, JobId, Millis};
use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA: &str = include_str!("schema.sql");

/// An effect that was started: its intent was recorded before it ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub job: JobId,
    pub attempt: u32,
    pub at: Millis,
}

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// A journaled event could not be encoded or decoded.
    Json(serde_json::Error),
    /// An outcome was recorded for an effect that never recorded an intent.
    NoSuchIntent { job: JobId, attempt: u32 },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "sqlite: {e}"),
            StoreError::Json(e) => write!(f, "journal encoding: {e}"),
            StoreError::NoSuchIntent { job, attempt } => {
                write!(f, "no intent recorded for job {} attempt {attempt}", job.0)
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) the journal at `path`.
    ///
    /// WAL mode with `synchronous = FULL`: a committed append survives the
    /// process being killed at any instant, which is the failure the
    /// crash-recovery test produces.
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        Store::with_schema(conn)
    }

    /// A journal that lives only as long as this value.
    pub fn open_in_memory() -> Result<Store> {
        Store::with_schema(Connection::open_in_memory()?)
    }

    fn with_schema(conn: Connection) -> Result<Store> {
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    /// Append events in one transaction: all of them are recorded, or none are.
    pub fn append(&mut self, events: &[Event]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut insert = tx.prepare_cached("INSERT INTO events (body) VALUES (?1)")?;
            for event in events {
                insert.execute(params![serde_json::to_string(event)?])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every event ever appended, in append order.
    pub fn events(&self) -> Result<Vec<Event>> {
        let mut select = self.conn.prepare_cached("SELECT body FROM events ORDER BY seq")?;
        let bodies = select.query_map([], |row| row.get::<_, String>(0))?;
        let mut events = Vec::new();
        for body in bodies {
            events.push(serde_json::from_str(&body?)?);
        }
        Ok(events)
    }

    /// Record that an effect is about to run. Recording the same attempt twice
    /// is an error: an attempt starts its effect once.
    pub fn record_intent(&mut self, intent: &Intent) -> Result<()> {
        self.conn.execute(
            "INSERT INTO effects (job, attempt, intent_at) VALUES (?1, ?2, ?3)",
            params![to_sql(intent.job.0)?, intent.attempt, to_sql(intent.at.0)?],
        )?;
        Ok(())
    }

    /// Record how an effect ended. The intent must already exist.
    pub fn record_outcome(&mut self, job: JobId, attempt: u32, outcome: &str) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE effects SET outcome = ?3 WHERE job = ?1 AND attempt = ?2",
            params![to_sql(job.0)?, attempt, outcome],
        )?;
        if changed == 0 {
            return Err(StoreError::NoSuchIntent { job, attempt });
        }
        Ok(())
    }

    /// The recorded outcome of one attempt's effect, if it finished.
    pub fn outcome(&self, job: JobId, attempt: u32) -> Result<Option<String>> {
        let outcome = self
            .conn
            .query_row(
                "SELECT outcome FROM effects WHERE job = ?1 AND attempt = ?2",
                params![to_sql(job.0)?, attempt],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        Ok(outcome.flatten())
    }

    /// Effects that started and never recorded an outcome, oldest first.
    /// After a crash, these are the ones whose fate is unknown.
    pub fn unfinished(&self) -> Result<Vec<Intent>> {
        let mut select = self.conn.prepare_cached(
            "SELECT job, attempt, intent_at FROM effects WHERE outcome IS NULL ORDER BY intent_at, job, attempt",
        )?;
        let rows = select.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?, row.get::<_, i64>(2)?))
        })?;
        let mut intents = Vec::new();
        for row in rows {
            let (job, attempt, at) = row?;
            intents.push(Intent { job: JobId(from_sql(job)?), attempt, at: Millis(from_sql(at)?) });
        }
        Ok(intents)
    }
}

/// SQLite integers are signed. An id or a time above `i64::MAX` is refused
/// rather than wrapped.
fn to_sql(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|e| StoreError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))
}

fn from_sql(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|e| {
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(e)))
    })
}
```

- [ ] **Step 2: Run the journal tests and watch them pass**

Run: `nix develop -c cargo test -p ledger-store --test journal`
Expected: 4 tests pass (3 examples and 1 property).

- [ ] **Step 3: The ratchets see real SQL for the first time**

Run: `nix develop -c bash -o pipefail -c './scripts/ratchets/all.sh'`
Expected: `all 5 ratchets hold`.
- `sql-containment` passes because all the SQL is in `ledger-store`.
- `wire-types-in-api` passes because the store serializes `ledger-api` types and derives nothing.

- [ ] **Step 4: Commit**

```bash
nix develop -c cargo fmt --all
git add Cargo.toml Cargo.lock crates/ledger-store
git commit -m "ledger-store: the event journal, with replay across reopen

Co-Authored-By: <your model> <noreply@anthropic.com>"
```

---

### Task 3: Effects, intent before outcome

- [ ] **Step 1: Write the effect tests**

<!-- file: crates/ledger-store/tests/effects.rs -->
```rust
//! An effect records its intent before it runs and its outcome after.
//! Whatever has an intent and no outcome is what a restart must decide about.

use ledger_api::{JobId, Millis};
use ledger_store::{Intent, Store, StoreError};

fn intent(job: u64, attempt: u32, at: u64) -> Intent {
    Intent { job: JobId(job), attempt, at: Millis(at) }
}

#[test]
fn an_intent_without_an_outcome_is_unfinished() {
    let mut store = Store::open_in_memory().unwrap();
    store.record_intent(&intent(0, 1, 10)).unwrap();
    assert_eq!(store.unfinished().unwrap(), vec![intent(0, 1, 10)]);
    assert_eq!(store.outcome(JobId(0), 1).unwrap(), None);
}

#[test]
fn an_outcome_finishes_its_intent() {
    let mut store = Store::open_in_memory().unwrap();
    store.record_intent(&intent(0, 1, 10)).unwrap();
    store.record_outcome(JobId(0), 1, "t").unwrap();
    assert_eq!(store.unfinished().unwrap(), vec![]);
    assert_eq!(store.outcome(JobId(0), 1).unwrap(), Some("t".to_string()));
}

#[test]
fn an_outcome_without_an_intent_is_refused() {
    let mut store = Store::open_in_memory().unwrap();
    let err = store.record_outcome(JobId(7), 1, "t").unwrap_err();
    assert!(matches!(err, StoreError::NoSuchIntent { job: JobId(7), attempt: 1 }), "{err}");
}

#[test]
fn an_attempt_records_its_intent_once() {
    let mut store = Store::open_in_memory().unwrap();
    store.record_intent(&intent(0, 1, 10)).unwrap();
    assert!(store.record_intent(&intent(0, 1, 20)).is_err());
}

#[test]
fn unfinished_effects_survive_a_reopen_oldest_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.db");
    {
        let mut store = Store::open(&path).unwrap();
        store.record_intent(&intent(1, 1, 30)).unwrap();
        store.record_intent(&intent(0, 1, 10)).unwrap();
        store.record_intent(&intent(2, 1, 20)).unwrap();
        store.record_outcome(JobId(2), 1, "done").unwrap();
    }
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.unfinished().unwrap(), vec![intent(0, 1, 10), intent(1, 1, 30)]);
}
```

- [ ] **Step 2: Run them**

Run: `nix develop -c cargo test -p ledger-store --test effects`
Expected: 5 tests pass. The behavior already exists from Task 2, so this step proves it; it is not a red/green cycle. If any test fails, fix the store, not the test, and record it as a deviation.

- [ ] **Step 3: Commit**

```bash
nix develop -c cargo fmt --all
git add crates/ledger-store/tests/effects.rs
git commit -m "ledger-store: effect intents and outcomes, across reopen

Co-Authored-By: <your model> <noreply@anthropic.com>"
```

---

### Task 4: ADR 0003

<!-- file: docs/decisions/0003-the-journal-records-intent-before-outcome.md -->
```markdown
# ADR 0003: The journal records intent before outcome

**Status:** accepted
**Date:** 2026-10-14
**Decided by:** Andy

## Context

Running a job does something in the world: an effect. A process can be
killed at any instant: before the effect starts, while it runs, after it
finishes but before anyone writes that down, or after. On restart, the ledger
has to know which of those happened, and the only evidence it will have is
what it wrote down before the kill.

## Decision

Every effect is recorded twice:
- an **intent**, before it runs: `ledger-store` `record_intent`
- an **outcome**, after it finishes: `record_outcome`

Every job kind declares its **effect semantics** (`Idempotent` or
`NotIdempotent`) where the kind is defined (`JobKind::effect_semantics` in
`ledger-api`).

On restart, an intent with no outcome is an effect whose fate is unknown:
- If the kind is **idempotent**, the effect is re-driven. Running it twice
  has the same result as running it once.
- If it is **not idempotent**, the job is killed, with a reason that names
  the ambiguity. It is never silently re-run.

## Consequences

- **Recovery is a lookup, not a guess.** Without the declaration, a restart
  would have to infer what is safe from the effect's code, after the fact.
  That is the one moment nobody can afford to be wrong.
- **An effect costs two writes.** The demo is not chasing throughput, and
  the alternative to paying them is not knowing.
- **Non-idempotent work dies instead of repeating.** For effects such as
  charging a card, killing the job is correct: the job surfaces in the
  ledger with a reason, and a person decides.
- **The store keeps rows; it makes no decisions.** It reports which intents
  are unfinished. The recovery rule itself lives where decisions live, which
  `ledgerd` wires up in plan 04.
```

- [ ] **Commit:** `git add docs/decisions/0003-the-journal-records-intent-before-outcome.md` and commit with a trailer.

---

### Task 5: Gates, report, PR

- [ ] **Step 1: Run every gate, in CI's order**

Run:

```bash
mkdir -p target && nix develop -c bash -o pipefail -c '
  echo "\$ ./scripts/ratchets/all.sh"; ./scripts/ratchets/all.sh; echo "exit $?"
  echo; echo "\$ ./scripts/ratchets/test-ratchets.sh"; ./scripts/ratchets/test-ratchets.sh; echo "exit $?"
  echo; echo "\$ cargo fmt --all -- --check"; cargo fmt --all -- --check && echo "(formatted)"; echo "exit $?"
  echo; echo "\$ cargo clippy --workspace --all-targets -- -D warnings"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3; echo "exit $?"
  echo; echo "\$ cargo test --workspace"; cargo test --workspace 2>&1 | grep -E "^(test result|running|test )"; echo "exit $?"
' 2>&1 | grep -v "^warning: Git tree" > target/plan03-gates.txt; cat target/plan03-gates.txt
```

Expected:
- `all 5 ratchets hold`, with the core still under budget (unchanged: this plan doesn't touch `ledger-core`)
- the self-tests pass
- fmt and clippy are clean
- every test passes, including the 9 new store tests

- [ ] **Step 2: Write `docs/pipeline/reports/2026-10-14-03-ledger-store.md`**

- **Measured:** the verbatim gate output, plus PR #1's CI run as the first run on real crates (`gh run view 38654361590`).
- **Inferred:** what these tests do not prove. At minimum: durability under a real `kill -9` (plan 04's crash test), and `synchronous = FULL` versus power loss, which nothing here exercises.
- **Deviations from the plan.**

- [ ] **Step 3: Commit, push and open the PR**

```bash
git add docs/pipeline/reports/2026-10-14-03-ledger-store.md
git commit -m "Plan 03: live-proof report

Co-Authored-By: <your model> <noreply@anthropic.com>"
git push -u origin HEAD
gh pr create --base main --title "Plan 03: ledger-store, the journal" --body "Executes docs/pipeline/plans/2026-10-14-03-ledger-store.md. Live proof: docs/pipeline/reports/2026-10-14-03-ledger-store.md."
```

Do not merge.
