---
name: review-state
description: Current securemail BA compliance state and the 7-criterion AC list derived from the GTM brief
metadata:
  type: project
---

Latest review: 2026-09-10, commit `042d36c`, verdict **non-compliant** (5/7 MET, 1 PARTIAL, 1 MISSING).
Full report: `~/dev/ops/reviews/securemail/ba-report.json`.

**Why this matters going forward**: the AES-256-GCM HIGH deviation from the first cycle
(commit `973a28c`, 2026-05-02) is now genuinely resolved — verified live with 37/37 tests against
real PostgreSQL, not just re-reading the source. Do not re-flag encryption as a deviation unless
`src/service/mail_config_service.rs::encrypt_password`/`decrypt_password` regress away from
`aes-gcm`.

## AC list (source: pdlc/gtm/securemail-gtm.md, no spec.md exists — see [[spec-gap]])

1. AC-001 Health/readiness probes — MET, stable across cycles.
2. AC-002 Mail config CRUD — MET, stable across cycles.
3. AC-003 Multi-tenant isolation (JWT tenant_id + RLS on all 5 tables) — MET, stable across cycles.
4. AC-004 CloudEvents v1.0 (config.created, config.verified) — MET, stable across cycles.
5. AC-005 AES-256-GCM encryption at rest — **MET as of 2026-09-10** (was the HIGH deviation in
   cycle 1, apparently mis-scored MET in the 2026-06-03 wiki-recorded cycle — see [[scoring-rule]]).
6. AC-006 Real SMTP dial on `verify` endpoint (GTM Open Item BA-001, HIGH) — **still MISSING**,
   unresolved since cycle 1. `src/service/mail_config_service.rs:184-198` is still a stub
   (`verified=true` unconditionally). Zero SMTP/TCP-dial crate in the dependency graph — check this
   with `grep -rn "TcpStream\|lettre\|smtp::" src/` + `grep -i "^name = \"lettre\"" Cargo.lock`
   before crediting this AC, every cycle.
7. AC-007 HTTP handler tests for 401/404/409 (GTM Open Item BA-002, MEDIUM) — **PARTIAL**. 401/404
   are now covered end-to-end (`tests/handler_tests.rs`, new as of this cycle — previously zero
   handler tests existed). 409 is still only unit-tested via direct `AppError::Conflict`
   construction (`src/error.rs`), never through an actual duplicate-name POST via the API route.

## Explicitly out-of-scope for Phase 1 compliance (per GTM brief's own Open Items "Gate" column)

Do not count these against compliance unless the GTM brief's rollout plan changes:
- DEV-002 `CONFIG_UPDATED` event not emitted on update — Phase 2 gate.
- SEC-002 no RBAC role guard on mutating endpoints — Phase 2 gate.
- ARCH-001 `KAFKA_BROKERS` silently defaults instead of being required — GA gate.
- MIG-001 (XOR→AES re-encryption migration plan) and CVE-001 (PG 17.8+ verification) — Phase 1
  *prod promotion* gates, not Phase 1 dev/staging BA gates.

Still worth noting as LOW deviations in the report even when N/A to the verdict.

## Live test command (works — ods-postgres runs on 127.0.0.1:5435, matches .env)

```
source ~/.cargo/env
DATABASE_URL="postgres://ods:ods-dev-2026@127.0.0.1:5435/ods" \
MASTER_ENCRYPTION_KEY="$(openssl rand -hex 32)" \
JWT_SECRET="ods-common-test-secret-32-chars!" \
JWT_ALLOW_HS256=true \
cargo test --offline
```
37 tests total as of 2026-09-10 (28 unit + 3 dependency-graph guards + 6 handler integration).
