# Ratchets before code: how I keep agent-written code from rotting

I build most of my software with AI coding agents now: several at a time, each in its own worktree, each working from a written plan. They are fast and they are obliging. That combination has a failure mode I have watched happen to a real system, and this repository is how I stop it.

It is two things at once. It is a small, working job ledger in Rust, with a pure decision core, a SQLite journal, and an HTTP server that survives `kill -9` without running a job's result twice. It is also a complete record of how that ledger was built: every brief, spec, plan, decision and live-proof report, written by me or by the agents, and kept in [`docs/`](docs/). The ledger is the specimen. The record is the point.

## The problem

Agents make the same reasonable exception under deadline that people make. They just make it far more often.

I learned this on a large agent platform I led. Nobody decided to let it decay. It got there one defensible change at a time. A new kind of conversation turn needed slightly different handling, so it got its own handler. Then another. Eventually there were about a hundred special-case turn handlers, each one reasonable on the day it was written. The development code path forked from the production one, because something needed to work locally before Friday. An authorization check got bypassed on one path for the same kind of reason.

Each of those was a reasonable exception made under deadline. The damage was in the sum, and nobody ever approved the sum.

When agents write most of the code, the rate of reasonable exceptions goes way up. An agent asked to make a test pass by the end of a task will find the shortest path. The shortest path through an architecture usually runs through one of its boundaries.

## Why documents don't stop decay, and a failing build does

Every system I've worked on had an architecture document. None of them ever failed a build.

A document describes intent. It has no way to say no. Code review can say no, but it is unreliable in exactly the way that matters. The reviewer who should catch the fifth exception is the one who waved through the first four, and each one looked fine in its own diff.

A test that fails the build is different. It says no every time, to everyone, including me. It doesn't get tired, and it doesn't have to remember why the rule exists. Its header comment says why. So the rules that matter in this repo are not written down as guidance. They are bash scripts in [`scripts/ratchets/`](scripts/ratchets/), and CI runs them before it compiles anything.

I call them ratchets because they only move one way without someone noticing. A ratchet can be loosened, but only in a diff that a person reads and approves.

## The ratchets and their rules

There are five:

| Ratchet | What it stops |
|---|---|
| `core-purity` | The decision core picking up I/O one exception at a time. Its dependencies are an allowlist with one entry, and its source may not touch the network, the filesystem, the clock, randomness, or a database. |
| `core-line-budget` | The core quietly absorbing everything. It has a hard line budget. |
| `sql-containment` | SQL leaking out of the one crate that owns the database. |
| `no-dev-paths` | A development-only code path that production never runs. |
| `wire-types-in-api` | Serialization formats spreading across crates instead of living in one API crate. |

The rules for writing them matter as much as the ratchets themselves:

**Comments are ignored.** A doc comment that explains why the core never touches the clock should not trip the ratchet that forbids touching the clock.

**Dependency checks are allowlists, never denylists.** A denylist only stops the crate someone already thought of. `core-purity` reads `cargo metadata` and fails on anything that isn't `ledger-api`.

**A budget starts below need, and raising it is a visible diff.** The line budget started at 800 before any code existed. Then the first real core came in at 201 lines. At 800, the ratchet would only have fired after the core roughly quadrupled. That makes it a description of the code, not a constraint on it. So I lowered it to 250 and recorded why in [ADR 0004](docs/decisions/0004-the-core-budget-is-250-lines.md). The core is 234 lines today. Any agent that wants more room has to edit the budget in the same diff, where I'll see it.

**Every ratchet has to prove it can fail.** A check that never fails might be broken. [`test-ratchets.sh`](scripts/ratchets/test-ratchets.sh) runs each ratchet against scratch copies of the repo with a violation planted in it. The ratchet has to exit 1, not 2 ("couldn't run"). It also has to stay quiet on lookalikes that are allowed. A ratchet with no failing fixture fails the self-test.

**They land first.** The five ratchets, their self-tests and the CI workflow were in [commit #1](https://github.com/ioitiki/ratchets-before-code/commit/db75b17), before any crate existed. With nothing to protect, each one passed trivially. That is the point. A rule added after the code it governs has to be negotiated against everything that already breaks it. A rule that was there first just holds. [ADR 0001](docs/decisions/0001-ratchets-land-before-code.md) is that decision.

## The pipeline

The ratchets keep the architecture from rotting. The pipeline keeps the work honest.

Every piece of work moves through four documents: a **brief** (the question), a **spec** (the design), a **plan** (task-by-task steps, with the code and the commands), and a **live-proof report** (what actually happened, with real commands and their real output). There are three roles. I'm the operator, and I make the decisions; the ADRs say "Decided by Andy". A coordinator agent writes briefs, specs and plans, dispatches workers, and merges. Workers each take one plan in their own git worktree and open one pull request.

The rule that makes this work: **the coordinator re-runs every gate a worker claims passed before anything merges.** The reports separate what was *measured* from what was *inferred*. A claim without its command and output doesn't count.

Agents are good at this when the plan is specific. They're also good at catching the coordinator. In [plan 02](docs/pipeline/reports/2026-10-14-02-ledger-core.md), the worker noticed that the coordinator's gate script piped `cargo clippy` into `tail` and then printed `$?`. That was `tail`'s exit status, not clippy's. A failing build would have reported `exit 0`. Every later plan runs its gates under `bash -o pipefail`.

## Investigations that build nothing

Most process produces code. This one is also allowed to produce a "no".

[Brief 05](docs/pipeline/briefs/2026-10-14-05-do-leases-need-a-distributed-lock.md) asked a question job queues often answer by reflex: do leases need a distributed lock? It was marked *investigate only*, and the answer was not fixed in advance. The worker measured it: 2,000 submissions of 1,000 keys from 32 concurrent clients, against one process. Every job ended with exactly one lease, one recorded intent, one outcome and one success. [The answer was no.](docs/pipeline/reports/2026-10-14-05-do-leases-need-a-distributed-lock.md) No Redis, no etcd, no lock service.

But the same measurement found something real. The design assumed one server process per journal, and nothing enforced it. Start a second process on the same file and it treats the first one's live lease as dead and runs the job again. That happened in 3 runs out of 3. The fix wasn't a distributed lock. It was a local file lock taken before the journal is read ([ADR 0005](docs/decisions/0005-one-ledgerd-per-journal.md)). The worker that built it then went looking for ways around its own lock, found that a symlink to the journal got past it, and recorded that in its report before fixing it. I didn't ask it to try. The report keeps both halves.

That is what I want from a process: "don't build the big thing", "build the small thing", and "here's the hole in what I just built", all on the record.

## The record

Everything above links to real artifacts:

- **The design:** [the spec](docs/pipeline/specs/2026-10-14-ratchets-before-code-design.md)
- **The decisions:** [`docs/decisions/`](docs/decisions/)
- **The plans and live-proof reports:** [`docs/pipeline/`](docs/pipeline/), including the [crash test's report](docs/pipeline/reports/2026-10-14-04-ledgerd.md). There, the server is killed with `kill -9` in the middle of a job's effect, restarted, and the job ends with exactly one success in the journal. That test runs in CI on every pull request.
- **A red CI run that catches an agent.** In honesty, this one was staged. No worker broke a ratchet on its own while this repo was built, so [plan 06](docs/pipeline/plans/2026-10-14-06-staged-demo-red-ci.md) ordered one to. It told the worker to add a SQL lookup for duplicate submissions to the pure core, which is the exact decay `core-purity`'s header describes. [CI went red](https://github.com/ioitiki/ratchets-before-code/pull/7) on `core-purity` and `sql-containment`. The fix removed the lookup, which was never needed, and added the test that proves the core's own check holds across a restart. The PR title says `[staged demo]`, and the red commit is still in the history.

## At scale

This repo is small on purpose: about 2,000 lines of Rust you can read in an hour. The method came from somewhere larger.

gato-os is an agent kernel I architect. It was built in about 9 weeks. Its ratchets shipped in its first commit. The kernel is about 4,800 implementation lines under a 15,000-line budget that CI enforces, and the project has about 2,100 tests. On every pull request and every push to `main`, a crash-recovery job kills the kernel in the middle of a turn and checks that it recovers.

That code is private. This repository is the clean-room version of how it was built: same method, none of its code.

## Honest limits

**Ratchets are blunt.** They catch the decay they were written for and nothing else. `sql-containment` looks for SQL statements in source text. An agent determined to get around it could build a query from string fragments, or hide it behind a helper crate the allowlist never anticipated. These checks catch the reasonable exception made in a hurry. They don't stop someone working against them.

**They encode yesterday's decay.** Every ratchet here exists because I watched that specific thing go wrong. The next kind of rot won't have a ratchet until someone notices it, and the ratchets can't do the noticing.

**A person still makes the calls.** Lowering the budget, accepting the lock, deciding that a finding is a known limit rather than a bug: those were judgment calls, and they are recorded as mine. The process makes the decisions visible. It doesn't make them.

**The ledger is a demo.** It runs on one host. The lock is host-local and not reliable on a network filesystem. Several smaller findings from the investigation are recorded as known limits, not fixed. All of that is written down in the reports, which is the other thing this repo is about.

## Running it

You need a Rust toolchain; `rust-toolchain.toml` pins the version. `jq` is needed for the ratchets. Nix users can run `nix develop` for everything.

```bash
git clone https://github.com/ioitiki/ratchets-before-code
cd ratchets-before-code

./scripts/ratchets/all.sh            # the five ratchets
./scripts/ratchets/test-ratchets.sh  # proof that each one can fail
cargo test --workspace               # the core's property tests, the store, the HTTP API

# the crash test: kill -9 mid-effect, restart, exactly one success
cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture
```

To see a ratchet fire, add `rusqlite` to `crates/ledger-core/Cargo.toml` and run `./scripts/ratchets/all.sh` again.
