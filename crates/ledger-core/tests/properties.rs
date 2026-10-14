//! Properties that must hold for every sequence of commands, not just the
//! examples someone thought of.

use std::collections::BTreeSet;

use ledger_api::{Command, Event, JobId, JobKind, JobState, LeaseId, Millis, Refusal};
use ledger_core::{State, apply, decide};
use proptest::prelude::*;

/// One step of a random scenario. Job and lease references are small numbers,
/// so steps often hit real jobs and, half the time, guess the wrong lease.
#[derive(Debug, Clone)]
enum Step {
    Submit {
        key: u8,
        max_attempts: u32,
    },
    Lease {
        advance_ms: u64,
        lease_for_ms: u64,
    },
    Complete {
        job: u8,
        use_current_lease: bool,
        guess: u64,
    },
    Fail {
        job: u8,
        use_current_lease: bool,
        guess: u64,
    },
    Expire {
        advance_ms: u64,
    },
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0u8..6, 0u32..4).prop_map(|(key, max_attempts)| Step::Submit { key, max_attempts }),
        (0u64..5_000, 1u64..10_000).prop_map(|(advance_ms, lease_for_ms)| Step::Lease {
            advance_ms,
            lease_for_ms
        }),
        (0u8..8, any::<bool>(), 0u64..8).prop_map(|(job, use_current_lease, guess)| {
            Step::Complete {
                job,
                use_current_lease,
                guess,
            }
        }),
        (0u8..8, any::<bool>(), 0u64..8).prop_map(|(job, use_current_lease, guess)| Step::Fail {
            job,
            use_current_lease,
            guess
        }),
        (0u64..20_000).prop_map(|advance_ms| Step::Expire { advance_ms }),
    ]
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec(step(), 0..80)
}

/// A scenario in progress: the state, every event recorded, and the clock.
struct Run {
    state: State,
    events: Vec<Event>,
    now: Millis,
}

impl Run {
    fn new() -> Self {
        Run {
            state: State::default(),
            events: vec![],
            now: Millis(0),
        }
    }

    fn lease_for(&self, job: JobId, use_current: bool, guess: u64) -> LeaseId {
        match self.state.job(job).map(|found| &found.state) {
            Some(JobState::Leased { lease, .. }) if use_current => *lease,
            _ => LeaseId(guess),
        }
    }

    fn command(&mut self, step: &Step) -> Command {
        match *step {
            Step::Submit { key, max_attempts } => Command::Submit {
                idempotency_key: format!("key-{key}"),
                kind: JobKind::SleepThenRecord {
                    ms: 1,
                    token: format!("t-{key}"),
                },
                max_attempts,
            },
            Step::Lease {
                advance_ms,
                lease_for_ms,
            } => {
                self.now = self.now.plus(advance_ms);
                Command::Lease {
                    now: self.now,
                    lease_for_ms,
                }
            }
            Step::Complete {
                job,
                use_current_lease,
                guess,
            } => {
                let job = JobId(u64::from(job));
                Command::Complete {
                    job,
                    lease: self.lease_for(job, use_current_lease, guess),
                    result: "done".into(),
                }
            }
            Step::Fail {
                job,
                use_current_lease,
                guess,
            } => {
                let job = JobId(u64::from(job));
                let lease = self.lease_for(job, use_current_lease, guess);
                Command::Fail {
                    job,
                    lease,
                    reason: "boom".into(),
                    now: self.now,
                }
            }
            Step::Expire { advance_ms } => {
                self.now = self.now.plus(advance_ms);
                Command::ExpireLeases { now: self.now }
            }
        }
    }

    /// Decide one step and apply what was decided.
    fn step(&mut self, step: &Step) -> (Command, Result<Vec<Event>, Refusal>) {
        let command = self.command(step);
        let outcome = decide(&self.state, command.clone());
        if let Ok(events) = &outcome {
            for event in events {
                apply(&mut self.state, event);
                self.events.push(event.clone());
            }
        }
        (command, outcome)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn a_job_succeeds_at_most_once(steps in steps()) {
        let mut run = Run::new();
        for step in &steps {
            let _ = run.step(step);
        }
        let mut succeeded = BTreeSet::new();
        for event in &run.events {
            if let Event::Succeeded { job, .. } = event {
                prop_assert!(succeeded.insert(*job), "{job:?} succeeded twice");
            }
        }
    }

    #[test]
    fn attempts_never_exceed_max_attempts(steps in steps()) {
        let mut run = Run::new();
        for step in &steps {
            let _ = run.step(step);
            for job in run.state.jobs() {
                prop_assert!(job.attempts <= job.max_attempts, "{job:?}");
            }
        }
    }

    #[test]
    fn a_stale_lease_changes_nothing(steps in steps()) {
        let mut run = Run::new();
        for step in &steps {
            let before = run.state.clone();
            let (command, outcome) = run.step(step);
            let held = |job: &JobId, lease: &LeaseId| {
                matches!(before.job(*job).map(|found| &found.state),
                    Some(JobState::Leased { lease: current, .. }) if current == lease)
            };
            let stale = match &command {
                Command::Complete { job, lease, .. } | Command::Fail { job, lease, .. } => !held(job, lease),
                _ => false,
            };
            if stale {
                prop_assert!(outcome.is_err(), "{command:?} was accepted with a stale lease");
                prop_assert_eq!(&run.state, &before);
            }
        }
    }

    #[test]
    fn replaying_the_events_rebuilds_the_state(steps in steps()) {
        let mut run = Run::new();
        for step in &steps {
            let _ = run.step(step);
        }
        let mut replayed = State::default();
        for event in &run.events {
            apply(&mut replayed, event);
        }
        prop_assert_eq!(replayed, run.state);
    }

    #[test]
    fn the_same_commands_give_the_same_events(steps in steps()) {
        let (mut first, mut second) = (Run::new(), Run::new());
        for step in &steps {
            let _ = first.step(step);
            let _ = second.step(step);
        }
        prop_assert_eq!(first.events, second.events);
    }
}
