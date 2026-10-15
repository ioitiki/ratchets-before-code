# ADR 0006: The merge commits keep their author email

**Status:** accepted. It is an exception to one check in spec §9.
**Date:** 2026-10-14
**Decided by:** Andy

## Context

Spec §9 says the repository becomes public only after a scan of its full history finds no email address other than the noreply author address. It also says "Any match blocks publishing."

The scan found one. The seven merge commits for PRs #1 to #7 name the operator's personal address as their author:

- `af30c3d`, `4a646df`, `af403b9`, `778ccd5`, `37e97d5`, `83c8379`, `5d1c8d3`

GitHub stamped them. The coordinator merged those PRs through GitHub's API, and GitHub writes the account's primary email into a merge commit it creates. Every other commit uses the noreply address from the repository's local configuration.

The same address already appears in public commits in two of the operator's other public repositories. Publishing this repository exposes nothing that isn't already public.

## Decision

The merge commits stay as they are. History is not rewritten to change their author.

From now on, the coordinator merges locally with `git merge --no-ff`, under the repository's noreply identity, and pushes `main`. It does not merge through GitHub's API.

## Consequences

- **The record stays intact.** The reports quote dozens of commit hashes. Rewriting the merges would change every commit after the first one, and every one of those references would point at a commit that no longer exists.
- **GitHub keeps the old merges anyway.** Its merged-PR pages link to the original merge commits, so rewriting `main` would not remove the address from the hosted repository.
- **§9's email check now has one recorded exception.** These seven commits, this address. Any other address the scan finds still blocks publishing.

## Alternatives rejected

- **Rewrite history to scrub the address.** Spec §3 allows it before first publication. It is rejected here because the address is already public, and the rewrite would break the record that this repository exists to keep.
