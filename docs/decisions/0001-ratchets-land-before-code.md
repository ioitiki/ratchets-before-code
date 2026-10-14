# ADR 0001: The ratchets land before the code they protect

**Status:** accepted
**Date:** 2026-10-14
**Decided by:** Andy

## Context

This repository is built mostly by AI coding agents working from written
plans. Agents are fast, and they are obliging. Asked to make a test pass by the
end of a task, a good agent finds the shortest path, and the shortest path
through an architecture usually runs through one of its boundaries. A person
under a deadline does the same, just less often.

Each such exception is defensible on the day it is made. The damage is in the
sum, and nobody ever approves the sum. A design document cannot stop it,
because a document has no way to fail a build. Review does not reliably stop
it either: the reviewer who should catch the fifth exception is the one who
waved through the first four.

## Decision

Every structural rule this repository depends on is a script that fails the
build, and those scripts are in the first commit, before any crate exists.

So the first commit holds:
- this decision
- the design spec
- five ratchets under `scripts/ratchets/`
- a self-test that proves each ratchet can fail
- the CI workflow that runs them before anything else

## Why first

- **Today they cost nothing.** Against an empty tree, every ratchet passes on
  its first run. A ratchet introduced later arrives with every violation that
  accumulated before it, and its first conversation is about which of those
  really count.
- **A rule that runs is a different thing from a rule that is written down.**
  "The core is pure" is a sentence in the spec. `core-purity.sh` is the same
  sentence, checked on every pull request, including the ones an agent opens at
  3 a.m.
- **A budget binds only when it starts below need.** `core-line-budget.sh`
  allows 800 lines for a core expected to settle at 400-600. Starting at
  whatever size the core reaches, plus some slack, would record the code's size
  rather than limit it.

## Consequences

- **Dependency rules are allowlists.** A denylist blocks only the crate
  someone thought of in advance.
- **Source scans skip whole-line comments.** Otherwise the explanation of a
  rule becomes its first violation, and people learn to silence the check.
- **Loosening a rule is a visible diff.** Raising a budget or widening an
  allowlist is allowed, and it is done by editing the ratchet in the same diff
  as the code that needs it. The trade is then in front of the reviewer, not in
  a setting nobody reads.
- **Every ratchet ships with fixtures that prove it can fail**
  (`test-ratchets.sh`). That also stops a ratchet from being weakened quietly:
  an agent that "fixes" a red build by loosening the check turns the self-test
  red instead.
- **They are a floor, not a proof.** The ratchets are text scans and metadata
  checks, and someone set on evading them can. What they catch is the
  ordinary, reasonable shortcut.
