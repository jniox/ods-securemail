---
name: shared-db-migration-tracking
description: securemail integration tests fail VersionMissing(N) against the shared dev Postgres because _sqlx_migrations carries other services' rows — verify on a fresh database instead
metadata:
  type: project
---

`cargo test --test handler_tests` against the shared ODS dev Postgres can fail every
test with `Failed to run securemail migrations: VersionMissing(7)`. The shared
`_sqlx_migrations` table held a `version 7 — repeatable constraint validation` row that
does not exist in this repo's `migrations/` (only 001–006). sqlx refuses to migrate when
the database records a version the migration set does not contain.

The harness in `tests/handler_tests.rs` deletes versions 1–6 before migrating, so any
stray higher version survives and breaks the run.

**Why:** observed 2026-09-09. It looks like a code regression and is not one — it is
database state. Diagnosing it as a test failure wastes a cycle.

**How to apply:** do not clean the shared table — it belongs to the other services too,
and the ODS dev Postgres is shared by design. Instead create a throwaway database
(`CREATE DATABASE`, additive and permitted) and point `DATABASE_URL` at it; the six
integration tests pass there in ~16 s. Note the exec-guard refuses `DROP DATABASE`
(rule R5), so scratch databases must be cleaned up by a human — prefer a single
reused name over one per run.

Related: [[ods-common-staging-pin]]
