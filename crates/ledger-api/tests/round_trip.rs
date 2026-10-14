//! Every wire type survives a trip through JSON unchanged, because the store
//! journals events as JSON and replay depends on reading back exactly what
//! was written.

use ledger_api::{Command, EffectSemantics, Event, JobId, JobKind, LeaseId, Millis, Refusal};

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    serde_json::from_str(&serde_json::to_string(value).expect("serializes")).expect("deserializes")
}

#[test]
fn events_round_trip() {
    let events = vec![
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
            lease: LeaseId(7),
            attempt: 1,
            expires: Millis(1_000),
        },
        Event::Succeeded {
            job: JobId(0),
            result: "t".into(),
        },
        Event::Retrying {
            job: JobId(0),
            not_before: Millis(2_000),
            reason: "boom".into(),
        },
        Event::Died {
            job: JobId(0),
            reason: "boom (after 3 of 3 attempts)".into(),
        },
    ];
    for event in &events {
        assert_eq!(&round_trip(event), event);
    }
}

#[test]
fn commands_and_refusals_round_trip() {
    let command = Command::Fail {
        job: JobId(1),
        lease: LeaseId(2),
        reason: "x".into(),
        now: Millis(3),
    };
    assert_eq!(round_trip(&command), command);
    let refusal = Refusal::StaleLease {
        job: JobId(1),
        lease: LeaseId(2),
    };
    assert_eq!(round_trip(&refusal), refusal);
}

#[test]
fn the_demo_job_declares_itself_idempotent() {
    let kind = JobKind::SleepThenRecord {
        ms: 1,
        token: "t".into(),
    };
    assert_eq!(kind.effect_semantics(), EffectSemantics::Idempotent);
}
