---
name: ods-common-promotion-carries-the-delta
description: Promoting ods-common dev to staging ships every commit merged since the last promotion, and its `full` feature set grows over time — measure the downstream graph in a scratch checkout before promoting
metadata:
  type: project
---

A `dev` → `staging` promotion of a shared library is **not** the one PR you want. It
carries the whole accumulated delta, and consumers taking `features = ["full"]` silently
gain whatever features landed in between.

**Why:** 2026-09-09, HR-20260909-031. The decision read "merge PR #6, promote to staging"
and looked like a two-command errand. `staging` was in fact four commits behind `dev`,
and one of them (ods-common #4) had added `metrics` to the `full` feature set. securemail
asked for `full`, so the promotion imported `prometheus 0.13` → `protobuf 2.28.0` and
**RUSTSEC-2024-0437** into the shipped graph — at the exact moment `h2` 0.3 left it.
A security unit would have traded one advisory for another and reported a win.

**How to apply:**
1. Before promoting, enumerate the delta: `git log --oneline origin/staging..origin/dev`.
   Read every feature-set change in the library's `[features]` block, not just the PR
   you came for.
2. Measure the effect on a consumer **before** moving `staging`: copy the consumer's
   `Cargo.toml`/`Cargo.lock` to `/tmp`, repoint the git dep at the PR branch, and run
   `cargo tree -e normal`. Cheap, and it does not move a protected branch on a guess.
3. Prefer naming the features a service actually consumes over `features = ["full"]`.
   `full` is a moving target by construction; a consumer that does not expose `/metrics`
   should not be carrying a protobuf parser.

Related: [[ods-common-staging-pin]], [[cargo-feature-activators]]
