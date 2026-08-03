# Security Agent Memory — securemail

## Last Review
- Date: 2026-06-03
- Commit: f69dfca
- Result: CONCERNS, severity=MEDIUM, OWASP 8/10

## Key Findings to Track
1. **SEC-002 — Dead hex fallback (MEDIUM)**: `src/main.rs:66-68` — `hex::decode().unwrap_or_else(|_| raw bytes)` is logically unreachable because config.rs rejects non-hex keys at startup. Should be replaced with `.expect()` to make the invariant explicit.
2. **No RBAC (MEDIUM)**: All endpoints authenticate via AuthContext but no role check. Any tenant member can delete configs. Persistent from previous review.
3. **.gitignore missing key patterns (LOW)**: `.gitignore` covers `.env` and `/target` but not `*.pem`, `*.key`, `*.cert`.

## Resolved Findings
- **SEC-001 XOR encryption (HIGH)**: RESOLVED at f69dfca. AES-256-GCM confirmed via `aes-gcm` crate. `encrypt_password()` uses `OsRng` nonce, output is `nonce(12) || ciphertext || auth_tag(16)`.
- **max_body_size not wired (MEDIUM)**: RESOLVED. `web::JsonConfig::default().limit(max_body_size)` and `web::PayloadConfig::default().limit(max_body_size)` wired in main.rs:92-93.
- **MASTER_ENCRYPTION_KEY validation (was missing)**: RESOLVED. config.rs validates: non-empty, not-all-zero, exactly 64 hex chars, valid hex charset. 6 unit tests added.

## Architecture Notes
- Stack: Rust + Actix-web + sqlx + ods-common
- Auth: ods-common AuthContext extractor (RS256 JWT). HS256 explicitly disabled in .env.example.
- DB: all 5 tables have RLS + begin_tenant_tx on every query
- CORS: restrictive (no wildcard, origin allowlist from env)
- Encryption: AES-256-GCM, MASTER_ENCRYPTION_KEY (required, exactly 64 hex chars / 32 bytes)
- Body size: max_body_size wired to both JsonConfig and PayloadConfig (25 MiB default)
- Dockerfile: BuildKit secret mount for github_token (no token in image layers), non-root user

## Patterns Confirmed
- All SQL uses parameterized .bind() — no injection risk
- validator::Validate on all request DTOs (CreateMailConfig, UpdateMailConfig)
- RLS on: mail_configs, encryption_keys, templates, emails, email_events
- rand pinned >=0.9.3 (RUSTSEC-2026-0097 addressed)
- Passwords never returned in API responses (MailConfigResponse strips encrypted fields)
- Events emitted: config_created, config_verified (not update/delete)

---

# Security Agent Memory — doceditor

## Last Review
- Date: 2026-05-02
- Commit: c0ec8cc
- Result: CONCERNS, severity=HIGH, OWASP 6/10

## Critical Findings
1. **JWT auth bypass (CRITICAL)**: `src/api/extractors.rs:20` — AuthUser reads X-Tenant-Id and X-User-Id headers with zero JWT validation. Any caller can forge tenant identity. jsonwebtoken crate in Cargo.toml but never imported.
2. **Tenant isolation breaks with forged header (CRITICAL)**: RLS is set correctly via begin_tenant_tx but fed a caller-controlled value. Consequence of #1.
3. **No body size limit (MEDIUM)**: max_document_size_mb loaded from AppConfig but never wired to web::JsonConfig or web::PayloadConfig in main.rs.
4. **LIMIT/OFFSET interpolation (MEDIUM)**: src/repository/document_repo.rs:110-113 — format!(" LIMIT {l}") instead of .bind(limit). Safe today via i64 clamp() but policy violation.
5. **No RBAC (MEDIUM)**: No role checks. Any tenant user can delete/publish/archive any document.
6. **No explicit tenant filter on single-resource queries (MEDIUM)**: get_document, update_document, delete_document, all version queries lack tenant_id = $N. Relies solely on RLS.
7. **Auth failures not logged (MEDIUM)**: Extractor returns Unauthorized silently.

## Architecture Notes
- Stack: Rust + Actix-web 4.13 + sqlx 0.8.6 + rdkafka 0.36.2
- Auth: Header-based STUB — X-Tenant-Id + X-User-Id (NOT JWT)
- DB: RLS on documents, document_versions, templates via begin_tenant_tx
- Events: NoopProducer in production (Redpanda not wired)
- CORS: actix-cors declared but NOT applied in App builder

## Patterns Confirmed
- All SQL except LIMIT/OFFSET uses parameterized .bind()
- list_documents has explicit tenant_id filter (defense-in-depth)
- validate_metadata() enforces key count + key name format
- Internal error details are masked from HTTP responses (error.rs:51-54)
- No secrets in source code
