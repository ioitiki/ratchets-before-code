//! Worked examples of every rule the core enforces.

use ledger_api::{Command, Event, JobId, JobKind, JobState, LeaseId, Millis, Refusal};
use ledger_core::{State, apply, decide};

fn submit(key: &str, max_attempts: u32) -> Command {
    Command::Submit {
        idempotency_key: key.into(),
        kind: JobKind::SleepThenRecord {
            ms: 1,
            token: key.into(),
        },
        max_attempts,
    }
}

/// Decide, then apply what was decided: what the server will do with every command.
fn run(state: &mut State, command: Command) -> Result<Vec<Event>, Refusal> {
    let events = decide(state, command)?;
    for event in &events {
        apply(state, event);
    }
    Ok(events)
}

#[test]
fn a_duplicate_submission_records_nothing() {
    let mut state = State::default();
    assert_eq!(run(&mut state, submit("a", 3)).unwrap().len(), 1);
    assert_eq!(run(&mut state, submit("a", 3)).unwrap(), vec![]);
    assert_eq!(state.jobs().count(), 1);
    assert_eq!(state.job_for_key("a"), Some(JobId(0)));
}

#[test]
fn zero_attempts_is_refused() {
    assert_eq!(
        decide(&State::default(), submit("a", 0)),
        Err(Refusal::MaxAttemptsMustBePositive)
    );
}

#[test]
fn nothing_due_means_nothing_leased() {
    let lease = Command::Lease {
        now: Millis(0),
        lease_for_ms: 1_000,
    };
    assert_eq!(decide(&State::default(), lease), Ok(vec![]));
}

#[test]
fn a_completed_job_records_its_result() {
    let mut state = State::default();
    run(&mut state, submit("a", 1)).unwrap();
    run(
        &mut state,
        Command::Lease {
            now: Millis(0),
            lease_for_ms: 1_000,
        },
    )
    .unwrap();
    run(
        &mut state,
        Command::Complete {
            job: JobId(0),
            lease: LeaseId(0),
            result: "a".into(),
        },
    )
    .unwrap();
    assert_eq!(
        state.job(JobId(0)).unwrap().state,
        JobState::Succeeded { result: "a".into() }
    );
}

#[test]
fn a_failure_retries_with_backoff_until_attempts_run_out() {
    let mut state = State::default();
    let job = JobId(0);
    run(&mut state, submit("a", 2)).unwrap();

    run(
        &mut state,
        Command::Lease {
            now: Millis(0),
            lease_for_ms: 5_000,
        },
    )
    .unwrap();
    let failed = Command::Fail {
        job,
        lease: LeaseId(0),
        reason: "boom".into(),
        now: Millis(10),
    };
    assert_eq!(
        run(&mut state, failed).unwrap(),
        vec![Event::Retrying {
            job,
            not_before: Millis(1_010),
            reason: "boom".into()
        }]
    );

    let too_early = Command::Lease {
        now: Millis(500),
        lease_for_ms: 5_000,
    };
    assert_eq!(
        run(&mut state, too_early).unwrap(),
        vec![],
        "not due before its backoff"
    );

    run(
        &mut state,
        Command::Lease {
            now: Millis(1_010),
            lease_for_ms: 5_000,
        },
    )
    .unwrap();
    let failed_again = Command::Fail {
        job,
        lease: LeaseId(1),
        reason: "boom".into(),
        now: Millis(1_020),
    };
    assert_eq!(
        run(&mut state, failed_again).unwrap(),
        vec![Event::Died {
            job,
            reason: "boom (after 2 of 2 attempts)".into()
        }]
    );
}

#[test]
fn an_expired_lease_is_retried_and_its_holder_is_refused() {
    let mut state = State::default();
    let job = JobId(0);
    run(&mut state, submit("a", 3)).unwrap();
    run(
        &mut state,
        Command::Lease {
            now: Millis(0),
            lease_for_ms: 1_000,
        },
    )
    .unwrap();

    assert_eq!(
        run(&mut state, Command::ExpireLeases { now: Millis(1_000) }).unwrap(),
        vec![Event::Retrying {
            job,
            not_before: Millis(2_000),
            reason: "lease expired".into()
        }]
    );
    assert_eq!(
        run(
            &mut state,
            Command::Complete {
                job,
                lease: LeaseId(0),
                result: "late".into()
            }
        ),
        Err(Refusal::StaleLease {
            job,
            lease: LeaseId(0)
        })
    );
}

#[test]
fn an_unknown_job_is_refused() {
    let complete = Command::Complete {
        job: JobId(9),
        lease: LeaseId(0),
        result: "x".into(),
    };
    assert_eq!(
        decide(&State::default(), complete),
        Err(Refusal::UnknownJob { job: JobId(9) })
    );
}
