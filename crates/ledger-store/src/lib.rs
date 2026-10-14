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
    NoSuchIntent {
        job: JobId,
        attempt: u32,
    },
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
        let mut select = self
            .conn
            .prepare_cached("SELECT body FROM events ORDER BY seq")?;
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
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut intents = Vec::new();
        for row in rows {
            let (job, attempt, at) = row?;
            intents.push(Intent {
                job: JobId(from_sql(job)?),
                attempt,
                at: Millis(from_sql(at)?),
            });
        }
        Ok(intents)
    }
}

/// SQLite integers are signed. An id or a time above `i64::MAX` is refused
/// rather than wrapped.
fn to_sql(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|e| StoreError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))
}

fn from_sql(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|e| {
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(e),
        ))
    })
}
