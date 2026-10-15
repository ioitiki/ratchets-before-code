//! Kill the server with SIGKILL in the middle of a job's effect, restart it,
//! and check the job completes exactly once. Spec §4.3.
//!
//! Ignored by `cargo test`; CI's `crash-recovery` job runs it with
//! `--ignored`, because it spawns, kills and restarts a real process.

mod common;

use std::time::{Duration, Instant};

use common::{http, json, start, wait_for};
use ledger_api::{Event, JobId};
use ledger_store::Store;

#[test]
#[ignore = "spawns and kills the real server; run by CI's crash-recovery job"]
fn a_job_killed_mid_effect_completes_exactly_once_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.db");

    // 1. Start the server and submit a job whose effect takes 2 seconds.
    let mut server = start(&db);
    let body = r#"{"idempotency_key":"crash","kind":{"SleepThenRecord":{"ms":2000,"token":"crash-test"}},"max_attempts":3}"#;
    let (code, reply) = http(&server.addr, "POST", "/jobs", Some(body));
    assert_eq!(code, 201, "{reply}");
    let job = json(&reply)["job"].as_u64().unwrap();

    // 2. Wait until the intent is journaled and the effect is running.
    wait_for(&server.addr, job, Duration::from_secs(10), |s| {
        s["effect"]["InFlight"].is_object()
    });

    // 3. SIGKILL: no shutdown handler runs, and nothing gets flushed politely.
    let killed_at = Instant::now();
    server.child.kill().unwrap();
    let exit = server.child.wait().unwrap();
    println!("killed mid-effect: {exit}");
    drop(server);

    // 4. Restart against the same journal and wait for the job to succeed.
    let server = start(&db);
    let status = wait_for(&server.addr, job, Duration::from_secs(30), |s| {
        s["job"]["state"]["Succeeded"].is_object()
    });
    println!(
        "succeeded {:?} after the kill: {status}",
        killed_at.elapsed()
    );
    assert_eq!(
        status["job"]["attempts"], 2,
        "the killed attempt was re-driven as attempt 2"
    );
    drop(server);

    // 5. The journal holds exactly one success, with the job's own result.
    let successes: Vec<Event> = Store::open(&db)
        .unwrap()
        .events()
        .unwrap()
        .into_iter()
        .filter(|event| matches!(event, Event::Succeeded { job: j, .. } if *j == JobId(job)))
        .collect();
    assert_eq!(
        successes,
        vec![Event::Succeeded {
            job: JobId(job),
            result: "crash-test".into()
        }]
    );
}
