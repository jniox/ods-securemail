# Dev Agent Memory

## ODS Shared DB Migrations
- All ODS services share a single PostgreSQL instance: `ods-postgres` (see CLAUDE.md for the DSN).
  The agent host publishes SEVERAL postgres on adjacent ports (5432 host, 5433 `honcho-poc`, 5434 `otodesk`,
  5435 `ods-postgres`). **An open port is not proof of the right server** — name the listener's owner with
  `docker ps --format '{{.Names}} {{.Ports}}'` before trusting a DSN.
- Migrations MUST use `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`
- RLS policies need `DO $$ BEGIN IF NOT EXISTS ... END $$` wrapper
- The `ods` user is superuser and bypasses RLS, so list queries must include explicit `tenant_id` filter (defense-in-depth)
- sqlx migration runner uses `_sqlx_migrations` for tracking; if content changes, checksums mismatch
- Each service's registry must live in ITS schema: an absent schema is silently dropped from `search_path`,
  so any UNQUALIFIED statement lands in `public` and hits the neighbours — always write `<schema>._sqlx_migrations`

## Rust/Actix-web Service Pattern (doceditor, form-engine)
- Domain models in `src/domain/`
- Repository layer with `begin_tenant_tx(pool, tenant_id)` for RLS context
- Service layer orchestrates repo + events
- API handlers use `AuthUser` extractor (FromRequest)
- CloudEvents via InMemoryProducer in tests, Redpanda in prod
- Tests use real DB with idempotent migrations

## Common Pitfalls
- COUNT queries must NOT have ORDER BY clause (PostgreSQL error 42803)
- Dynamic query builders: separate ORDER BY logic from WHERE clause building
- `build_list_query` param_idx must account for pre-bound params (e.g., tenant_id = $1, so dynamic starts at $2)

## Indexed memories
- [ods-common staging pin](project_ods_common_staging_pin.md) — upstream fixes merged to ods-common's `dev` do NOT reach this repo until promoted to `staging`
- [ods-common promotion delta](project_ods_common_promotion_delta.md) — a `dev`→`staging` promotion ships the whole delta; `full` grows, measure the consumer graph first
- [Shared DB migration tracking](project_shared_db_migration_tracking.md) — registry now isolated in the `securemail` schema; check WHERE the registry is before blaming code; no throwaway DB needed
- [Cargo feature activators](project_cargo_feature_activators.md) — for feature-gated advisories, enumerate who ACTIVATES the feature, and re-read the advisory's patched range
- [Work-item intent is frozen](project_workitem_intent_is_frozen.md) — the intent text never updates; measure the branch before re-applying what it asks
- [Running securemail locally](project_running_securemail_locally.md) — the repo `.env` is a placeholder: pass a throwaway key + HS256 secret; ports <1024 cannot be bound here
