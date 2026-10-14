//! The ledger's state. It is rebuilt only by applying events.

use std::collections::BTreeMap;

use ledger_api::{Event, Job, JobId, JobState, LeaseId, Millis};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    jobs: BTreeMap<JobId, Job>,
    by_key: BTreeMap<String, JobId>,
    next_job: u64,
    next_lease: u64,
}

impl State {
    pub fn job(&self, id: JobId) -> Option<&Job> {
        self.jobs.get(&id)
    }

    pub fn job_for_key(&self, key: &str) -> Option<JobId> {
        self.by_key.get(key).copied()
    }

    pub fn jobs(&self) -> impl Iterator<Item = &Job> {
        self.jobs.values()
    }

    pub(crate) fn next_job_id(&self) -> JobId {
        JobId(self.next_job)
    }

    pub(crate) fn next_lease_id(&self) -> LeaseId {
        LeaseId(self.next_lease)
    }

    /// The pending job that has been due longest at `now`. Ties go to the lowest id.
    pub(crate) fn next_due(&self, now: Millis) -> Option<&Job> {
        self.jobs
            .values()
            .filter_map(|job| match job.state {
                JobState::Pending { not_before } if not_before <= now => Some((not_before, job)),
                _ => None,
            })
            .min_by_key(|(not_before, job)| (*not_before, job.id))
            .map(|(_, job)| job)
    }

    pub(crate) fn expired_leases(&self, now: Millis) -> impl Iterator<Item = &Job> {
        self.jobs.values().filter(
            move |job| matches!(job.state, JobState::Leased { expires, .. } if expires <= now),
        )
    }

    pub(crate) fn apply(&mut self, event: &Event) {
        match event {
            Event::Submitted {
                job,
                idempotency_key,
                kind,
                max_attempts,
            } => {
                self.jobs.insert(
                    *job,
                    Job {
                        id: *job,
                        idempotency_key: idempotency_key.clone(),
                        kind: kind.clone(),
                        max_attempts: *max_attempts,
                        attempts: 0,
                        state: JobState::Pending {
                            not_before: Millis(0),
                        },
                    },
                );
                self.by_key.insert(idempotency_key.clone(), *job);
                self.next_job = self.next_job.max(job.0 + 1);
            }
            Event::Leased {
                job,
                lease,
                attempt,
                expires,
            } => {
                if let Some(found) = self.jobs.get_mut(job) {
                    found.attempts = *attempt;
                    found.state = JobState::Leased {
                        lease: *lease,
                        expires: *expires,
                    };
                }
                self.next_lease = self.next_lease.max(lease.0 + 1);
            }
            Event::Succeeded { job, result } => {
                self.set_state(
                    *job,
                    JobState::Succeeded {
                        result: result.clone(),
                    },
                );
            }
            Event::Retrying {
                job, not_before, ..
            } => {
                self.set_state(
                    *job,
                    JobState::Pending {
                        not_before: *not_before,
                    },
                );
            }
            Event::Died { job, reason } => {
                self.set_state(
                    *job,
                    JobState::Dead {
                        reason: reason.clone(),
                    },
                );
            }
        }
    }

    fn set_state(&mut self, job: JobId, state: JobState) {
        if let Some(found) = self.jobs.get_mut(&job) {
            found.state = state;
        }
    }
}
