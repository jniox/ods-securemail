# Scenario Agent Memory — doceditor / ods-platform

## Key patterns

### doceditor has no spec.md
`~/dev/specs/ods-platform/specs/doceditor/spec.md` does not exist.
Derive scenarios from:
1. `~/dev/projects/doceditor/CLAUDE.md` — endpoint list, auth model, env vars
2. `src/api/documents.rs`, `src/api/versions.rs` — request/response shapes
3. `src/domain/document.rs` — business rules (DocumentStatus transitions)
4. `src/service/document_service.rs` — validation logic (BR-001, BR-022, BR-029)
5. `~/dev/ops/reviews/doceditor/ba-report.json` — AC list and deviations

### Auth model
doceditor does NOT validate JWT. It reads X-Tenant-Id and X-User-Id headers
directly (UUID strings). Missing or malformed UUIDs return 401. This is a BA
deviation (MEDIUM severity) from the ODS standard JWT/OID flow.

### Error codes
- Validation errors: 422 with {"error":"validation_error"}
- Not found: 404 with {"error":"not_found"}
- Invalid status transition: 400 with {"error":"bad_request"}
- Unauthorized: 401 with {"error":"unauthorized"}

### Multi-tenancy result codes
Cross-tenant access returns 404 (not 403) because RLS makes the row invisible.

### Pagination clamping
per_page is clamped to [1, 100]. page is clamped to min 1.

### Status transitions (BR-002)
Allowed: draft->published, draft->archived, published->archived.
Forbidden: published->draft, archived->anything, same->same.

### Output locations
- Scenarios: ~/dev/ops/scenarios/doceditor-e2e.json
- Mock data: ~/dev/projects/doceditor/tests/e2e/mock-data.sql
- Cleanup: ~/dev/projects/doceditor/tests/e2e/cleanup.sql
- Status: ~/dev/ops/outputs/doceditor-scenario.status
