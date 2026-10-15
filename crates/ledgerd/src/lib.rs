//! The ledger server: HTTP in front, the executor behind, the journal
//! underneath. Everything that touches a clock, a socket or a disk lives
//! here. Every decision is made by `ledger-core`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path as UrlPath, State as Shared};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ledger_api::{
    Command, EffectStatus, Event, JobId, JobKind, JobState, JobStatus, LeaseId, Millis, Refusal,
    SubmitJob, SubmitReply,
};
use ledger_core::{EffectRecord, State, apply, decide, recover};
use ledger_store::{Intent, Store, StoreError};
use tokio::sync::Mutex;

/// How long the executor holds a lease before the ledger may expire it.
pub const LEASE_FOR_MS: u64 = 30_000;
const IDLE_POLL: Duration = Duration::from_millis(100);

pub type SharedLedger = Arc<Mutex<Ledger>>;

/// The time now, as the core takes it. This is the only clock read in the ledger.
pub fn now() -> Millis {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Millis(u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX))
}

#[derive(Debug)]
pub enum LedgerError {
    Refused(Refusal),
    Store(StoreError),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerError::Refused(refusal) => write!(f, "refused: {refusal:?}"),
            LedgerError::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<Refusal> for LedgerError {
    fn from(refusal: Refusal) -> Self {
        LedgerError::Refused(refusal)
    }
}

impl From<StoreError> for LedgerError {
    fn from(e: StoreError) -> Self {
        LedgerError::Store(e)
    }
}

/// One leased job, ready for the executor.
pub struct Work {
    pub job: JobId,
    pub lease: LeaseId,
    pub attempt: u32,
    pub kind: JobKind,
}

/// The in-memory state, and the journal it is rebuilt from.
pub struct Ledger {
    state: State,
    store: Store,
}

impl Ledger {
    /// Open the journal, replay it, then recover from whatever the last
    /// process left unfinished.
    pub fn open(path: &Path, now: Millis) -> Result<Ledger, LedgerError> {
        let store = Store::open(path)?;
        let mut state = State::default();
        for event in store.events()? {
            apply(&mut state, &event);
        }
        let mut ledger = Ledger { state, store };
        ledger.recover_from_restart(now)?;
        Ok(ledger)
    }

    fn recover_from_restart(&mut self, now: Millis) -> Result<(), LedgerError> {
        let unfinished: BTreeSet<(JobId, u32)> = self
            .store
            .unfinished()?
            .into_iter()
            .map(|intent| (intent.job, intent.attempt))
            .collect();
        let mut known = BTreeMap::new();
        for job in self
            .state
            .jobs()
            .filter(|job| matches!(job.state, JobState::Leased { .. }))
        {
            let key = (job.id, job.attempts);
            if unfinished.contains(&key) {
                known.insert(key, EffectRecord::Unfinished);
            } else if let Some(outcome) = self.store.outcome(job.id, job.attempts)? {
                known.insert(key, EffectRecord::Finished(outcome));
            }
        }
        let events = recover(&self.state, now, |job, attempt| {
            known.get(&(job, attempt)).cloned()
        });
        if !events.is_empty() {
            eprintln!(
                "ledgerd: recovered {} leased job(s) after a restart",
                events.len()
            );
        }
        self.record(&events)?;
        for (job, attempt) in unfinished {
            self.store.record_outcome(
                job,
                attempt,
                "abandoned: the process restarted before the effect reported back",
            )?;
        }
        Ok(())
    }

    /// Journal first, then apply: memory never runs ahead of the disk.
    fn record(&mut self, events: &[Event]) -> Result<(), LedgerError> {
        self.store.append(events)?;
        for event in events {
            apply(&mut self.state, event);
        }
        Ok(())
    }

    fn execute(&mut self, command: Command) -> Result<Vec<Event>, LedgerError> {
        let events = decide(&self.state, command)?;
        self.record(&events)?;
        Ok(events)
    }

    pub fn submit(&mut self, request: SubmitJob) -> Result<SubmitReply, LedgerError> {
        let key = request.idempotency_key.clone();
        let events = self.execute(Command::Submit {
            idempotency_key: request.idempotency_key,
            kind: request.kind,
            max_attempts: request.max_attempts,
        })?;
        let job = self
            .state
            .job_for_key(&key)
            .expect("an accepted submission always has a job");
        Ok(SubmitReply {
            job,
            created: !events.is_empty(),
        })
    }

    pub fn status(&self, job: JobId) -> Result<JobStatus, LedgerError> {
        let found = self
            .state
            .job(job)
            .ok_or(Refusal::UnknownJob { job })?
            .clone();
        let attempt = found.attempts;
        let effect = if attempt == 0 {
            EffectStatus::NotStarted
        } else if self
            .store
            .unfinished()?
            .iter()
            .any(|intent| intent.job == job && intent.attempt == attempt)
        {
            EffectStatus::InFlight { attempt }
        } else if self.store.outcome(job, attempt)?.is_some() {
            EffectStatus::Finished { attempt }
        } else {
            EffectStatus::NotStarted
        };
        Ok(JobStatus { job: found, effect })
    }

    /// Expire stale leases, then lease the next due job, if there is one.
    pub fn next_work(&mut self, now: Millis) -> Result<Option<Work>, LedgerError> {
        self.execute(Command::ExpireLeases { now })?;
        let events = self.execute(Command::Lease {
            now,
            lease_for_ms: LEASE_FOR_MS,
        })?;
        Ok(events.into_iter().find_map(|event| match event {
            Event::Leased {
                job,
                lease,
                attempt,
                ..
            } => {
                let kind = self.state.job(job)?.kind.clone();
                Some(Work {
                    job,
                    lease,
                    attempt,
                    kind,
                })
            }
            _ => None,
        }))
    }

    /// Journal the intent before the effect runs (ADR 0003).
    pub fn start_effect(&mut self, work: &Work, now: Millis) -> Result<(), LedgerError> {
        self.store.record_intent(&Intent {
            job: work.job,
            attempt: work.attempt,
            at: now,
        })?;
        Ok(())
    }

    /// Journal the outcome, then complete the job.
    pub fn finish_effect(&mut self, work: &Work, outcome: String) -> Result<(), LedgerError> {
        self.store
            .record_outcome(work.job, work.attempt, &outcome)?;
        self.execute(Command::Complete {
            job: work.job,
            lease: work.lease,
            result: outcome,
        })?;
        Ok(())
    }
}

/// Perform a job's effect and return what to record as its outcome.
async fn perform(kind: &JobKind) -> String {
    match kind {
        JobKind::SleepThenRecord { ms, token } => {
            tokio::time::sleep(Duration::from_millis(*ms)).await;
            token.clone()
        }
        JobKind::Notify { message } => {
            eprintln!("ledgerd: notify: {message}");
            format!("notified: {message}")
        }
    }
}

/// Run jobs forever: lease, journal the intent, perform, journal the outcome, complete.
pub async fn run_executor(ledger: SharedLedger) {
    loop {
        let next = ledger.lock().await.next_work(now());
        match next {
            Ok(Some(work)) => {
                if let Err(e) = ledger.lock().await.start_effect(&work, now()) {
                    eprintln!("ledgerd: executor: {e}");
                    continue;
                }
                let outcome = perform(&work.kind).await;
                if let Err(e) = ledger.lock().await.finish_effect(&work, outcome) {
                    eprintln!("ledgerd: executor: {e}");
                }
            }
            Ok(None) => tokio::time::sleep(IDLE_POLL).await,
            Err(e) => {
                eprintln!("ledgerd: executor: {e}");
                tokio::time::sleep(IDLE_POLL).await;
            }
        }
    }
}

pub fn router(ledger: SharedLedger) -> Router {
    Router::new()
        .route("/jobs", post(submit))
        .route("/jobs/{id}", get(status))
        .with_state(ledger)
}

async fn submit(Shared(ledger): Shared<SharedLedger>, Json(request): Json<SubmitJob>) -> Response {
    match ledger.lock().await.submit(request) {
        Ok(reply) => {
            let code = if reply.created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (code, Json(reply)).into_response()
        }
        Err(e) => error_response(e),
    }
}

async fn status(Shared(ledger): Shared<SharedLedger>, UrlPath(id): UrlPath<u64>) -> Response {
    match ledger.lock().await.status(JobId(id)) {
        Ok(status) => Json(status).into_response(),
        Err(e) => error_response(e),
    }
}

fn error_response(error: LedgerError) -> Response {
    match error {
        LedgerError::Refused(refusal @ Refusal::UnknownJob { .. }) => {
            (StatusCode::NOT_FOUND, Json(refusal)).into_response()
        }
        LedgerError::Refused(refusal) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(refusal)).into_response()
        }
        LedgerError::Store(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
