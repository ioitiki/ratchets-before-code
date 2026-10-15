//! A second `ledgerd` on a journal that is already in use exits without
//! writing, and the first one's job finishes on its first attempt
//! (ADR 0005; report 05, findings 1 and 2).

mod common;

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{http, json, start, wait_for};

#[test]
fn a_second_ledgerd_on_the_same_journal_exits_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.db");

    // A is running a job whose effect takes 2 seconds.
    let a = start(&db);
    let body = r#"{"idempotency_key":"held","kind":{"SleepThenRecord":{"ms":2000,"token":"held"}},"max_attempts":3}"#;
    let (code, reply) = http(&a.addr, "POST", "/jobs", Some(body));
    assert_eq!(code, 201, "{reply}");
    let job = json(&reply)["job"].as_u64().unwrap();
    wait_for(&a.addr, job, Duration::from_secs(10), |s| {
        s["effect"]["InFlight"].is_object()
    });

    // B is pointed at the same journal while A's effect is running.
    let mut b = Command::new(env!("CARGO_BIN_EXE_ledgerd"))
        .env("LEDGER_DB", &db)
        .env("LEDGER_ADDR", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ledgerd starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    let exit = loop {
        if let Some(exit) = b.try_wait().unwrap() {
            break exit;
        }
        if Instant::now() > deadline {
            let _ = b.kill();
            let _ = b.wait();
            panic!("B is still running: it started on a journal that was already in use");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    b.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
    b.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    println!("B exited with {exit}; stderr: {stderr}");
    assert!(!exit.success(), "B must exit non-zero");
    assert!(stderr.contains("JournalInUse"), "B names the cause: {stderr}");
    assert!(stdout.is_empty(), "B never got as far as listening: {stdout}");

    // A's job finishes on its first attempt: B never expired A's lease.
    let status = wait_for(&a.addr, job, Duration::from_secs(10), |s| {
        s["job"]["state"]["Succeeded"].is_object()
    });
    assert_eq!(status["job"]["attempts"], 1, "{status}");
    assert_eq!(status["effect"]["Finished"]["attempt"], 1, "{status}");
}
