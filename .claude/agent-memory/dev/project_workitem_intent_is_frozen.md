---
name: workitem-intent-is-frozen
description: A work-unit's intent text is written once at creation and never updated, so it can describe work a previous lot already delivered — measure the repo before re-applying anything it asks for
metadata:
  type: project
---

The `intent` field of `~/dev/ops/workitems/<unit>.json` is frozen at unit creation. The
`progress.history` notes underneath it move; the intent never does. So an intent can
instruct you to apply a fix that is already committed, pushed and in an open PR.

**Why:** 2026-09-10 on `securemail-c20260909-1345`. The intent gave a full step-by-step
for removing `h2` 0.3 (HR-20260909-001) as if nothing existed. In fact both halves were
already delivered — `68b0b1b` for the manifest line and `809db72` for the ods-common
promotion — the branch was pushed, and PR #6 was open and `MERGEABLE`. Re-applying step
(1) would have been a no-op at best; treating the intent as a to-do list rather than as
context would have burned the turn. The real remaining work was: **re-measure** (the
branch had not been checked since the upstream promotion and since HR-20260910-006 fixed
the test harness), close the counter, and clear the residue. Note also that the intent's
own acceptance criterion — *« cargo audit : 0 vulnerabilities »* — is contradicted by
BR-0010 and by the runbook's own correction; the standards win over the frozen text.

**How to apply:** first three commands of any unit, before reading further into the
intent — `git log --oneline origin/dev..HEAD`, `git status --short`, and
`gh pr list --state all --head <branch>`. If the work is there, switch mode from *apply*
to *verify, finish and close*: re-run the checks end to end, fix what has gone stale in
the PR body and the review notes, commit any residue in the working tree, then update the
counter with `workitem-progress.sh`. Report what you measured, not what the intent asked.

Related: [[cargo-feature-activators]], [[shared-db-migration-tracking]]
