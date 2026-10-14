//! What a restart does to a job that was leased when the last process died.

use ledger_api::{Command, Event, JobId, JobKind, Millis};
use ledger_core::{EffectRecord, State, apply, decide, recover};

fn sleep() -> JobKind {
    JobKind::SleepThenRecord {
        ms: 1,
        token: "t".into(),
    }
}

fn notify() -> JobKind {
    JobKind::Notify {
        message: "hi".into(),
    }
}

fn run(state: &mut State, command: Command) {
    for event in decide(state, command).unwrap() {
        apply(state, &event);
    }
}

/// One job, submitted and leased at time 0.
fn leased(kind: JobKind, max_attempts: u32) -> State {
    let mut state = State::default();
    run(
        &mut state,
        Command::Submit {
            idempotency_key: "a".into(),
            kind,
            max_attempts,
        },
    );
    run(
        &mut state,
        Command::Lease {
            now: Millis(0),
            lease_for_ms: 30_000,
        },
    );
    state
}

#[test]
fn a_finished_effect_completes_without_running_again() {
    let state = leased(sleep(), 3);
    let events = recover(&state, Millis(100), |_, _| {
        Some(EffectRecord::Finished("t".into()))
    });
    assert_eq!(
        events,
        vec![Event::Succeeded {
            job: JobId(0),
            result: "t".into()
        }]
    );
}

#[test]
fn an_unfinished_idempotent_effect_is_re_driven() {
    let state = leased(sleep(), 3);
    let events = recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished));
    assert_eq!(
        events,
        vec![Event::Retrying {
            job: JobId(0),
            not_before: Millis(1_100),
            reason: "restarted mid-attempt".into()
        }]
    );
}

#[test]
fn an_unfinished_non_idempotent_effect_is_killed_not_repeated() {
    let state = leased(notify(), 3);
    let events = recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished));
    assert!(
        matches!(&events[..], [Event::Died { job: JobId(0), reason }] if reason.contains("not idempotent")),
        "{events:?}"
    );
}

#[test]
fn a_lease_whose_effect_never_started_is_retried_whatever_its_kind() {
    let state = leased(notify(), 3);
    let events = recover(&state, Millis(100), |_, _| None);
    assert!(
        matches!(&events[..], [Event::Retrying { job: JobId(0), .. }]),
        "{events:?}"
    );
}

#[test]
fn the_last_attempt_dies_instead_of_retrying() {
    let state = leased(sleep(), 1);
    let events = recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished));
    assert!(
        matches!(&events[..], [Event::Died { job: JobId(0), .. }]),
        "{events:?}"
    );
}

#[test]
fn jobs_that_are_not_leased_are_left_alone() {
    let mut state = State::default();
    run(
        &mut state,
        Command::Submit {
            idempotency_key: "a".into(),
            kind: sleep(),
            max_attempts: 3,
        },
    );
    assert_eq!(
        recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished)),
        vec![]
    );
}
