# ADR 0002: Time is an input to the core

**Status:** accepted
**Date:** 2026-10-14
**Decided by:** Andy

## Context

Retries, backoff and lease expiry are all decisions about time. If
`ledger-core` reads a clock to make them, every test of those decisions has
to control a real clock, either by sleeping or by mocking a global. The
properties that matter most (a job succeeds at most once, a stale lease
changes nothing, replaying events rebuilds the state) would only ever be
tested at whatever moments the test happened to run.

## Decision

The core never reads a clock. Every command that depends on time carries
`now: Millis`. Whoever calls the core, here `ledgerd`, reads the clock once
and passes it in. The same holds for randomness: backoff has no jitter, and if
jitter is ever needed, its entropy arrives as an argument.

`core-purity.sh` enforces this. `SystemTime::now`, `Instant::now` and `rand`
in the core's source fail the build.

## Consequences

- Every time-dependent rule can be tested at any instant, including instants
  no real test run would ever hit. The property tests drive the clock forward
  in random jumps.
- Replaying a journal reproduces the same decisions, because the times are in
  the journal, not in the machine that replays it.
- Clock skew and monotonicity become the caller's problem, in one place,
  instead of the core's problem in every function.
- Commands are slightly larger. That is the whole cost.
