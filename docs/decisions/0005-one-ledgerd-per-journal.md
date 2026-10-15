# ADR 0005: One `ledgerd` per journal, enforced at open

**Status:** accepted.
**Date:** 2026-10-14
**Decided by:** Andy

## Context

Spec §4.2 assumes one `ledgerd` per journal. On startup, `ledgerd` expires every lease, because on a single node "any lease alive at startup belongs to a dead process."

Brief 05 asked whether leases need a distributed lock, and measured that they don't. Inside one process, 1,000 jobs from 32 concurrent clients each ended with exactly one lease, one intent, one outcome and one `Succeeded` event.

The same report measured what happens when the assumption is broken (report 05, finding 1). A second `ledgerd` started on a journal already in use behaves like this:
- It treats the first process's live lease as dead.
- It runs the effect again.
- It leaves two `Succeeded` events for one job.

That happened in 3 runs out of 3. A second process that fails to bind its port has still written to the journal by then, because replay and recovery come first (finding 2).

So the assumption was stated but not enforced.

## Decision

`Store::open` takes an exclusive, non-blocking advisory lock before it touches the journal. If another holder has it, the open fails with `StoreError::JournalInUse`, and `ledgerd` exits non-zero having written nothing.

- **The lock lives on a sidecar file, `<journal>.lock`.** It is held through `std::fs::File::try_lock` for as long as the `Store` lives.
- **Taken first:** the lock comes before the SQLite connection is opened, so before replay, recovery or binding a port.
- **Released by the kernel:** the operating system drops the lock when the process dies, including on `kill -9`. Crash recovery (ADR 0003) is unchanged: the restarted process takes the lock and recovers as before.

## Consequences

- **Finding 1 is closed.** A second process on the same journal now refuses to start. Report 05's case 3 is the regression check: the job should end with one intent, one outcome and one `Succeeded` event.
- **Finding 2 is closed too.** The refusal happens before replay, so a process that cannot start never writes.
- **The guarantee is host-local.** Advisory locks are not reliable on network filesystems. Multiple nodes and shared storage stay out of scope (spec §10).
- **The lock file is left behind on exit.** It is harmless: the lock lives on the open file, not on the file's existence.
- **Report 05's other findings are recorded, not fixed.** They are findings 3 to 6. With one writer per journal, each one becomes a known limit rather than a live bug.

## Alternatives rejected

- **A distributed lock (Redis, etcd, ZooKeeper, a Postgres advisory lock).** Brief 05 measured no in-scope case that needs one. It also would not have stopped finding 1 unless recovery consulted it, which brings back the same question this lock answers directly: does another live process hold the journal?
- **Locking the journal file itself.** SQLite holds POSIX locks on the journal, and closing any other descriptor for the same file drops them. A separate file keeps the two kinds of lock apart.
- **SQLite's `PRAGMA locking_mode = EXCLUSIVE`.** It ties the guarantee to SQLite's lock timing and busy handling. The sidecar lock fails fast, before the journal is read, with an error that names the cause.
