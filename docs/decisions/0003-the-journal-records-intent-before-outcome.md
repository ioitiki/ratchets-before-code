# ADR 0003: The journal records intent before outcome

**Status:** accepted
**Date:** 2026-10-14
**Decided by:** Andy

## Context

Running a job does something in the world: an effect. A process can be
killed at any instant: before the effect starts, while it runs, after it
finishes but before anyone writes that down, or after. On restart, the ledger
has to know which of those happened, and the only evidence it will have is
what it wrote down before the kill.

## Decision

Every effect is recorded twice:
- an **intent**, before it runs: `ledger-store` `record_intent`
- an **outcome**, after it finishes: `record_outcome`

Every job kind declares its **effect semantics** (`Idempotent` or
`NotIdempotent`) where the kind is defined (`JobKind::effect_semantics` in
`ledger-api`).

On restart, an intent with no outcome is an effect whose fate is unknown:
- If the kind is **idempotent**, the effect is re-driven. Running it twice
  has the same result as running it once.
- If it is **not idempotent**, the job is killed, with a reason that names
  the ambiguity. It is never silently re-run.

## Consequences

- **Recovery is a lookup, not a guess.** Without the declaration, a restart
  would have to infer what is safe from the effect's code, after the fact.
  That is the one moment nobody can afford to be wrong.
- **An effect costs two writes.** The demo is not chasing throughput, and
  the alternative to paying them is not knowing.
- **Non-idempotent work dies instead of repeating.** For effects such as
  charging a card, killing the job is correct: the job surfaces in the
  ledger with a reason, and a person decides.
- **The store keeps rows; it makes no decisions.** It reports which intents
  are unfinished. The recovery rule itself lives where decisions live, which
  `ledgerd` wires up in plan 04.
