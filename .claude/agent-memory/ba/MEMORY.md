# BA Agent Memory — securemail

## Stable Facts

- spec.md does NOT exist at ~/dev/specs/ods-platform/specs/securemail/spec.md
  - The specs/ subdirectory does not exist in ods-platform at all
  - Use CLAUDE.md + task description as authoritative reference when spec is absent
- Service uses Rust + Actix-web (NOT Go — the tasks/todo.md mentions Go but that was the original plan, Rust was chosen)
- Commit 973a28c is the Phase 1 implementation commit

## Review History

| commit | date | status | summary |
|--------|------|--------|---------|
| 973a28c | 2026-05-02 | non-compliant | 2/5 MET, 2/5 PARTIAL, 1 N/A. HIGH: XOR not AES-256-GCM. MEDIUM: max_body_size unapplied, no HTTP handler tests |

## Known Deviations (unresolved as of 973a28c)

1. **HIGH** — `encrypt_password()` at `src/service/mail_config_service.rs:31` uses XOR cipher, not AES-256-GCM. Comment says "placeholder". Must be fixed before BA can pass AC-005.
2. **MEDIUM** — `AppConfig.max_body_size` configured but never applied to Actix `JsonConfig`/`PayloadConfig` in `src/main.rs`.
3. **MEDIUM** — No HTTP-layer tests for mail config handlers (`src/api/mail_configs.rs`). No 401/404/409 coverage.
4. **MEDIUM** — `verify` endpoint is a stub — always returns `verified=true` without real SMTP connection.
5. **LOW** — `update` operation does not emit `CONFIG_UPDATED` CloudEvent.

## What AC MET Means Here

- AC-001 (health): fully MET
- AC-003 (multi-tenancy / RLS): fully MET — all 5 tables have RLS, begin_tenant_tx everywhere
- AC-004 (CloudEvents): fully MET — CONFIG_CREATED + CONFIG_VERIFIED emitted, specversion=1.0

## For Next Review

If dev agent fixes XOR → AES-256-GCM in `encrypt_password()`, AC-005 becomes MET.
If HTTP handler tests added for 401/404/409, AC-002 becomes MET.
Both together → compliant (assuming no new deviations).
