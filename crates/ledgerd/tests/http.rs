//! The HTTP API, against the real binary.

mod common;

use std::time::Duration;

use common::{http, json, start, wait_for};

#[test]
fn a_duplicate_submission_returns_the_same_job() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(&dir.path().join("ledger.db"));
    let body = r#"{"idempotency_key":"k","kind":{"SleepThenRecord":{"ms":1,"token":"t"}},"max_attempts":2}"#;
    let (first, first_body) = http(&server.addr, "POST", "/jobs", Some(body));
    let (second, second_body) = http(&server.addr, "POST", "/jobs", Some(body));
    assert_eq!((first, second), (201, 200), "{first_body} / {second_body}");
    assert_eq!(json(&first_body)["job"], json(&second_body)["job"]);
    assert_eq!(json(&second_body)["created"], false);
}

#[test]
fn an_unknown_job_is_not_found_and_zero_attempts_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(&dir.path().join("ledger.db"));
    assert_eq!(http(&server.addr, "GET", "/jobs/999", None).0, 404);
    let zero = r#"{"idempotency_key":"z","kind":{"Notify":{"message":"m"}},"max_attempts":0}"#;
    assert_eq!(http(&server.addr, "POST", "/jobs", Some(zero)).0, 422);
}

#[test]
fn a_submitted_job_runs_to_success() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(&dir.path().join("ledger.db"));
    let body = r#"{"idempotency_key":"n","kind":{"Notify":{"message":"hello"}},"max_attempts":1}"#;
    let (code, reply) = http(&server.addr, "POST", "/jobs", Some(body));
    assert_eq!(code, 201, "{reply}");
    let job = json(&reply)["job"].as_u64().unwrap();
    let status = wait_for(&server.addr, job, Duration::from_secs(10), |s| {
        s["job"]["state"]["Succeeded"].is_object()
    });
    assert_eq!(
        status["job"]["state"]["Succeeded"]["result"],
        "notified: hello"
    );
    assert_eq!(status["effect"]["Finished"]["attempt"], 1);
}
