# Brief 05: do leases need a distributed lock?

**Type:** investigate only. This brief authorizes measurement and a report, and nothing else. It does not authorize changes to `crates/`, `scripts/`, `.github/`, `Cargo.toml`, `Cargo.lock`, `flake.nix` or `docs/decisions/`.
**Spec:** [`../specs/2026-10-14-ratchets-before-code-design.md`](../specs/2026-10-14-ratchets-before-code-design.md): §6 (required evidence 1), §7 step 5, §10 (multiple nodes are out of scope).
**Executed by:** a worker in its own Orca worktree. It ends with a PR that contains only the report and its evidence, which the coordinator verifies before merging.

## The question

A lease says "this executor owns this job until time T." Job queues often back leases with a distributed lock (Redis, etcd, ZooKeeper, a Postgres advisory lock) so that two executors can never run the same job at once. Does `ledgerd` need one?

## What a distributed lock would guard against

Two holders of a live lease on the same job at the same moment, so the job's effect runs twice. In this design that could happen four ways. Investigate each one.

1. **Inside one process.** Concurrent HTTP clients and the executor all reach the same `Ledger`.
   - *Hypothesis:* `SharedLedger` (`Arc<tokio::sync::Mutex<Ledger>>`) serializes every `decide`, so two leases on one job can never both be granted.
   - *Measure:* run at least 1,000 jobs through one `ledgerd`. Submit them from many concurrent clients, and include duplicate idempotency keys. When the queue drains, count from the journal (the SQLite file) the `Succeeded` events per job and the effect intents per `(job, attempt)`.
   - *Expected:* exactly 1 of each per job. Report the real distribution, whatever it is.
2. **Across a restart.** Plan 04's crash test already measures this. Run it once, quote the output and cite report 04. Don't build anything new for it.
3. **Two processes on one journal.** The design assumes one `ledgerd` per journal: at startup it expires every lease, on the grounds that on a single node "any lease alive at startup belongs to a dead process" (spec §4.2). Measure what happens when that assumption is broken.
   - *Steps:*
     1. Start `ledgerd` A.
     2. Submit a long `sleep_then_record` job and wait until its effect is in flight.
     3. Start `ledgerd` B on the same `LEDGER_DB`, with a different `LEDGER_ADDR`.
     4. Wait for both processes to go quiet.
   - *Record:*
     - Does B start?
     - Does B expire A's live lease and re-drive the effect?
     - Do the two processes' appends collide in the journal?
     - What counts of intents, outcomes and `Succeeded` events does the job end with?
   - Run it at least 3 times and report every run.
4. **Clock disagreement.** Leases expire against `now`, and a single node has one clock. Check whether anything other than `ledgerd::now` can supply `now` to a lease decision. A reasoned answer is enough here, marked *inferred*, citing ADR 0002 and the code.

## Rules

- **The answer is not predetermined.** If the measurements show a lock is needed within the spec's scope, the report says so. Don't shape the result to fill the "builds nothing" slot in §6. That slot needs a real measurement, not a convenient one.
- **Measure the code on `main` as it is.** Change nothing under `crates/`, `scripts/` or `.github/`.
- **The harness is one bash script:** `docs/pipeline/reports/2026-10-14-05-evidence/measure.sh`.
  - It builds and starts `ledgerd` with cargo and drives it over HTTP.
  - It reads the journal with `sqlite3`, using the schema in `crates/ledger-store/src/schema.sql`.
  - It uses only the dev shell's tools, plus `curl` from the host if the dev shell lacks it. Say which in the report.
  - It must pass `shellcheck`, and CI must stay green. If a ratchet flags the script, stop and report. Never weaken a ratchet.
- **Raw output goes beside the script** as `run-*.txt`, and the report quotes it verbatim. Use the same trimming conventions as report 04: shorten paths to `…`, drop Nix's dirty-tree warnings, and mark omissions `[…]`.
- **Report:** `docs/pipeline/reports/2026-10-14-05-do-leases-need-a-distributed-lock.md`, shaped like reports 01–04, in this order:
  1. **Answer:** one paragraph: "Do leases need a distributed lock? {Yes/No}, because {measured reason}."
  2. **Measured:** each exact command and its real output.
  3. **Inferred:** reasoning that no command proved, labeled as such.
  4. **Recommendation:** what to build, if anything, and what not to build.
  5. **Findings for the operator:** anything you found outside the question, such as a guard the design lacks. Give each one a proposed next step. Don't build it; the operator decides.
- **Clean-room, confidentiality and commit rules:** the same as plan 04's "Rules for the worker". Read them there. Commit with the repo's configured author, and add a `Co-Authored-By` trailer naming your model.
- **Finish:** open a PR against `main` titled `Brief 05: do leases need a distributed lock? (investigate only)`. Don't merge it.
