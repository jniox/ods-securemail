## Task: Initialize SecureMail service and implement base structure (AC-1)

### Objective
Set up the Go project with all dependencies, domain models, repository interfaces, API skeleton, event types, database migrations with RLS, and health endpoints. All with TDD.

### Plan
- [x] Generate SecureMail spec
- [ ] Initialize Go module (go mod init, dependencies)
- [ ] Domain models + validation + unit tests (TDD)
- [ ] Repository interfaces
- [ ] Database migrations with RLS policies
- [ ] API handlers (health, email, template, smtp-config) + tests
- [ ] Event producer (CloudEvents) + tests
- [ ] Configuration loading
- [ ] Wire main.go
- [ ] Quality gates (go test, go vet, staticcheck)

### Risks
- No existing ODS Go service to reference patterns from (OID is Rust)
- SMTP password encryption needs careful key management

### Rollback
- All changes on `dev` branch, can `git reset` to d44d844

### Verification
- `go test ./...` all green
- `go vet ./...` clean
- `go build ./cmd/securemail` succeeds
- Health endpoint responds 200
