---
name: scoring-rule
description: Do not inherit a prior review's PASS verdict when its own notes contradict it — re-derive that criterion from source
metadata:
  type: feedback
---

The service wiki (`~/dev/docs/entities/securemail.md`) records a `ba R2 PASS (compliant) 5/5 MET`
entry dated 2026-06-03 whose own note says "WARN: verify stub open (BA-001, Phase 1 gate)" in the
same breath as declaring full compliance. The GTM brief states explicitly that BA-001 resolution is
*required* for the 5 ACs to be MET — so that cycle's own PASS verdict contradicts its own evidence.

**Why**: presence-of-endpoint-shape (route exists, returns 200) was apparently treated as
sufficient without checking whether the endpoint's defining behavior (an actual SMTP dial) was
implemented. Same pattern flagged platform-wide this same week on pdf-engine (job-lifecycle API
shape credited without a rendering call site) and ujuzai-demo-booking (env var presence credited
without a consumption call site).

**How to apply**: when reusing or referencing a prior review's verdict for any criterion, check
whether that review's own notes/WARNs contradict the verdict it assigned. If they do, do not
inherit the verdict — re-derive that specific criterion from source (grep the actual
implementation, not just the route table). This is stronger than [[review-state]]'s AC-by-AC
history: it's about *how* to treat a past verdict you find, not just what the current state is.

General form of the check, reusable for any service: does the GTM brief / spec state a compound
gate condition ("all N AC MET requires X and Y resolved")? If so, treat X and Y as scoring
criteria in their own right, not as separate non-blocking notes layered on top of a PASS.
