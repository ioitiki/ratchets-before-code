//! The journal gives back exactly what was appended, in order, across restarts.

use ledger_api::{Event, JobId, JobKind, LeaseId, Millis};
use ledger_store::Store;
use proptest::prelude::*;

fn sample() -> Vec<Event> {
    vec![
        Event::Submitted {
            job: JobId(0),
            idempotency_key: "k".into(),
            kind: JobKind::SleepThenRecord {
                ms: 5,
                token: "t".into(),
            },
            max_attempts: 3,
        },
        Event::Leased {
            job: JobId(0),
            lease: LeaseId(0),
            attempt: 1,
            expires: Millis(1_000),
        },
        Event::Succeeded {
            job: JobId(0),
            result: "t".into(),
        },
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
        (0u64..50, "[a-z]{0,8}").prop_map(|(job, result)| Event::Succeeded {
            job: JobId(job),
            result
        }),
        (0u64..50, 0u64..1_000_000, "[a-z ]{0,12}").prop_map(|(job, at, reason)| Event::Retrying {
            job: JobId(job),
            not_before: Millis(at),
            reason,
        }),
        (0u64..50, "[a-z]{0,8}").prop_map(|(job, reason)| Event::Died {
            job: JobId(job),
            reason
        }),
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
