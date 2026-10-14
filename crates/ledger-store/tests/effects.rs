//! An effect records its intent before it runs and its outcome after.
//! Whatever has an intent and no outcome is what a restart must decide about.

use ledger_api::{JobId, Millis};
use ledger_store::{Intent, Store, StoreError};

fn intent(job: u64, attempt: u32, at: u64) -> Intent {
    Intent {
        job: JobId(job),
        attempt,
        at: Millis(at),
    }
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
    assert!(
        matches!(
            err,
            StoreError::NoSuchIntent {
                job: JobId(7),
                attempt: 1
            }
        ),
        "{err}"
    );
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
    assert_eq!(
        reopened.unfinished().unwrap(),
        vec![intent(0, 1, 10), intent(1, 1, 30)]
    );
}
