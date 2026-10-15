//! Wire types for the ledger: everything that crosses a process boundary.
//!
//! This crate holds data, not decisions. The only behavior here is what a
//! type must say about itself, such as which effect semantics a job kind
//! declares (ADR 0003).

use serde::{Deserialize, Serialize};

/// A point in time, in milliseconds. The core never reads a clock: whoever
/// calls it says what time it is (ADR 0002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Millis(pub u64);

impl Millis {
    /// `self` plus `ms`, saturating instead of overflowing.
    pub fn plus(self, ms: u64) -> Millis {
        Millis(self.0.saturating_add(ms))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LeaseId(pub u64);

/// What a job does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    /// Sleep for `ms`, then record `token` as the job's result.
    SleepThenRecord { ms: u64, token: String },
    /// Send `message`, once. Sending it twice is visible to whoever receives
    /// it, so this kind is not idempotent.
    Notify { message: String },
}

/// What recovery may assume about re-running an effect whose outcome was
/// never recorded (ADR 0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectSemantics {
    /// Running it twice has the same result as running it once: re-drive it.
    Idempotent,
    /// Running it twice may do harm: never re-run it silently.
    NotIdempotent,
}

impl JobKind {
    /// Every kind declares its semantics here, where it is defined, because
    /// they cannot be worked out after a crash.
    pub fn effect_semantics(&self) -> EffectSemantics {
        match self {
            JobKind::SleepThenRecord { .. } => EffectSemantics::Idempotent,
            JobKind::Notify { .. } => EffectSemantics::NotIdempotent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    /// Waiting to be leased, no earlier than `not_before`.
    Pending {
        not_before: Millis,
    },
    /// Held by a worker until `expires`.
    Leased {
        lease: LeaseId,
        expires: Millis,
    },
    Succeeded {
        result: String,
    },
    Dead {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub idempotency_key: String,
    pub kind: JobKind,
    pub max_attempts: u32,
    /// Attempts started so far. A lease starts an attempt.
    pub attempts: u32,
    pub state: JobState,
}

/// A request to change the ledger. The core turns each one into events or a
/// refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Submit {
        idempotency_key: String,
        kind: JobKind,
        max_attempts: u32,
    },
    Lease {
        now: Millis,
        lease_for_ms: u64,
    },
    Complete {
        job: JobId,
        lease: LeaseId,
        result: String,
    },
    Fail {
        job: JobId,
        lease: LeaseId,
        reason: String,
        now: Millis,
    },
    ExpireLeases {
        now: Millis,
    },
}

/// A fact the ledger has recorded. State only ever changes by applying events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Submitted {
        job: JobId,
        idempotency_key: String,
        kind: JobKind,
        max_attempts: u32,
    },
    Leased {
        job: JobId,
        lease: LeaseId,
        attempt: u32,
        expires: Millis,
    },
    Succeeded {
        job: JobId,
        result: String,
    },
    Retrying {
        job: JobId,
        not_before: Millis,
        reason: String,
    },
    Died {
        job: JobId,
        reason: String,
    },
}

/// Why the core declined a command. A refusal records nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    MaxAttemptsMustBePositive,
    UnknownJob {
        job: JobId,
    },
    /// The lease is not the job's current lease, or the job is not leased.
    StaleLease {
        job: JobId,
        lease: LeaseId,
    },
}

/// The body of `POST /jobs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitJob {
    pub idempotency_key: String,
    pub kind: JobKind,
    pub max_attempts: u32,
}

/// The reply to `POST /jobs`. `created` is false when the key was already known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitReply {
    pub job: JobId,
    pub created: bool,
}

/// What the journal knows about the effect of a job's current attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectStatus {
    NotStarted,
    InFlight { attempt: u32 },
    Finished { attempt: u32 },
}

/// The reply to `GET /jobs/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobStatus {
    pub job: Job,
    pub effect: EffectStatus,
}
