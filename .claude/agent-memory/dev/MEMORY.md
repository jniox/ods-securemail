# Dev Agent Memory

## ODS Shared DB Migrations
- All ODS services share a single PostgreSQL instance (ods-postgres, port 5433)
- Migrations MUST use `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`
- RLS policies need `DO $$ BEGIN IF NOT EXISTS ... END $$` wrapper
- The `ods` user is superuser and bypasses RLS, so list queries must include explicit `tenant_id` filter (defense-in-depth)
- sqlx migration runner uses `_sqlx_migrations` table for tracking; if content changes, checksums mismatch

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
- [Shared DB migration tracking](project_shared_db_migration_tracking.md) — `VersionMissing(N)` on the shared dev Postgres is database state, not a regression; verify on a fresh DB
- [Cargo feature activators](project_cargo_feature_activators.md) — for feature-gated advisories, enumerate who ACTIVATES the feature, and re-read the advisory's patched range
