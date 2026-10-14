//! The ledger's decisions, as pure functions.
//!
//! `decide` turns a command into events, or a refusal. `apply` folds an event
//! into state. Nothing here performs I/O, reads a clock or draws randomness;
//! `scripts/ratchets/core-purity.sh` fails the build if that changes.

mod backoff;
mod state;

pub use backoff::backoff_ms;
pub use state::State;

use ledger_api::{Command, Event, Job, JobId, JobState, LeaseId, Millis, Refusal};

/// Decide what a command means against the current state.
///
/// Returns the events to record. An empty list means the command was valid
/// but changes nothing: a duplicate submission, or a lease request when no
/// job is due.
pub fn decide(state: &State, command: Command) -> Result<Vec<Event>, Refusal> {
    match command {
        Command::Submit {
            idempotency_key,
            kind,
            max_attempts,
        } => {
            if max_attempts == 0 {
                return Err(Refusal::MaxAttemptsMustBePositive);
            }
            if state.job_for_key(&idempotency_key).is_some() {
                return Ok(vec![]);
            }
            let job = state.next_job_id();
            Ok(vec![Event::Submitted {
                job,
                idempotency_key,
                kind,
                max_attempts,
            }])
        }
        Command::Lease { now, lease_for_ms } => Ok(state
            .next_due(now)
            .map(|job| Event::Leased {
                job: job.id,
                lease: state.next_lease_id(),
                attempt: job.attempts + 1,
                expires: now.plus(lease_for_ms),
            })
            .into_iter()
            .collect()),
        Command::Complete { job, lease, result } => {
            holding(state, job, lease)?;
            Ok(vec![Event::Succeeded { job, result }])
        }
        Command::Fail {
            job,
            lease,
            reason,
            now,
        } => {
            let held = holding(state, job, lease)?;
            Ok(vec![after_failed_attempt(held, reason, now)])
        }
        Command::ExpireLeases { now } => Ok(state
            .expired_leases(now)
            .map(|job| after_failed_attempt(job, "lease expired".to_string(), now))
            .collect()),
    }
}

/// Fold one event into state. Events are facts, so applying one never fails.
pub fn apply(state: &mut State, event: &Event) {
    state.apply(event);
}

/// The job, if `lease` is its current lease.
fn holding(state: &State, job: JobId, lease: LeaseId) -> Result<&Job, Refusal> {
    let found = state.job(job).ok_or(Refusal::UnknownJob { job })?;
    match &found.state {
        JobState::Leased { lease: current, .. } if *current == lease => Ok(found),
        _ => Err(Refusal::StaleLease { job, lease }),
    }
}

/// A failed or expired attempt schedules a retry, or kills the job when no
/// attempts are left.
fn after_failed_attempt(job: &Job, reason: String, now: Millis) -> Event {
    if job.attempts >= job.max_attempts {
        Event::Died {
            job: job.id,
            reason: format!(
                "{reason} (after {} of {} attempts)",
                job.attempts, job.max_attempts
            ),
        }
    } else {
        Event::Retrying {
            job: job.id,
            not_before: now.plus(backoff_ms(job.attempts)),
            reason,
        }
    }
}
