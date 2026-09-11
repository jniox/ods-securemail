---
name: spec-gap
description: securemail has no spec.md anywhere in ods-platform specs; use the GTM brief as de facto contract
metadata:
  type: reference
---

`~/dev/specs/ods-platform/specs/securemail/spec.md` does not exist, and the `specs/` subdirectory
of ods-platform only contains `clm-assistant`, `clm-extraction-ocr-rag`, `clm-webapp`, `oid` — no
securemail. There is also no `pdlc/handoffs/securemail-handoff.json`.

The only available de facto contract is `~/dev/specs/ods-platform/pdlc/gtm/securemail-gtm.md`,
which itself states it was "inferred from CLAUDE.md, review reports, and ADLC state." It carries
a "Feature Summary" (Phase 1 scope) and an "Open Items" table with explicit severities and gates
(Phase 1 blocker / Phase 2 gate / GA gate) — treat the Open Items table's gate column as the
authoritative scoping decision for which findings count against Phase 1 BA compliance.

Confirmed absent again on 2026-09-10 (2nd consecutive review cycle). No securemail-specific rows
exist in `~/dev/specs/ods-platform/context/business-rules.md` either — checked full file, only
BR-0001/0003/0004/0006/0007/0008/0009, none reference securemail.

See [[review-state]] for the AC list derived from this brief and [[scoring-rule]] for how to
handle a prior review's verdict when it conflicts with the brief's own gate conditions.
