# AGENTS.md

## 1. Documentation Rules

Docs describe the system **as it is now**. History, process, and change narratives belong in PRs / commits / ADRs — not here.

### Don't write

- **Change narrative**: "was X, now Y"; "adjusted because of Z"; "as requested by N"
- **Temporal words**: now / recently / newly / "as of vX"; 中文：新增了 / 现已支持 / 之前 / 原本
- **Process artifacts**: task splits, debug logs, file lists, verification steps, options considered
- **Preambles**: "This document introduces..."; "The following changes..."

### Don't create

No new doc without an explicit request.

### When updating

Rewrite the whole section. Delete what is replaced. Never keep two versions side by side.

### Self-check

Read every sentence as a first-time reader. If removing the timestamp or context makes it false, delete it.

## 2. Comment Rules

- Write code that speaks for itself. Comment only when necessary to explain **WHY**, not WHAT. We do not need comments most of the time.
- ❌ AVOID — **Obvious Comments**(复述代码)、**Redundant Comments**、**Outdated Comments**、**Commented-out Code**、**Changelog Comments**、**Divider Comments**

## 3. Verify Before Done

- Build:  `make build`
- Test:   `make test`

## 4. Code Style Rules

- No `type X = Y` aliases. Use the underlying type directly; don't keep aliases for backward-compat names.
- Test merges: when tests conflict in a rebase, prefer rewriting over mechanical 3-way merge.

## 5. The Rules of This Crate

- The verification model does not change: a `Verified` result needs the marker **and** the ownership of the address, and a request that cannot be verified fails closed.
- The marker is matched byte-exact and on a word boundary; the longest marker wins.
- The request path never awaits and never holds a lock that a background task holds for long. A reverse lookup happens off the request path.
- A failure of a refresh is `Unreachable` when the environment is at fault and `Broken` when this crate is at fault. The two never change places.
- An embedded definition of `conf.d/` is a fact of this repository: an incomplete one fails the load. A definition of a host is the business of the host: an incomplete one is skipped and reported.
- Every file of `conf.d/` is listed in `EMBEDDED_CONFIGS`. The contract test checks that, and every other rule the definitions have to satisfy.
- The tests never reach the internet. The endpoints of the vendors are mocked with `wiremock`.
- The crate carries no vocabulary of a host: the labels of an admin panel belong to the host that uses the crate.
