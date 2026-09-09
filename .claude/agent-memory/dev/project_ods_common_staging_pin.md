---
name: ods-common-staging-pin
description: securemail consumes ods-common via git branch = "staging", so upstream fixes merged to ods-common's dev do NOT reach this repo until someone promotes dev to staging
metadata:
  type: project
---

securemail's `Cargo.toml` pins `ods-common = { git = ..., branch = "staging", ... }`.
An upstream fix merged to ods-common's `dev` is therefore **invisible here** until
someone promotes `dev` → `staging`. Cargo resolves the branch tip and records the rev
in `Cargo.lock`, so a `cargo update -p ods-common` is also needed after promotion.

**Why:** hit on 2026-09-09 while enacting HR-20260909-001 (drop `h2` 0.3 /
RUSTSEC-2026-0258). The platform runbook and the sibling agent both spoke of "landing
on dev", which is one promotion short of unblocking this repo. Raised as
HR-20260909-031 and **resolved the same day**: ods-common #6 → `dev` (`3d5a1c5`),
then #7 promoted `dev` → `staging` (`f8c938d`), and this repo's lock now points there.

**How to apply:** when a fix in ods-common is a prerequisite here, check
`gh api repos/jniox/ods-common/commits/staging` — not the PR's merge state — and compare
that sha to the `#<rev>` recorded in this repo's `Cargo.lock`. Equal sha means the fix
has *not* arrived, whatever the PR says. This applies to every ODS repo taking
ods-common by git branch. And see [[ods-common-promotion-carries-the-delta]] before
promoting: the promotion is never just the one PR you care about.

Related: [[cargo-feature-activators]], [[ods-common-promotion-carries-the-delta]]
