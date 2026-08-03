# Security Responder Memory

## Key Paths
- Findings: `~/dev/ops/innovation/findings/YYYY-MM-DD/FIND-*.json`
- Impact reports: `~/dev/ops/reviews/security-batch-YYYY-MM-DD.json`
- Deployed versions: `~/dev/ops/innovation/last-versions.json`
- Wiki log: `~/dev/docs/log.md`
- Entity docs: `~/dev/docs/entities/$SERVICE.md`
- Publish script: `~/dev/ops/adlc-v2/scripts/cli/publish-doc.sh`

## Infrastructure Versions (check last-versions.json first)
- `docker-engine` and `traefik` versions are tracked in `last-versions.json`
- These are infrastructure-level, not service dependencies — verify deployed version against CVE fix threshold
- Do NOT patch code for infrastructure CVEs; document as not_affected or patched_at_deploy

## cargo-audit Not Available
- cargo-audit is not installed on the agent machine (2026-04-30)
- Do manual version analysis: grep Cargo.lock for crate names + versions, compare against known RUSTSEC advisories
- govulncheck also not available for Go services

## npm audit False Positives: Strapi v5
- npm audit shows Strapi v4 advisory ranges against Strapi v5 packages (elliptic, grant, jwk-to-pem)
- These are false positives: fix = "4.26.1 isSemVerMajor=True" means v4 fix, not applicable to v5 track
- vite 5.4.21 is PATCHED for GHSA-4w7w-66w2-5vf9 (v5 fix at >=5.4.15, npm audit shows v6 range)
- esbuild: range was <=0.24.2; anything > 0.24.2 is patched

## Slack Message Formatting
- Do NOT use python3 -c with multiline strings in Slack payloads — use heredoc or temp JSON file
- Pattern: `cat > /tmp/slack_msg.json << 'ENDJSON'` then `curl -d @/tmp/slack_msg.json`

## Services Map
- Rust (Cargo.lock): oid, docstore, pdf-engine, notification-hub, billing-engine, form-engine, ods-clm-engine, ods-clm-extraction-ia, ods-common
- Go (go.mod/go.sum): agenda, securemail, doceditor, workflow-engine
- Node (package-lock.json or pnpm-lock.yaml): ods-dashboard (pnpm), nextjs-frontend (npm), strapi-cms (npm)
- workflow-engine: has Dockerfile (Go) but no Cargo.lock — it's a Go service despite being listed with Rust

## strapi-cms repo
- GitHub: `lejecos-strapi-cms` (not ods-platform)
- On `dev` branch for security patches

## Observability
- Always publish markdown report to dashboard before Slack post
- Category: `incident-security`, source: `security-responder`
