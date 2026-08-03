# Merge Agent Memory — securemail

## Key patterns

### Chore-waiver scope (learned PR #4 vs #5)
- A chore-waiver (E2E SKIP / Coverage SKIP) is only valid when the PR diff contains NO application code changes.
- PR #4: Dockerfile-only → chore-waiver valid.
- PR #5: Dockerfile + .env.example + src/config.rs → chore-waiver INVALID. Application code in diff requires real E2E and coverage gate evaluation.
- Rule: if any file under `src/` appears in the PR diff, SKIP status for E2E and Coverage gates is a gate FAIL, not a waiver.

### Security gate — SEC-001 (HIGH) securemail
- SEC-001: MASTER_ENCRYPTION_KEY must be validated for exactly 32 bytes (64 hex chars) at startup. Empty and all-zero checks are necessary but not sufficient.
- PR #5 added empty + all-zero rejection but NOT the 64-char hex length check. SEC-001 remains HIGH until length validation is added.
- Fix needed: `if trimmed_key.len() != 64 { return Err(...) }` (after hex validation) in config.rs.

### Label: human-review-required vs human-required
- The bypass protocol exact trigger is `human-required`.
- `human-review-required` (present on PR #5) does NOT trigger the automatic bypass but DOES warrant Slack escalation to DM.
- Always escalate human-review-required to DM even when proceeding (or rejecting) on gate logic.

### securemail gate history
- PR #4 (2026-06-02): merged. Rust 1.88→1.96.0 CVE fix. Chore-waiver valid.
- PR #5 (2026-06-03): rejected. SEC-001 HIGH + E2E/Coverage SKIP invalid + human-review-required label.

## Files
- Gate reports: /home/jniox_orbusdigital_com/dev/ops/reviews/securemail/
- Decisions JSONL: /home/jniox_orbusdigital_com/dev/ops/outputs/merge-decisions.jsonl
- Status: /home/jniox_orbusdigital_com/dev/ops/outputs/securemail-merge.status
