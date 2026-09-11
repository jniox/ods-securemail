---
name: shared-db-migration-tracking
description: securemail's sqlx registry now lives in the securemail schema, not public — the shared-registry collapse of 2026-09-09 is fixed at both ends and needs no throwaway-database workaround
metadata:
  type: project
---

securemail's `_sqlx_migrations` registry lives in the **`securemail` schema**. It is the
service's own; cleaning it is correct and the harness does so before each run. Integration
tests run against the shared dev Postgres directly — the six pass in ~1.6 s. **No throwaway
database is needed**, and creating one would now be the wrong reflex.

**Why:** on 2026-09-09 the registry was landing in `public` and was therefore shared with
docstore. `ods_common::db::create_pool` sets `search_path TO securemail, public`, but
PostgreSQL *silently drops* a schema that does not exist from a search_path — the effective
path collapsed to `{public}` and every unqualified object fell through to it. The harness's
`DELETE FROM _sqlx_migrations` then wiped docstore's versions 1–6, and securemail died on
`VersionMissing(7)` for 16 h, counted as a code regression. It was neither a code regression
nor "shared by design": it was a bootstrap defect, since the schema meant to isolate the
registry is created by migration 001 — i.e. *after* sqlx has already chosen where to put it.

Closed at both ends under HR-20260910-006: the schema is pre-created on the shared instance
(closing the wound), and the harness's two DELETEs are qualified to
`securemail._sqlx_migrations` (removing the weapon — commit `4d4669a`). Verified after the
suite: `to_regclass('public._sqlx_migrations')` is NULL.

**How to apply:** a `VersionMissing(N)` here is worth ten seconds of checking *where the
registry actually is* before touching any code —
`select table_schema from information_schema.tables where table_name='_sqlx_migrations'`.
Never reintroduce an unqualified destructive statement in a test harness on this instance:
qualification travels with the repo, database state does not. If a scratch database is ever
genuinely needed, BR-0011 governs it — name `tmp_<service>_<YYYYMMDD>_<reason>`, and whoever
creates it deletes it before writing their final status.

Related: [[ods-common-staging-pin]]
