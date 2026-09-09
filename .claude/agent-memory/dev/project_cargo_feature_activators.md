---
name: cargo-feature-activators
description: When a Rust advisory arrives through an optional dependency, enumerate which crates ACTIVATE the feature, not which package depends on it — one consumer taking defaults re-enables it for the whole graph
metadata:
  type: project
---

An advisory reached through a Cargo **feature** is not cleared by fixing the top-level
manifest alone. Cargo unifies features across the whole graph: if any consumer takes the
dependency with its default feature set, the feature is on for everyone.

**Why:** 2026-09-09, RUSTSEC-2026-0258 / `h2` 0.3 on securemail. The platform runbook
established that `actix-http` declares `h2` optional behind `http2`, checked that `h2` 0.3
had a single *dependant package*, and concluded the repo's own `actix-web = "4"` was the
only activator. On securemail there was a second one — `ods-common` declaring
`actix-web = { version = "4", optional = true }` with no `default-features = false` — so
the one-line fix left `h2` 0.3.27 in the lockfile. The runbook's premise held for
`actix-cors` and `tracing-actix-web`, which do opt out, and missed the git dependency.

**How to apply:** two cheap checks, neither needing a compile.
1. `cargo tree -e features -i <crate>` and then read *every* consumer's manifest for
   `default-features = false`. `cargo tree -i <crate>` errors "ambiguous" when two major
   lines coexist — use `-i <crate>@<version>` to disambiguate.
2. Re-read the advisory's **patched range** before believing any claim that a newer line
   is safe. RUSTSEC-2026-0258 also covered `h2` 0.4 below 0.4.16, while the runbook stated
   the 0.4 line "has the right to stay". Ranges widen as new versions are found affected.

Also, in actix-web every `rustls-*` and `openssl` feature re-enables `http2` without naming
it — which is why the durable guard must ask `cargo tree`, not the manifest text.

Related: [[ods-common-staging-pin]]
