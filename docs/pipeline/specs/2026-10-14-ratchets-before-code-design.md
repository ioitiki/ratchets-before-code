# Spec: ratchets-before-code

**Status:** approved by the operator in design review, 2026-10-14. Awaiting operator review of this written spec.
**Operator:** Andy (Andrew Malkin). He makes every decision recorded here.
**Coordinator:** Claude Code session (writes briefs, specs and plans; dispatches workers; verifies gates; merges).

This is the first artifact in this repository's own pipeline record. It was produced by a brainstorming session in which the coordinator proposed options and the operator chose. Each decision below records the choice.

---

## 1. Purpose

This is a public portfolio piece. It shows one engineer's method for producing durable code with orchestrated AI coding agents. The method has two halves:

1. **Ratchets.** Architectural invariants are expressed as failing builds and land before the code they protect.
2. **A delivery pipeline.** Work moves from brief to spec to plan to live-proof report. A coordinator verifies worker gates before merge.

**Audience:** hiring managers, CTOs and staff engineers at AI, blockchain, trading and technology companies.

**Success:** a reader who spends 5 minutes on the README, then clicks into the CI history and `docs/`, can verify each of these from the repository itself:
- the ratchets came first
- the ratchets can fail
- an agent was caught by one
- the repository was built the way it describes

## 2. Decisions (from design review)

| # | Question | Decision |
|---|---|---|
| D1 | How much of gato-os may this use? | **Clean-room.** All code and prose here are written fresh. gato-os is named only as where the method was developed, with the public-safe numbers in §8.3. No gato-os code, ADR text, doc text or internal detail is copied. |
| D2 | What does the piece teach? | **The full method:** ratchets *and* the orchestration pipeline, as one story. |
| D3 | What form? | **Essay (README) plus a small working Rust demo** with real pipeline artifacts and CI. |
| D4 | Is the repo built by its own method? | **Yes.** The artifacts in `docs/` are the actual record of building this repo, not samples. |
| D5 | What does the toy kernel do? | **A durable task ledger.** The domain is neutral, so the method is what the reader sees. |
| D6 | Whose voice is the essay in? | **The operator's.** The coordinator drafts in a plain, direct engineering voice, first person, and the operator does an editing pass before publishing. |

## 3. Hard constraints

- **Confidentiality.** Nothing in this repo or its git history may contain:
  - the name of any Chainstarters client, or any project name derived from one. The list of forbidden terms is kept **outside this repository**, in the operator's private notes, so the rule never leaks the names it protects.
  - internal hostnames or URLs
  - any mention of a pending acquisition or exit
  - personal email addresses

  Commits are authored as `Andrew Malkin <26385844+ioitiki@users.noreply.github.com>`. Agent commits add `Co-Authored-By` trailers.
- **Clean-room.** Workers never get access to gato-os or any Chainstarters repository. Their briefs, specs and plans come only from this repo.
- **Honesty.**
  - Reports separate *measured* results (with the exact command and its real output) from *inferred* ones.
  - A staged demonstration is labeled as staged in its title and description.
  - History is never rewritten to look tidier, with one exception: removing confidential content before the repo is first made public, if the §9 scan finds any. The scan should make that unnecessary.
- **Small.** About 2–3k lines in total, readable in an hour.

## 4. Architecture

A Rust workspace (stable toolchain pinned in `rust-toolchain.toml`) with four crates, each named for what it owns:

| Crate | Owns | Normal dependencies |
|---|---|---|
| `ledger-api` | Wire types only: `JobId`, `Job`, `JobState`, `Command`, `Event`, `EffectSemantics`. No logic beyond constructors and validation. | `serde` |
| `ledger-core` | Pure decision functions. No I/O, no ambient time, no randomness. | `ledger-api` only — **ratchet-enforced allowlist** (§5) |
| `ledger-store` | SQLite persistence: an append-only event journal, effect intent/outcome records, and state rebuilt by replay. **The only crate that may contain SQL.** | `ledger-api`, `rusqlite`, `serde_json` |
| `ledgerd` | A thin HTTP server and in-process executor that wires the core to the store. All transports and clocks live here. | `ledger-api`, `ledger-core`, `ledger-store`, `axum`, `tokio`, `serde_json` |

Only `ledger-core`'s dependencies are ratchet-enforced. The other rows show the intended shape and are kept honest by review. Dev-dependencies such as `proptest` and `tempfile` are allowed in any crate and are not covered by the allowlist.

### 4.1 Core semantics (`ledger-core`)

- **Lifecycle:** `Pending → Leased → Succeeded`. A failure goes `Leased → Pending` with backoff while attempts remain, and `Leased → Dead` once `max_attempts` is reached.
- **Commands:**
  - `Submit { idempotency_key, kind, max_attempts }`
  - `Lease { now, lease_for }`
  - `Complete { job, lease }`
  - `Fail { job, lease, reason, now }`
  - `ExpireLeases { now }`
- **Entry point:** `decide(&State, Command) -> Result<Vec<Event>, Refusal>`. `apply(&mut State, &Event)` folds events into state. State is only ever changed by applying events.
- **Idempotent submission:** a second `Submit` with the same `idempotency_key` returns the existing `JobId` and emits no new job.
- **Retry backoff:** exponential with a cap, as a pure function of the attempt count. There is no jitter; if jitter is ever added, its entropy must come in as an input.
- **Leases:** a stale lease (wrong lease id, or expired) cannot complete or fail a job.
- **Time** is always an argument (`now`). The core never reads a clock.

### 4.2 Durability (`ledger-store` and `ledgerd`)

- Every event is appended to the journal before it takes effect in memory. On startup, state is rebuilt by replaying the journal.
- Executing a job is an **effect**. The executor records an **intent** before running it and an **outcome** after. Every job kind declares its `EffectSemantics` (`Idempotent` or `NotIdempotent`) at registration, and recovery decides from that declaration.
- On restart, `ledgerd` replays the journal and expires every lease (on a single node, any lease alive at startup belongs to a dead process). Then:
  - For an intent with no outcome, if the effect is `Idempotent`, it is re-driven.
  - If it is `NotIdempotent`, the job goes to `Dead` with a reason that names the ambiguity. It is never silently re-run.
- **Demo job kind:** `sleep_then_record { ms, token }`, declared `Idempotent`. It sleeps, then records `token` as the job's result.

### 4.3 Tests

- **Core:** property tests (`proptest`) over random command sequences. Invariants:
  - a job never has more than one `Succeeded` event
  - attempts never exceed `max_attempts`
  - a stale lease can never change a job
  - replaying `apply` over the events reproduces the state
- **Store:** integration tests for journal append and replay, and for intent/outcome bookkeeping.
- **Crash recovery** (a separate CI job):
  1. Start `ledgerd` as a child process.
  2. Submit `sleep_then_record { ms: 2000 }`.
  3. Wait until `GET /jobs/{id}` reports the effect as in flight (intent journaled, no outcome yet), then `kill -9` the process.
  4. Restart it and wait for completion.
  5. Assert that the job has **exactly one** `Succeeded` event and that the recorded result appears once.

## 5. The ratchets

Each ratchet lives in `scripts/ratchets/`, is written in bash using `cargo metadata` and `jq` where dependencies are involved, and begins with a header comment naming the specific decay it prevents. `scripts/ratchets/all.sh` runs them all.

| Ratchet | Fails when |
|---|---|
| `core-purity.sh` | (a) `ledger-core`'s normal dependencies, read from `cargo metadata`, include anything outside the allowlist `{ledger-api}`; or (b) non-comment lines in `ledger-core/src` reference `std::net`, `std::fs`, `std::process`, `std::env`, `SystemTime::now`, `Instant::now`, `tokio`, `rusqlite`, `reqwest` or `rand`. |
| `core-line-budget.sh` | Implementation lines in `ledger-core/src` exceed `BUDGET`. Implementation lines are non-blank, non-comment lines outside `#[cfg(test)]` modules and `tests/`. The starting budget is **800**. ADR 0004 lowered it to 250 once the real core was measured. The core is expected to settle at about 400–600 lines, so the budget is a real constraint, not a description of whatever size the core reaches. Raising it means editing `BUDGET` in the script in the same diff. |
| `sql-containment.sh` | Non-comment lines outside `ledger-store` contain SQL statements (case-insensitive `SELECT … FROM`, `INSERT INTO`, `UPDATE … SET`, `DELETE FROM`, `CREATE TABLE`, `ALTER TABLE`, `DROP TABLE`), or `rusqlite` appears in any manifest or source outside `ledger-store`. |
| `no-dev-paths.sh` | Any source contains `cfg(debug_assertions)`, a `cfg(feature = …)` whose feature name matches `dev`, `debug`, `local`, `insecure` or `skip`, or an environment-variable read whose name matches `SKIP`, `INSECURE`, `BYPASS` or `DEV_`. |
| `wire-types-in-api.sh` | `Serialize` or `Deserialize` derives, or hand-written impls of them, appear in any crate other than `ledger-api`. |

**Rules for every ratchet:**

1. **Comment lines are ignored by every source scan,** so a doc comment explaining a rule never trips it.
2. **Dependency checks are allowlists, never denylists.**
3. **Every ratchet must prove it can fail.** `scripts/ratchets/test-ratchets.sh` runs each ratchet against scratch copies of the repo. Every ratchet must exit 0 on a clean tree. Each `fixtures/<ratchet>/fail-*.sh` plants one violation, and the ratchet must exit exactly 1 (exit 2, "could not run", does not count). Each `fixtures/<ratchet>/pass-*.sh` plants something that must not trip it, such as the forbidden thing named in a comment or an allowed lookalike, and the ratchet must exit 0. A ratchet without a `fail-*` fixture, or a fixtures directory without a ratchet, fails the test.
4. **The ratchets land in commit #1, before any crate exists.** Commit #1 contains this spec, plan 01 and its report, ADR 0001, the five ratchets with their self-tests, the CI workflow, and the dev shell. With no crates present, each ratchet passes trivially.

**CI** (`.github/workflows/ci.yml`, on every pull request and every push to `main`):

1. `ratchets`: shellcheck, `all.sh`, then `test-ratchets.sh`. There is no toolchain install step; the job uses the bash, jq, cargo and shellcheck already on the runner.
2. `build` (requires `ratchets`): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
3. `crash-recovery` (requires `build`): the §4.3 crash test.

**Local development:** a `flake.nix` dev shell for Nix users. CI uses the standard Rust toolchain action, so nobody needs Nix.

## 6. The pipeline and the record

**Roles:**
- **Operator** (Andy): decides. ADRs record decisions as "Decided by Andy".
- **Coordinator** (Claude Code): writes briefs, specs and plans; dispatches workers; personally re-runs every gate a worker claims passed; merges.
- **Workers:** Claude Code, plus Codex where suitable. Each works in its own Orca worktree, follows one plan, and opens one PR.

**Record layout:**

```
docs/
  decisions/            ADRs: 0001 ratchets land before code; 0002 time is an input to the core;
                        0003 the journal records intent before outcome; more as they arise
  pipeline/
    briefs/             question, evidence, and "investigate only" or "build"
    specs/              designs (this file is the first)
    plans/              task-by-task plans a worker executes
    reports/            live proofs: exact commands, real output, "Measured" vs "Inferred"
```

**Required evidence:**

1. **An investigation that builds nothing.** At least one brief marked *investigate only* must end with a real measurement and a recommendation not to build. The candidate question is "do leases need a distributed lock?", answered by measuring the single-node design against the failure it would guard against.
2. **A red CI run that catches an agent.**
   - If a worker breaks a ratchet for real during the build, keep that PR and its red run unedited and link both from the essay.
   - If no natural violation has happened by the time the demo is complete, open a demonstration PR titled `[staged demo] …` whose description says it is staged. A worker is told to implement duplicate-submission checking as a SQL query inside `ledger-core`. CI goes red on `sql-containment` and `core-purity`, and a follow-up commit moves the check to the store or the pure core and goes green.
   - The essay says which of the two happened.

**Commit rules:**
- Every agent-authored commit carries a real `Co-Authored-By` trailer naming the model.
- Every merge to `main` is a PR the coordinator verified.

## 7. Build sequence

Each step is one plan executed by one or more workers and ends with a report.

1. **Commit #1:** this spec, plan 01 and its report, ADR 0001, the ratchets, their self-tests, the CI workflow and the dev shell. The coordinator commits it directly; it is the only commit not made through a PR.
2. `ledger-api` and `ledger-core` with property tests. ADR 0002.
3. `ledger-store`: journal, replay, intent and outcome. ADR 0003.
4. `ledgerd` and the crash-recovery CI job.
5. The investigation brief that builds nothing.
6. The red-CI evidence (natural or staged).
7. The essay draft (README), followed by the operator's editing pass.
8. The pre-publish scan (§9), then make the repo public.

## 8. The essay (README)

### 8.1 Shape

About 2,000 words, first person, in the operator's voice.

**Title:** *Ratchets before code: how I keep agent-written code from rotting.*

**Sections:**

1. **The problem.** Agents make the same reasonable exception under deadline that people make, only far more often. Tell the decay story from the operator's experience without naming the product: a large agent platform he led accumulated about 100 special-case handlers, a development path that had forked from production, and an auth bypass. Each was a reasonable exception made under deadline.
2. **Why architecture docs don't stop decay, and a failing build does.**
3. **The ratchets and their rules** (§5): allowlists, comments ignored, a budget that starts below need, raising it as a visible diff, and ratchets that prove they can fail.
4. **The pipeline** (§6): brief → spec → plan → live proof; a coordinator that verifies before merge; investigations that build nothing; measured separated from inferred.
5. **The record.** Links into this repo's real artifacts and the red CI run.
6. **At scale.** gato-os, using only the facts in §8.3.
7. **Honest limits.** Ratchets are blunt. Text scans can be evaded. They encode yesterday's decay, not tomorrow's. A human still makes the calls.
8. **Running it.** Clone, `cargo test`, run the ratchets, run the crash test.

### 8.2 Voice

The coordinator drafts. The operator edits, and nothing is published without his approval of the final text.

### 8.3 Allowed facts about gato-os

Use these and nothing else:
- gato-os is an agent kernel the operator architects.
- It was built in about 9 weeks.
- Its kernel is about 4.8k implementation lines under a 15k budget enforced in CI.
- It has about 2,100 tests.
- A crash-recovery job kills the kernel mid-turn on every pull request and every push to `main`.
- Its ratchets shipped in its first commit.

Do not include its internal architecture, crate names, ADR titles, clients or hosting.

## 9. Publishing gates

The repository is created **private**. It becomes public only when all of these hold:

1. The operator has approved the final README text.
2. CI is green on `main`: ratchets, ratchet self-tests, build and crash recovery.
3. The **pre-publish scan** comes back clean over the working tree **and the full git history**. It checks for:
   - every term on the operator's private forbidden-terms list, case-insensitive (client names and derived project names, kept outside this repository)
   - `chainstarters.com` hostnames other than the public marketing site
   - email addresses other than the noreply author address
   - secrets (gitleaks, or an equivalent pattern scan)

   Any match blocks publishing.
4. Every report's "Measured" section contains real commands and output.

After the repo is public:
- pin it on the GitHub profile
- add the profile README and bio
- add a Projects entry linking to it in the operator's CV

## 10. Out of scope

- authentication
- multiple nodes or distributed operation
- a UI
- real scheduling (cron, priorities)
- publishing the ratchets as a reusable tool (crate or GitHub Action)
- any blog platform other than this README
