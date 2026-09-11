---
name: running-securemail-locally
description: The repo's .env carries __NEEDS_HUMAN__ placeholders, so the binary refuses to start until a throwaway key and HS256 secret are passed on the command line; and ports below 1024 cannot be bound on this host
metadata:
  type: project
---

Starting `target/debug/ods-securemail` with the repo's `.env` alone **fails at startup**:
`MASTER_ENCRYPTION_KEY` holds `__NEEDS_HUMAN__` (15 chars) and `JWT_SECRET` the same, while
`JWT_ALLOW_HS256=false` and no RSA key is set. Both are the *documented external blockers*
for deployment, not a misconfiguration to "fix" — the real values belong in Secret Manager
and only a human can deposit them.

For a local run, pass throwaway values on the command line (`dotenvy` does not override
variables already present in the environment, so they win):

```
MASTER_ENCRYPTION_KEY=$(openssl rand -hex 32) JWT_ALLOW_HS256=true \
JWT_SECRET=<at least 32 chars> PORT=8186 ./target/debug/ods-securemail
```

Second fact from the same exercise: `net.ipv4.ip_unprivileged_port_start = 1024` on the
agent host, so **no test fixture can listen on 25/465/587** — exactly the SMTP ports the
business rule accepts. A fake mail server has to sit on a high port, and the config row is
repointed at it one layer below the validation (repository, not service).

**Why:** verified on 2026-09-11 while smoke-testing the real binary for the SMTP-verify lot.
Two startup panics were spent discovering the placeholder.

**How to apply:** when a local run of this service is needed as evidence, pass the two
variables up front instead of editing `.env` (which is gitignored and holds the blocker on
purpose), and never plan a live test that needs to bind a privileged port.

Related: [[shared-db-migration-tracking]] for the database side of local runs.
