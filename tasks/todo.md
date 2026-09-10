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

**The dangling `.env.example` edit, now closed (2026-09-10).** The previous pass found an
uncommitted `5433` → `5435` edit it had not authored and left it alone, because it
contradicted the port documented in `CLAUDE.md` and nothing said which of the two was
right. It is now measured rather than guessed: `127.0.0.1:5433` is
`honcho-poc-database-1` (`pgvector/pgvector:pg15`), an unrelated project, while
`ods-postgres` publishes on `127.0.0.1:5435`. Both ports answer, which is exactly why the
wrong one is a trap and not a connection error — `.env.example` was pointing a fresh
checkout at another project's database with ODS credentials. The edit is therefore
committed and `CLAUDE.md` aligned with it, in a `docs` commit kept separate from the
security diff.

**The integration-test note above is obsolete.** It said the six integration tests fail
`VersionMissing(7)` against the shared `ods` database and must be run against the scratch
database `securemail_h2_verify`. Both halves have since been fixed elsewhere:
HR-20260910-006 created the `securemail` schema on `ods-postgres` and commit `4d4669a`
qualified the harness's two `DELETE FROM _sqlx_migrations` to
`securemail._sqlx_migrations`. Measured today against the shared `ods` database on 5435:
**37 tests, 0 failed, 0 ignored** — 28 unit + 3 guards + 6 integration. The scratch
database `securemail_h2_verify` no longer exists (purged under BR-0011); do not recreate
it.

### Re-verification, 2026-09-10

Re-run end to end on `feat/securemail-c20260909-1345-lot0` before closing the unit, since
the branch had not been re-measured since the ods-common promotion:

| check | result |
|---|---|
| `cargo tree -e normal \| grep 'h2 v0.3'` | no match |
| `cargo tree -e normal -i h2` | `h2 v0.4.19` only, via `hyper` ← `tonic`/`reqwest` (patched, `>= 0.4.16`) |
| `cargo tree -e normal -i rsa` | *nothing to print* — RUSTSEC-2023-0071 not shipped |
| `cargo tree -e normal \| grep 'protobuf v2.'` | no match — the second guard still holds |
| `cargo audit` | 1 vulnerability: RUSTSEC-2023-0071 only. **RUSTSEC-2026-0258 absent.** |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo test` | 37 passed, 0 failed, 0 ignored |

No `.cargo/audit.toml` exists in this repo, so there was no `RUSTSEC-2026-0258` ignore to
remove — the advisory is cleared on the graph, not silenced.

**Also declared, not fixed (BR-0010).** `cargo audit` reports six allowed warnings. None
is a vulnerability and none is in scope for a decision about `h2`, but they are traced
here rather than left implicit, with their shipped-graph status measured one by one
(`cargo tree -e normal -i <crate>`):

| crate | warning | in the shipped graph? |
|---|---|---|
| `chacha20 0.10.0` | yanked | yes, via `rand 0.10.1` |
| `anyhow 1.0.102` | unsound (RUSTSEC-2026-0190) | yes, direct dependency |
| `rustls-pemfile 2.2.0` | unmaintained (RUSTSEC-2025-0134) | yes, via `sqlx`'s rustls TLS |
| `proc-macro-error2 2.0.1` | unmaintained (RUSTSEC-2026-0173) | yes, proc-macro edge |
| `event-listener 5.4.1` | unsound (RUSTSEC-2026-0221) | two major lines coexist |
| `spin 0.9.8` | yanked | **no** — reached only via `sqlx-mysql`/`sqlx-sqlite` |

The one that is tempting to "fix" is `chacha20`: it arrives through `rand 0.10.1`, which
this manifest pins at `>= 0.9.3` on purpose for RUSTSEC-2026-0097. Downgrading that pin to
dodge a *yanked* warning would reopen a real advisory. Left as is, deliberately. The rest
belong to a dependency-hygiene unit, not to this one — the runbook's §8 is explicit that
one unit carries one subject.
