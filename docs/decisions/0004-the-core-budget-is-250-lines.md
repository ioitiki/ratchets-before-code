# ADR 0004: The core's line budget is 250

**Status:** accepted. Supersedes the starting number in ADR 0001.
**Date:** 2026-10-14
**Decided by:** Andy

## Context

ADR 0001 set `core-line-budget.sh` to 800 lines before any code existed,
against an estimate that the core would settle at 400–600. Plan 02 then built
the whole of spec §4.1 (decide, apply, idempotent submission, backoff, leases
and expiry), and report 02 measured it at **201** implementation lines.

At 800, the ratchet would fire only after the core roughly quadrupled. That
makes it a description of the code, not a constraint on it, which is the
failure ADR 0001 warns about. The estimate was wrong, and the measurement is
what corrects it.

## Decision

`BUDGET` is **250**. That is 24% above the measured core. It leaves room for
the one core rule the spec still expects (on restart: re-drive an
intent-without-outcome if `Idempotent`, kill it if `NotIdempotent`, §4.2).
Anything beyond that trips the ratchet and forces the trade into a reviewed
diff.

## Consequences

- **The count depends on formatting.** The number is measured under rustfmt's
  defaults, because the repository has no `rustfmt.toml`. Under
  `use_small_heuristics = "Max"` the same code measures 148 (report 02).
  Adopting any rustfmt configuration therefore means recounting and resetting
  `BUDGET` in the same diff.
- **Lowering it was cheap now and only gets more expensive.** With 201 lines
  there is nothing to cut. Lowering it after the core had grown to 700 would
  have meant a refactor and an argument.
- **Changing the budget stays a visible decision.** Raising it later works the
  same way as lowering it now: edit the number in the same diff as the code
  that needs it, with a recorded reason.
