# securemail

## Stack
Rust (Actix-web) — ods-common (auth, error, events, db, health, middleware)

## Project
ods-platform

## Architecture
- Domain models: `src/domain/` (MailConfig, EncryptionKey, Template, Email)
- Service layer: `src/service/` (business logic, orchestrates repo + events)
- API handlers: `src/api/` (HTTP handlers, health)
- Repository: `src/repository/` (PostgreSQL via sqlx, RLS via ods-common::db)
- Events: `src/events/` (Redpanda/Kafka CloudEvents via ods-common::events)
- Config: `src/config.rs` (env-based config)
- Error: `src/error.rs` (re-exports ods-common::error::AppError)
- Entrypoint: `src/main.rs`

## ods-common Integration
- **Auth**: `ods_common::auth::{JwtConfig, AuthContext, Claims}` — JWT validation + Actix extractor
- **Error**: `ods_common::error::{AppError, AppResult}` — RFC 7807 error responses
- **Events**: `ods_common::events::{CloudEvent, EventProducer, RedpandaProducer}` — CloudEvents v1.0
- **DB**: `ods_common::db::{create_pool, begin_tenant_tx}` — PG pool + RLS tenant context
- **Health**: `ods_common::health::{health_handler, HealthConfig}` — /health endpoint

## API Endpoints
- `GET /health` — public health check
- `GET /ready` — public readiness probe (DB check)
- `POST /api/v1/mail-configs` — create mail config
- `GET /api/v1/mail-configs` — list mail configs (paginated)
- `GET /api/v1/mail-configs/{id}` — get mail config
- `PUT /api/v1/mail-configs/{id}` — update mail config
- `DELETE /api/v1/mail-configs/{id}` — soft-delete mail config
- `POST /api/v1/mail-configs/{id}/verify` — verify SMTP connection

## Multi-Tenancy
- JWT auth via ods-common (AuthContext extractor with tenant_id)
- All DB queries use `begin_tenant_tx` which sets `app.tenant_id` via RLS
- RLS policies on all tables enforce tenant isolation
- All events include tenant_id

## Database
```
PostgreSQL 17 — ods-postgres container
Host: 127.0.0.1:5435
User: ods / Password: ods-dev-2026 / DB: ods
Schema: securemail
Tables: mail_configs, encryption_keys, templates, emails, email_events
Connection: postgres://ods:ods-dev-2026@127.0.0.1:5435/ods
```

## Environment Variables
- `DATABASE_URL` (required)
- `PORT` / `SERVICE_PORT` (default: 8086)
- `KAFKA_BROKERS` / `REDPANDA_BROKERS` (default: localhost:9092)
- `KAFKA_TOPIC` / `REDPANDA_TOPIC` (default: ods.securemail.events)
- `MASTER_ENCRYPTION_KEY` (required, 32-byte hex string for AES-256)
- JWT: `JWT_RSA_PUBLIC_KEY_B64` + `JWT_ISSUER` + `JWT_AUDIENCE` (RS256 production)
  or `JWT_ALLOW_HS256=true` + `JWT_SECRET` (HS256 dev only)
- `LOG_LEVEL` (default: info)
- `CORS_ALLOWED_ORIGINS` (comma-separated, default: empty)

## Tests
```bash
cargo test                              # All tests
cargo test --lib                        # Unit only
cargo clippy -- -D warnings             # Lint
cargo fmt --check                       # Format check
```
