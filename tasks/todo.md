## Task: Enact HR-20260909-031 — clear RUSTSEC-2026-0258 on securemail

### Objective
Human decision HR-20260909-031 (it@orbusdigital.com, 2026-09-09T15:12:55, "approved promote
ods-common-staging") selects option « Merge and promote ods-common »:
merge jniox/ods-common PR #6 to `dev`, promote `dev` to `staging`, then reopen the securemail
work unit to bump `Cargo.lock` and lift the `#[ignore]`d graph guard.

Goal on securemail: `h2` 0.3.27 must leave the shipped graph
(`cargo tree -e normal` — the recipe criterion of BR-0010), and
`tests/framework.rs::no_h2_zero_three_in_the_shipped_dependency_graph` must run unignored.

### Plan
- [x] Verify prerequisite state: PR #6 open/mergeable, ods-common `staging` tip == the rev in securemail `Cargo.lock`
- [x] Merge ods-common PR #6 → `dev` (explicitly authorised by HR-20260909-031) — `3d5a1c5`
- [x] Promote ods-common `dev` → `staging` via PR #7 (no direct push to a protected branch) — `f8c938d`
- [x] securemail: `cargo update -p ods-common` → new staging rev in `Cargo.lock`
- [x] Red first: guard failed on the pre-promotion lockfile, passes after
- [x] Lift the `#[ignore]` on the graph guard
- [x] `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo audit`
- [x] Commit + push on `feat/securemail-c20260909-1345-lot0` (explicit paths only)
- [x] `enact-human-review.sh HR-20260909-031` + `write-status.sh securemail dev`

### Risks
- Promoting ods-common `dev` → `staging` carries every commit merged to `dev` since the last
  promotion, not only PR #6. Must enumerate that delta before merging and report it.
- Every ODS repo pinning `ods-common branch = "staging"` picks the change up on its next
  `cargo update`. Feature reduction (`actix-web` without `http2`) is the blast radius:
  any sibling serving HTTP/2 in-process would lose it. Cloud Run terminates HTTP/2 at the
  edge and speaks HTTP/1.1 to containers unless declared h2c — checked, we declare h2c nowhere.
- `cargo audit` will NOT reach zero (RUSTSEC-2023-0071 / rsa 0.9.10, no upstream fix).
  Per BR-0010 that is declared and traced, not a blocker.

### Rollback
- ods-common `staging` → `git revert` the promotion merge via a PR (never a force-push).
- securemail → `git revert` the lockfile commit; the `#[ignore]` returns with it.

### Verification
```
cargo tree -e normal -i h2@0.3.27        # must say "nothing to print" / no match
cargo tree -e normal,dev | grep 'h2 v0.3'  # no match
cargo test                                # guard runs unignored, green
cargo audit                               # RUSTSEC-2026-0258 absent
```

### Review Notes

**Outcome: the decision's objective is met.** `h2` 0.3 is absent from securemail's
shipped graph; only `h2 v0.4.19` remains, which is inside the advisory's patched range
(`>= 0.4.16`). `cargo audit` went from 2 vulnerabilities to 1.

| check | before | after |
|---|---|---|
| `cargo tree -e normal \| grep 'h2 v0.3'` | reached via `ods-common` | absent |
| `cargo tree -e normal \| grep -o 'h2 v.*'` | `h2 v0.3.27`, `h2 v0.4.19` | `h2 v0.4.19` |
| `cargo audit` | RUSTSEC-2026-0258 + RUSTSEC-2023-0071 | RUSTSEC-2023-0071 only |
| guard `no_h2_zero_three...` | `#[ignore]`d | runs, green |
| tests | 36 (1 ignored) | 37, **0 ignored** |

**The finding that changed the shape of the work.** The promotion could not carry PR #6
alone: `staging` was four commits behind `dev`, and one of them (#4) added `metrics` to
ods-common's `full` feature set. securemail asked for `full`, so the promotion would have
imported `prometheus 0.13` → `protobuf 2.28.0` → **RUSTSEC-2024-0437** into the shipped
graph at the exact moment `h2` 0.3 left it — trading one advisory for another on the very
axis BR-0010 measures. This was measured in a `/tmp` scratch checkout pinned at PR #6's
head *before* promoting, so `staging` was never moved on a guess. Fix: the manifest now
names the eight features this service consumes (`full` minus `metrics`), so the graph is
unchanged apart from `h2` 0.3 leaving. A second guard holds that line.

**Declared, not fixed (BR-0010).** `cargo audit` is not at zero and cannot be:
RUSTSEC-2023-0071 (`rsa` 0.9.10, medium 5.9) has **no upstream fix**. It is also absent
from the shipped graph — `cargo tree -e normal -i rsa` answers *"nothing to print"* —
because it is reached only through `sqlx-macros`/`sqlx-mysql`, and this service uses
postgres. Reporting this as "audit at zero" would be false; reporting it as a blocker
would be wrong.

**Left deliberately untouched.** `.env.example` carries an uncommitted edit
(`5433` → `5435`) that predates this unit and that I did not author. It is factually
correct — the `ods-postgres` container is published on `127.0.0.1:5435` and the local
`.env` already says 5435 — but it contradicts the port documented in `CLAUDE.md`, so
reconciling the two is a dev-environment concern and not part of a security decision.
Flagged in the status rather than folded into this diff.

**Note for whoever runs the integration tests.** They fail `VersionMissing(7)` against the
shared `ods` database: `public._sqlx_migrations` holds a `version 7` row belonging to
another service, and the harness only clears versions 1–6. Run them against the scratch
database `securemail_h2_verify` instead — the shared table must not be cleaned, it is
shared by design.
