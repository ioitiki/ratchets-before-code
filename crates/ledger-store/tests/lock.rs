//! One journal, one holder (ADR 0005).

use ledger_store::{Store, StoreError};

#[test]
fn a_journal_already_open_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.db");
    let _first = Store::open(&path).unwrap();
    match Store::open(&path) {
        Err(StoreError::JournalInUse { lock }) => {
            let real = dir.path().canonicalize().unwrap();
            assert_eq!(lock, real.join("ledger.db.lock"));
        }
        Err(other) => panic!("expected JournalInUse, got: {other}"),
        Ok(_) => panic!("a second store opened a journal that is already in use"),
    }
}

#[test]
fn a_symlink_to_a_journal_already_open_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.db");
    let alias = dir.path().join("alias.db");
    let _first = Store::open(&path).unwrap();
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    match Store::open(&alias) {
        Err(StoreError::JournalInUse { .. }) => {}
        Err(other) => panic!("expected JournalInUse, got: {other}"),
        Ok(_) => panic!("a symlink opened a journal that is already in use"),
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
