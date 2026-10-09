# Operations

This guide describes production-facing procedures and invariants. Development conventions live in [`.rules`](../.rules), and executable verification commands live in [Testing](./TESTING.md).

## Runtime Topology

- `backend-admin` listens on container port `8080`, owns the admin database, school records, tenant database provisioning metadata, and deployment coordination.
- `backend-school` listens on container port `8081`, calls backend-admin over the internal network, resolves the tenant from the request, and connects to that tenant's PostgreSQL database.
- `frontend-admin` is the administrative web application.
- `frontend-school` is built once per input/configuration and deployed per tenant/subdomain and calls the school API.

Local source-build topology is defined in [`compose.local.yml`](../compose.local.yml), and the production topology is defined in [`podman-compose.yml`](../podman-compose.yml). Use Docker Compose locally and rootless `podman-compose` in production. The production file is the sole Compose owner for both backends, Nginx, clamd, their explicitly named networks, and the scanner volume. The production host publishes backend ports only on `127.0.0.1`; containers use service DNS names internally and must not use `localhost` to reach another container.

For first-time production server bootstrap, follow [Podman server setup](./PODMAN_SETUP.md).

## Zero course timetable targets

Migration 090 permits nonnegative actual weekly targets in the opening change journal.
The canonical opening snapshot owns the effective target; no retired catalog column is restored.
Courses set to zero remain opened with their curriculum, credits, groups, teachers and assessment;
only standalone timetable demand becomes zero. Activities retain a positive target.
Publish the opening revision and explicitly update a timetable draft's opening source before the
new target applies there. Existing placed lessons and published history are preserved; remove
zero-target lessons from the draft before publishing it.

The migration expands constraints without rewriting data. Older binaries remain compatible until
zero targets are stored in opening revisions. After that point, use a roll-forward fix: older
validation rejects those revisions. Do not roll back by deleting opening or timetable history.

## Automatic room roster tracking

Migration [100](../backend-school/migrations/100_learning_group_roster_tracking.sql) adds opt-in group roster policy. Existing groups remain manual. An authorized group manager selects room tracking and its effective date using the group's existing source-room coverage. Published group memberships then reconcile in the same transaction as room add, transfer, removal, admission enrollment, account deactivation, promotion execution/correction or term activation. Capacity and date conflicts roll back the originating write. Draft rosters reconcile when prepared and published. Planned placements in a planning or ready year produce dated upcoming memberships; activation reconciles them again. Reads never synchronize memberships.

Tracking is operational roster policy and does not change immutable opening or timetable snapshots. Existing membership identities, results and scores are retained. Cancelled-before-start (`removed`) episodes retain their history but occupy no teaching dates. Switching to manual preserves the current roster. Group managers must explicitly publish rosters before automatic room mutations apply to them.

Deploy the backend, generated API contracts and frontend through the normal maintenance and all-tenant migration gates. An older binary can read the expanded schema, but must not be restored once automatic tracking is enabled: it cannot maintain that invariant. Recover with a compatible forward fix; never delete membership history or alter migration checksums to roll back.

## Required Environment and Secrets

All secrets and environment-specific URLs come from the runtime environment or deployment secret store. Example files are templates only.

Core backend-school secrets:

- `SESSION_HMAC_KEY`
- `INTERNAL_API_SECRET`
- `ENCRYPTION_KEY`
- `BLIND_INDEX_KEY`
- `DEPLOY_KEY`

Backend-school also requires `BASE_DOMAIN` and `TRUSTED_PROXY_CIDRS`; `SCHOOL_ALLOWED_DEV_ORIGINS` must be empty in production. `SCHOOL_ROLLBACK_JWT_SECRET` is a separate rollback-only secret and must differ from both `SESSION_HMAC_KEY` and the backend-admin `JWT_SECRET`. Backend-admin retains ownership of `JWT_SECRET`, its admin `DATABASE_URL`, and the provider credentials required by the operations it performs. Tenant provisioning uses the configured Neon values; `NEON_BRANCH_ID` is the branch identifier beginning with `br-`, not the display name such as `main`. The backend resolves an existing legacy branch-name value for compatibility, while new installer input requires the identifier. Deployment/DNS operations use the configured GitHub and Cloudflare values.

Internal service calls identify the caller with `X-Internal-Caller`. A caller-specific `INTERNAL_API_SECRET_<CALLER>` may override the shared `INTERNAL_API_SECRET`; the shared value is the controlled fallback during rotation. Keep both sides synchronized while rotating and remove the old value only after all callers are confirmed.

Never commit, print, or paste production secrets into logs, issues, screenshots, test fixtures, or generated artifacts.

## Health and Readiness

Both backends expose:

- `/health` for process liveness without dependency checks;
- `/ready` for dependency readiness and deployment gating.

Recurring Compose healthchecks use `/health` so process monitoring does not wake Neon or probe external dependencies. Backend deployment workflows and smoke tests use `/ready`; backend-school readiness verifies its backend-admin control-plane connection without waking every tenant database. External uptime monitors must use `/health`, because polling `/ready` would keep the admin Neon compute active. A dependency failure must fail the deployment readiness gate, while a live process remains diagnosable through `/health`.

## Deployment Workflows

There are four public workflow entry points:

| Owner | Trigger and responsibility |
| --- | --- |
| [pipeline.yml](../.github/workflows/pipeline.yml) | PR checks; main or manual preparation and production release |
| [merge.yml](../.github/workflows/merge.yml) | Serialize trusted team PR integration after `Pipeline gate` |
| [operations.yml](../.github/workflows/operations.yml) | Explicit smoke, browser E2E, diagnostics, benchmarks, Neon compatibility and tenant provisioning |
| [maintenance.yml](../.github/workflows/maintenance.yml) | Bounded GHCR retention; dry-run unless explicitly enabled |

The callable [verify.yml](../.github/workflows/verify.yml), [prepare.yml](../.github/workflows/prepare.yml)
and [release.yml](../.github/workflows/release.yml) implement the pipeline. The [deploy-school-release.yml](../.github/workflows/deploy-school-release.yml) and other `deploy-*` files
are callable component implementations, with no separate push or manual writers.

The flow is `plan → selected verification + artifact preparation → Pipeline gate → production lock
→ recheck main/baselines → universal maintenance → selected deployment → readiness/acceptance
→ publish accepted baselines → open maintenance`. PRs verify without production credentials or
release builds. Main builds selected immutable images and frontend bundles in parallel with checks.
All production mutations, including Admin and tenant provisioning, share `schoolorbit-production`
with cancellation disabled. Cloudflare Workers Builds must remain disconnected from the managed
Admin/School Workers: repository-linked automatic deployment is a competing writer and bypasses
the production lock and maintenance. Pipeline stages and promotes Worker versions; do not enable
an independent Git-triggered deployment in the Cloudflare dashboard. An obsolete candidate is rejected before maintenance. GitHub can
replace a queued run; component baselines therefore include every outstanding runtime change,
not only the immediately preceding commit. A newer push during an active release waits for that
release to finish.

The planner distinguishes verification, build and deployment paths. Frontend-only changes do not
compile a backend; body-only backend edits do not rebuild a frontend. Generated contract changes
include their School backend/frontend consumers. Test-only and documentation changes do not
normally deploy. Manual `scope` can add one component or `full`, and cannot omit queued changes.
The accepted schema-2 `pipeline-state` artifact records each component SHA and input hash, immutable
OCI digest or frontend bundle digest/run, and exact Worker versions. The reader checks repository,
main ref, workflow, accepted attempt and successful acceptance job. A cache hit alone never proves
verification or acceptance. Missing/expired/divergent state requires reconciliation; the first
transition from the previous workflows deliberately reconciles all four components.

`RUNTIME_DEPLOY_ENABLED` and `FRONTEND_DEPLOY_ENABLED` control automatic releases. Manual installer
releases remain possible while these are disabled. The installer dispatches one full Pipeline with
an exact deployment ID and selected target origin, and enables automatic releases only after its
public handoff verification. Direct-origin checks use the intended API hostname, loopback resolution
and the pinned Cloudflare Origin CA; the still-public DNS name is not evidence of a replacement VPS.

Every production deployment enters maintenance for both School and Admin, including frontend-only
updates and provisioning. `/deployment-status` remains available with no-store typed status;
public API requests return CORS-safe `503`, with `204` preflights. Frontends show maintenance and
poll every 10 seconds while visible, then reload when ready. Per-release mode-0600 probe tokens permit readiness and authenticated smoke
through the proxy while users remain blocked; tokens bypass only maintenance, not authorization.
Backend-authenticated Admin `/internal/` remains available for control-plane calls. A fresh origin
starts the maintenance proxy before its first backend exists, without unresolved upstream names.

Selected backend services use immutable digests, canonical rootless Podman topology, readiness,
migration completion and cutover audits. Admin starts before School on a full release. School
frontends consume one prepared bundle, synchronize menus through authenticated VPS loopback,
promote exactly their staged Worker version and apply routes. Asset propagation and real browser
mount checks precede acceptance. Admin uploads code and secrets together in one inactive version,
promotes the exact ID from structured Wrangler output and verifies it. Node 24 and the application
lockfile own Wrangler versions. Temporary secrets are mode `0600` and always removed.

The final acceptance owner verifies both proxy readiness paths and authenticated School academic
smoke, advances only selected image aliases, persists component baselines, and opens maintenance
last. Any earlier failure leaves maintenance active. A normal-proxy readiness failure restores
maintenance. Retry the same reviewed run or dispatch the current main; retained Worker recovery
manifests bind candidates to the exact trusted pipeline SHA and preserve the rollback boundary.
Successful replay uses accepted component baselines rather than a parallel legacy replay owner.
Provisioning reuses the trusted accepted School artifact and its public configuration, with an
exact request UUID; it has the same maintenance and acceptance discipline.

Team development uses one branch/worktree per independent developer and a PR for every main
change. Write collaborators automatically squash merge when the latest-main candidate passes;
no human approval is mandatory. Fork/external PRs cannot auto-merge. The trusted controller
updates and explicitly rechecks a stale candidate, merges one at a time, then dispatches the main
pipeline because `GITHUB_TOKEN` merges do not emit push workflows. Native protection for `main`
requires a PR, the GitHub Actions `Pipeline gate`, and an up-to-date branch, including for admins.
It requires zero human approvals, linear history, and forbids force pushes and branch deletion.
Equivalent PR verification may be reused only with exact tree/base, latest successful attempt,
suite receipts and matching runner/toolchain/profile evidence; otherwise main executes its checks.
Backend owners prime the main-owned compiled snapshot after proven PR verification; the PR's
fresh fixtures are reused once rather than repeated. Compiler-snapshot receipts are distinct from
test verification, and deployed readiness, migrations and authenticated smoke always execute.

### Build, deployment timing, and image retention

Backend image builds keep separate BuildKit cache scopes for admin and school. Rust, cargo-chef, and
sccache are pinned in their Dockerfiles. GitHub's cache runtime reaches the final Cargo build as
BuildKit secrets; a missing or unavailable compiler cache falls back to an ordinary Cargo build.
Compiler-cache builds explicitly select GitHub cache API v2 inside Docker and use
`SCCACHE_GHA_VERSION` to separate admin and school entries. Passing the cache URL and token alone
does not select v2; a build that reports write errors has not populated a usable cache.
Component hashes exclude application READMEs and top-level integration tests already excluded
from image contexts. Public frontend configuration and the preparation recipe participate in keys.
Prepared OCI images are tagged by input hash and release SHA; runtime replacement uses their exact
digest. Prepared frontend bundles are checked by content digest and consumed without rebuilding.
Compiler evidence artifacts include Cargo HTML timings and sccache statistics when a build actually
runs. Compare compilation, restore, push and deployment durations separately; an exact cache marker
alone is not evidence of a faster release. A failed release does not advance accepted baselines.

A source-only edit invalidates the final source/build layer, so the changed application crate and
final executable must be compiled and linked again. It does not discard the cargo-chef dependency
layer when `Cargo.toml` and `Cargo.lock` are unchanged. sccache may reuse eligible compiler outputs,
but it cannot avoid every changed-crate compile or the final link. If two ordinary warm builds show
no useful hits or increase total duration, remove the sccache integration while retaining the pinned
toolchain, Cargo timing artifact, and BuildKit cache.

School release builds use GitHub compiler caching. The release workflow passes
`SCHOOL_COMPILER_CACHE=gha` to both the image and timing export so exporting
metrics reuses the completed build. Credentials enter only through BuildKit secrets and never the
runtime image. A missing credential or cache I/O failure falls back to compilation. Standalone builds
default to `off` without runner credentials. To disable production compiler caching, change both
workflow build arguments to `SCHOOL_COMPILER_CACHE=off`; update the cache summary alongside them. Local Docker
experiments may explicitly select `local`, which is not the cross-runner GitHub cache.

After the workspace split, a source-changing GitHub comparison measured 24 eligible library hits
with zero write errors in each of two warm builds. Cargo duration fell from 6m25s without compiler
caching to 4m39s and 4m35s; the complete measured build, including BuildKit cache export, also fell.
These are build measurements, not production request latency or an entire deployment duration.
The application binary still compiles and links. Observe two ordinary warm releases after changes
to the workspace graph, toolchain, or cache integration; disable this cache if useful hits stop
reducing total duration. A new branch or cache namespace must first populate its own entries.

Exam scheduling models and persistence now compile in the existing assessment crate, while
the application retains HTTP/router/OpenAPI and lifecycle adapters. A same-runner Docker
comparison with a two-CPU quota and unchanged release optimization measured two application-source
rebuilds at 308 seconds before the move and 261/260 seconds afterward. Changing an exam service
after the move rebuilt assessment, its dependent academic crates, and the application in 297 seconds;
it does not eliminate the final application build. OpenAPI documents were identical. These are
warm Cargo measurements, not complete build/push/deployment timings. The first changed workspace
graph may require dependency-cache priming before those savings apply.

Academic and certificate handlers now compile in `school-academic-http` and
`school-certificates-http`. Menu models, templates, route synchronization and feature
toggles compile in `school-navigation`; notification persistence, tenant/user events,
publishing and Web Push compile in `school-notifications`. `school-auth-http` provides
their shared HTTP authentication context. The application owns routing, middleware,
startup and OpenAPI composition, and supplies narrow realtime, result-lock, lifecycle
and file-deletion adapters through `FromRef`. All contexts retain the same process-owned
identity and permission caches. New crates must never depend on the application or
accept its whole `AppState`.

Use the manual [HTTP Boundary Benchmark](../.github/workflows/backend-http-boundary-experiment.yml)
to compare the pre-extraction commit and a candidate on one Docker runner. Its two samples
per source owner separate first-graph priming from warm compilation and record rebuilt
packages, binary size, persistent-target reuse and exact OpenAPI equality. This comparison
disables sccache to isolate Cargo invalidation; use production build statistics to assess
cross-runner compiler-cache hits. A changed workspace graph may make its first image build
slower while dependency caches are populated. Do not equate a warm Cargo result with
the entire build, push, deployment, or request latency.

The [paired extraction experiment](https://github.com/akephisit/schoolorbit-new/actions/runs/37749256658)
used baseline `75b46a68`, candidate `27c5aba8`, the same Docker container and two
source-changing samples per case. Mean warm Cargo time changed from 265.4 to 236.5
seconds for application edits, 282.1 to 264.8 for Academic HTTP, 268.8 to 246.6 for
Certificates HTTP, 269.5 to 232.9 for Navigation, and 258.5 to 235.6 for Notifications.
Application edits rebuilt only the application; each extracted-owner edit rebuilt
that owner and the application, with no other domain rebuild. All twenty OpenAPI
exports were byte-identical to baseline. Unchanged target reuse took 0.40 seconds.
The candidate's first graph prime took 7m30s and is excluded from these means;
the binary grew from 109.2 to 111.2 MiB (about 1.8%). These two-sample compile savings
do not establish a percentage reduction in complete deployment time.

The pinned Rust toolchain already passes `-fuse-ld=lld` on this Linux target. Selecting lld again
does not remove compiler work. The manual build benchmark records the actual driver flag and
compares default/GNU BFD/lld using the same object files. Keep production optimization unchanged
unless both repeatable compile improvements and representative runtime performance support a
new profile; profile/exporter experiments alone do not establish production request latency.

The school Dockerfile uses `cargo rustc` with `-C lto=off` only for the final application
crate to reduce source-changing release build work. Dependencies retain their existing
release settings and cargo-chef cache; release optimization level is unchanged. This is
not a project-wide LTO override. Keep the existing runtime acceptance gates: faster local
compilation does not establish production latency or GitHub build/push duration. To roll
back this setting, restore the final `cargo build --release --bin backend-school --timings`
command; no runtime configuration or database change is needed.

Each backend deploy emits bounded `deployment_timing phase=<name> seconds=<integer>` records.
School also times `r2_reconciliation`: bucket access is still verified on each deployment,
but the private-bucket CORS policy is read once and compared in full, ignoring array order.
Only drift or an explicit missing-CORS response permits a write; a write requires a fresh
read-back match before deployment continues. Other read errors fail closed. The
`r2_cors_action` log distinguishes unchanged and updated policies without exposing credentials.
Admin reports image pull, backend readiness, and origin verification. School additionally reports
scanner readiness, tenant migration/status, authenticated smoke, and proxy cutover when those phases
run. These records contain no environment values or credentials.

After the existing release-acceptance gate succeeds, the deploy removes only old, exact 40-character
SHA tags from its own SchoolOrbit repository and keeps the newest three. Image IDs used by any
container or addressed by `latest` or `rollback` are always protected, so a protected older SHA can
temporarily remain in addition to the newest three. Cleanup never uses Podman system, image,
container, or volume prune and never removes `schoolorbit-clamav-signatures`.

Run **Runtime Diagnostics** manually before and after the first rollout. It reports the filesystem
capacity for Podman's graph root, `podman system df` accounting, per-backend SHA-tag counts,
container state/network aliases, Nginx validation, bounded database activity, and endpoint status.
It does not print container environments or application logs.

The school deploy pulls the pinned ClamAV image and reuses `schoolorbit-clamd` only when its image,
3 GiB memory, CPU/PID limits, restart/security settings, lack of published ports, named signature
volume, two networks, running state, and health all match the canonical Compose definition. It emits
`clamd_action=reused` when exact; otherwise it recreates only that container, emits a bounded reason,
preserves the signature volume, and waits through the same health gate.
Scanner creation also passes `--pids-limit=256` directly to Podman because podman-compose before
1.4 ignores `pids_limit`. This compatibility flag applies only to clamd, with `--no-deps`;
deployment verifies the resulting PID limit before proceeding. Keep this value aligned with Compose.
Image-exposed ports with null or empty binding lists do not publish host ports; the matcher accepts
those Podman inspect entries but rejects every actual host binding, including loopback bindings.

GHCR retention runs weekly and can be dispatched manually. It preserves `latest`, the 30 newest
SHA-tagged releases for each backend, and every untagged or unrecognized version such as an
attestation. Manual dispatch defaults to dry-run. The weekly run also remains dry-run unless the
repository variable `GHCR_RETENTION_ENABLED` is exactly `true`. Before enabling it, run a manual
dry-run and review both package candidate lists. The repository must have the package's Actions
admin access; a `403` is an access/configuration failure and must not be bypassed with another token
in source. Execution re-reads all selected candidates before mutation and again immediately before
each delete, processes oldest first, and deletes at most 100 versions per package per run.

For the first post-merge observation, use the next ordinary admin and school deployments rather
than triggering benchmark-only production changes:

1. Dispatch Runtime Diagnostics and record graph-root capacity, image accounting, and SHA counts.
2. Observe both build records, Cargo timing artifacts, and sccache hit/miss statistics. Repeat on the
   following ordinary source-changing deployment to obtain two warm samples.
3. Confirm each accepted deploy reaches all existing readiness, migration, smoke, and origin gates;
   check the phase timing records and `clamd_action` result.
4. Dispatch Runtime Diagnostics again. Confirm the active and rollback images resolve and each
   backend has the newest three SHA tags, allowing only additional protected image IDs.
5. Run GHCR Retention manually in dry-run mode. Enable `GHCR_RETENTION_ENABLED=true` only after the
   candidate order and protected versions are verified.

If cleanup fails after acceptance, keep the accepted service running and diagnose the exact protected
reference or Podman error; do not substitute a broad prune. If a cache path regresses, revert only
that cache integration. If ClamAV is recreated unexpectedly, use its bounded drift reason and the
canonical Compose definition to repair forward; never delete its signature volume.

## Reverse Proxy and Realtime

The current proxy sources are
[`nginx-configs/school-api.conf.template`](../nginx-configs/school-api.conf.template)
for the school API and
[`nginx-configs/admin-api.conf.template`](../nginx-configs/admin-api.conf.template)
for the admin API. The backend deployment workflows render them with the validated base domain,
install the result, validate Nginx, and then reload it.

Preserve:

- WebSocket upgrade headers and long-lived connection timeouts;
- direct, uncached realtime paths;
- the expected tenant/frontend CORS origins and credentials;
- forwarded origin/host information required for tenant resolution;
- access-log redaction so tokens, cookies, raw query strings, and PII are not recorded.

Validate WebSocket heartbeat, reconnect, and authenticated server-owned identity through the same proxy path clients use.

## School Session Runtime and Cutover

`SESSION_HMAC_KEY` is the stable backend-school owner for opaque browser-session hashes and domain-separated CSRF HMACs. Generate a unique random value of at least 32 characters in the deployment secret store, never print it, and keep it unchanged across ordinary deployments. Replacing it invalidates every current school session. It must never equal the admin JWT or rollback key.

`BASE_DOMAIN` owns the production tenant-domain boundary. `TRUSTED_PROXY_CIDRS` must contain only the networks of proxies that are allowed to supply forwarded client addresses; broad or unverified networks let clients spoof rate-limit identity. `SCHOOL_ALLOWED_DEV_ORIGINS` is only for explicit local origins such as `http://localhost:5173` and `http://127.0.0.1:5173`; leave it empty in production. Nginx must allow credentials and expose `X-CSRF-Token` while preserving exact tenant-origin validation.

Session policy is fixed in backend-school:

- normal sessions: two-hour idle and twelve-hour absolute lifetime;
- remembered sessions: seven-day idle and thirty-day (30-day) absolute lifetime;
- credential rotation: every 15 minutes with a 60-second previous-token grace window;
- last-seen/idle touch interval: five minutes;
- revoked or expired session retention: 30 days.

Replacement cookies never outlive the remaining absolute lifetime. SSE and WebSocket authentication use touch-only maintenance; the next ordinary request performs any due credential rotation. Login, creation, revocation, rotation failure, CSRF/origin rejection, and realtime disconnect logs use structured event/reason fields. Never log passwords, raw session credentials, cookies, CSRF values, request bodies, or database URLs.

Run exactly one backend-school process: [the session cache](../backend-school/crates/school-auth/src/session_cache.rs)
and revocation/permission events are process-local. API authentication reuses a successful database
check for up to 60 seconds, bounded by session expiry and due maintenance. Realtime connections
share database validation for up to five minutes per tenant/session/user; their 30-second heartbeat
continues checking cached expiry without extending idle lifetime. Normal authenticated API reads
also seed realtime validation. Logout, password change, and application-driven identity/permission
changes invalidate the relevant cache synchronously; no Redis or additional database is needed.

Direct SQL changes bypass those events and can remain unseen for up to the applicable cache window
(plus the realtime heartbeat interval). Use application operations for immediate revocation. If an
emergency operation changes identity/session state directly, restart the single backend process to
clear caches and terminate old realtime connections. Restarts preserve database-backed sessions and
reload them on demand. Do not scale to multiple workers/replicas or overlap old and new processes
without first implementing shared invalidation. A fresh cache can serve during a database outage;
once its validation window ends, authentication fails closed instead of extending stale entries.

Browser tabs hidden for 60 seconds pause notifications SSE and timetable WebSocket. On returning,
the client refreshes authentication and reconciles authoritative state across the disconnected gap.
Web Push subscriptions remain independent of the SSE connection. The application change reduces
session queries; actual Neon compute savings also depend on other traffic and background jobs.

For the one-time JWT-to-session cutover:

1. Enter backend-school maintenance and provision `SESSION_HMAC_KEY` plus a newly generated `SCHOOL_ROLLBACK_JWT_SECRET` without printing either.
2. Run the centralized all-tenant migration gate through migration `034_auth_sessions.sql`; stop on any tenant failure.
3. Deploy the session-enabled backend-school while maintenance remains active. Keep backend-admin and its `JWT_SECRET` unchanged.
4. Deploy frontend-school. Validate Nginx CORS/preflight, then run login, `/api/auth/me`, a protected read, a CSRF mutation, session list/revoke, logout-all, SSE, WebSocket, the repository smoke script, and two-context Playwright.
5. Leave maintenance only after every check passes. Every school user then performs one clean login.

A rollback keeps migration `034_auth_sessions.sql` applied. Deploy the prior backend-school image with `SCHOOL_ROLLBACK_JWT_SECRET` mapped to that process's `JWT_SECRET`, roll back frontend-school, and require another clean login. Never restore the old shared school JWT key, never modify `_sqlx_migrations`, and never change backend-admin's `JWT_SECRET`. Remove the rollback mapping only after the rollback window closes.

## Replacement VPS Migration and DNS Rollback

Use [`scripts/schoolorbit-installer`](../scripts/schoolorbit-installer) from an administrator
machine running Bash 4.4 or newer. The target may be Debian or Ubuntu. Supply credentials only
through environment variables, hidden prompts, or `--secrets-stdin`; the installer never accepts
secret values as command-line arguments. Run the read-only provider and target preflight first:

```bash
./scripts/schoolorbit-installer migrate-vps \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run
```

Remove `--dry-run` for the real migration. The installer creates a mode-`0600` checkpoint under
`~/.local/state/schoolorbit-installer/`, prints its run ID, and records only non-secret state.
`SCHOOLORBIT_SERVER_PASSWORD` is also required: use a unique value of at least 10 characters for
the `schoolorbit` Linux/Cockpit account. It is installer input, not an application runtime value,
and must remain in `.env.local`, JSON stdin, a hidden prompt, or the operator's secret manager.
After correcting a failure before or during migration, resume the same run without repeating a
verified phase:

```bash
./scripts/schoolorbit-installer migrate-vps --resume RUN_ID
```

The migration bootstraps the target, installs runtime configuration and Origin CA material,
dispatches backend-admin, frontend-admin, and one `full` school release, and pins school tenant
discovery and verification to the selected origin until DNS is changed. Menu synchronization uses
the backend-school VPS loopback while public DNS still points at the prior origin. The installer
verifies both APIs directly with `curl --resolve` and pinned Origin CA trust, then
prints the DNS diff and
requires the exact phrase `CUTOVER <target-ip>` before one two-record Cloudflare batch. Public
verification covers API identity, both frontends, authenticated SSE, and the File Platform. Only
after those checks pass does the migration configure the Cockpit management Tunnel, verify its
connector came from the selected target, publish `server.schoolorbit.app`, and verify the public
Cockpit endpoint before the deployment gates are enabled. A failed post-cutover verification reports recovery commands;
it never performs an automatic rollback.

Rollback restores the complete checkpointed record content, TTL, and proxy state. Confirm that
the current records still represent this run, then execute:

```bash
./scripts/schoolorbit-installer rollback-dns --run-id RUN_ID
```

The command prints the reverse diff and requires the exact phrase `ROLLBACK <original-ip>` before
applying one reverse API-DNS batch. If that migration published management DNS, the same rollback
also restores or removes its management CNAME after revalidating both current states. The
replacement VPS, GitHub configuration, and both Cloudflare Tunnels are retained for diagnosis or a
later retry. Keep the old VPS available until the rollback window has been closed explicitly.

The TLS checkpoint stores the Cloudflare Origin CA certificate ID and `certificate_expiry`, but
never the private key. Monitor that expiry independently and schedule replacement in advance;
Cloudflare does not send Origin CA expiry notifications. Keep Cloudflare SSL/TLS mode at
`Full (strict)` for installer-managed API origins.

## School homepage search indexing

School homepage indexing follows the configured `PUBLIC_BACKEND_URL` hostname, which must use the production topology `school-api.<base-domain>`. The shared policy in `frontend-school/src/lib/school-public/seo.ts` excludes sandbox, local, preview and reserved hosts. `/robots.txt` allows page resources and declares the current production school's `/sitemap.xml`; the sitemap contains only that school's HTTPS homepage. Other HTML responses use `X-Robots-Tag: noindex`. Robots rules and noindex are indexing controls, not replacements for authentication or resource authorization.

After a frontend release, verify initial HTML without JavaScript, tenant-specific metadata and canonical URL, sitemap XML, and sandbox/login noindex as described in [Testing](./TESTING.md). Existing school settings remain the source of truth; do not enter a second name or logo for SEO.

The domain owner completes Google Search Console setup in their own Google account: add the Domain property for the production base domain, copy Google's TXT verification value into that domain's DNS, verify ownership, then submit each real school's `/sitemap.xml` and request indexing of its homepage through URL Inspection. Do not submit sandbox. Google tokens and account credentials must not be stored in repository files. A sitemap or indexing request does not guarantee immediate inclusion or ranking; monitor indexing in Search Console. See [Google's ownership guide](https://support.google.com/webmasters/answer/9008080) and [recrawl guide](https://developers.google.com/search/docs/crawling-indexing/ask-google-to-recrawl).

## Cockpit Management over Cloudflare Tunnel

The supported management path is:

```text
browser -> Cloudflare edge -> Cloudflare Tunnel -> 127.0.0.1:9090 -> Cockpit
```

Cockpit and cloudflared run as host systemd services; they are not Compose services and do not use
the application Nginx container. The host firewall must not allow inbound `9090/tcp`. Cockpit keeps
`root` in `/etc/cockpit/disallowed-users`; log in at `https://server.schoolorbit.app` as
`schoolorbit` so Cockpit Podman sees the same rootless containers as production. A root Cockpit
session would use a different Podman namespace and is intentionally unsupported. Cockpit starts a
new browser profile in Limited access mode by design. Select **Administrative access** and enter the
same `schoolorbit` password when host administration is needed. The installer adds `schoolorbit` to
the standard `sudo` group but does not create a `NOPASSWD` rule. After a group-membership repair,
sign out of Cockpit and sign in again so the new login session receives the group.

Cockpit Podman reaches that namespace through `podman.socket` in the linger-enabled `schoolorbit`
user manager. Its expected API path is `/run/user/<schoolorbit-uid>/podman/podman.sock`; the root
socket under `/run/podman` belongs to a separate rootful namespace and must not be used as a
replacement. Enabling this socket does not stop, recreate, or restart existing containers.

This deployment intentionally has no Cloudflare Access, OTP, or account-member gate. It is a
public login and the login page is therefore publicly reachable. Use a unique strong password, retain SSH key access for recovery,
monitor authentication activity, and treat Cockpit/cloudflared security updates as production
patches. Cloudflare terminates public TLS; Cockpit accepts unencrypted HTTP only on its loopback
listener. The Tunnel token is stored at `/etc/cloudflared/schoolorbit-cockpit.token` as root mode
`0600` and is consumed with `--token-file`; it must never appear in a command argument or checkpoint.

For an already migrated VPS, first ensure the operator environment contains
`SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN` with the account/zone permissions required to manage
Cloudflare Tunnels and DNS plus `SCHOOLORBIT_SERVER_PASSWORD`. Run a read-only check:

```bash
./scripts/schoolorbit-installer configure-cockpit \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run
```

Remove `--dry-run` to apply. A distinct Tunnel named from the installer run is created or safely
adopted on resume; the management CNAME is published only after Cockpit, the loopback listener, the
fresh SSH verification, and the connector origin IP pass. Resume the same operation with:

```bash
./scripts/schoolorbit-installer configure-cockpit --resume RUN_ID
```

If management DNS was published but validation failed, use the reported run ID:

```bash
./scripts/schoolorbit-installer rollback-cockpit --run-id RUN_ID
```

The command revalidates drift, prints the reverse management diff, and requires the exact phrase
`ROLLBACK COCKPIT server.schoolorbit.app`. It restores an existing snapshotted CNAME or deletes only
the record created by that run. It never deletes either Tunnel or changes application API DNS.

After setup, verify on the target through a privileged SSH session:

```bash
systemctl is-active cockpit.socket schoolorbit-cloudflared.service
ss -ltnH '( sport = :9090 )'
curl -fsS http://127.0.0.1:9090/ping
stat -c '%a %U:%G' /etc/cloudflared/schoolorbit-cockpit.token

server_uid=$(id -u schoolorbit)
server_home=$(getent passwd schoolorbit | awk -F: 'NR == 1 { print $6 }')
runtime_directory="/run/user/$server_uid"
podman_socket="$runtime_directory/podman/podman.sock"
runuser -u schoolorbit -- env \
  HOME="$server_home" \
  XDG_RUNTIME_DIR="$runtime_directory" \
  DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
  systemctl --user is-active podman.socket
test -S "$podman_socket"
runuser -u schoolorbit -- env \
  HOME="$server_home" \
  XDG_RUNTIME_DIR="$runtime_directory" \
  podman --remote --url "unix://$podman_socket" info >/dev/null
```

The only listener must be `127.0.0.1:9090`, the ping service must be `cockpit`, and the token file
must be `600 root:root`. The per-user Podman socket and remote API check must both succeed. Confirm
the Cloudflare connector origin matches the target IP, the CNAME is
proxied to the checkpointed Tunnel UUID, direct access to `<target-ip>:9090` fails, and the
`schoolorbit` Cockpit Podman page lists `schoolorbit-backend-admin`, `schoolorbit-backend-school`,
`schoolorbit-clamd`, and `schoolorbit-nginx`. Retain the prior Tunnel/VPS through the rollback window.

## Tenant Migrations

Active tenant migrations begin at `backend-school/migrations/001_baseline.sql`. Do not modify an applied migration or hide a checksum mismatch.

New tenant provisioning calls the centralized runner in [`backend-school/crates/school-migrations/src/lib.rs`](../backend-school/crates/school-migrations/src/lib.rs), applies every pending active migration, and synchronizes the permission contract before creating the tenant administrator.

The coordinated school release keeps the school API in maintenance mode while it calls
`/internal/migrate-all`. It then verifies `/internal/migration-status` reports every tenant at the
repository's latest migration with no pending, failed, or outdated tenant. The normal proxy opens
only after the authenticated read-only smoke and, for a full release, every staged frontend
promotion succeeds.

The one-time legacy rebaseline is complete and its operational scripts are retired. If a tenant with legacy `_sqlx_migrations` history is discovered, stop the rollout and prepare a new reviewed recovery plan. Never point the current release at that database, copy migration history, or edit SQLx checksum records.

### Curriculum editions, educational levels and recovery boundary

Migrations 091/092 replace calendar bounds with a Buddhist revision year and reconcile the verified five-plan secondary-school migration shape. Migration [093](../backend-school/migrations/093_curriculum_edition_levels.sql) establishes one school-owned edition above its educational levels and study programs. It groups only matching known revision year, edition name and publication status; unknown years stay isolated and versionless roots become drafts requiring an explicit year before publication. Existing version UUIDs become level UUIDs. Program, requirement, room, student-year and admission references remain stable; the locked transaction verifies exact requirement content and records `curriculum.edition_hierarchy.reconciled` before committing. Ambiguous duplicate level membership refuses migration.

Migration [094](../backend-school/migrations/094_curriculum_school_permissions.sql) replaces curriculum owner scopes with school read/manage permissions. It preserves each existing curriculum action in role grants, organization-position grants and delegations, including delegation expiry and revocation. Other organization-scoped modules retain their ownership policies. Runtime code has no legacy curriculum root/version API or duplicate publication owner.

Create an edition on the curriculum overview, add educational levels with covered grades, then add programs and their grade/term requirements. Copying selects one published source program and creates independent draft requirements in the destination level, mapping semantic term slots and refusing uncovered grades or stale revisions. Publication validates every level and publishes the entire edition atomically. Room assignment explicitly chooses an edition then a grade-compatible published program; revision year does not limit calendar use, and publication never reassigns existing rooms.

Migration [095](../backend-school/migrations/095_curriculum_publications.sql) captures each currently published edition as immutable publication 1, preserving its complete graph and stable references. Unknown original publisher/date remain unknown. Existing educational-level/program tables become the sole editable draft graph; runtime consumers read the current immutable publication through published views. Opening an amendment creates or reuses one edition-owned draft token. Every mutation requires that exact token and resource row version; publishing validates all levels, seals the complete graph, advances the counter atomically and closes the token. Historical snapshots cannot be edited or appended after their creating transaction. New unpublished editions have no history until publication succeeds.

Migration [097](../backend-school/migrations/097_curriculum_draft_discard.sql) permits amendment cancellation only after the complete workspace matches the current publication. Staff with curriculum manage permission can preview and permanently discard an amendment using its exact token, edition row version and preview content hash. Discard restores all released levels, programs, term slots and requirements in one owner-locked transaction, deletes unpublished additions, closes the token and creates no publication or draft archive. Published history and actual teaching records remain intact. A changed preview or an external reference to a new draft resource refuses cancellation without partial deletion; reload the preview or resolve the reference through its owning workflow. This operation does not delete a never-published edition.

Migration [096](../backend-school/migrations/096_delivery_curriculum_provenance.sql) records publication sources for new curriculum-derived offerings and curriculum preparation runs. Legacy openings without provable publication provenance remain explicitly unknown. Actual offering requirements reference an immutable identity registry so replacing draft requirements cannot delete their provenance. Publication alone never changes existing openings, teaching groups, timetables or delivery snapshots. Staff must review and apply changes through Academic Delivery and manage the resulting timetable separately.

After 095 applies, school binaries that read or mutate the draft tables as published data are prohibited. Deploy 095/096, backend, contracts and frontend together under maintenance. Require the `curriculum.publication_baseline.reconciled` audit, exact snapshot/content preservation, unchanged actual references, native draft/publication tests and a protected-copy rehearsal for every active tenant. Keep maintenance active and repair forward on failure; a restore must account for writes since the recovery point.

Before release, rehearse the central runner on disposable copies of every active tenant and preserve fresh recovery evidence using the seven-day procedure under Personnel data cutover. Deploy migrations, backend, generated contracts and frontend together through a coordinated `full` release under maintenance. Require every active tenant at the exact latest migration version, passing preservation evidence, publication immutability, expected content/reference counts, readiness and authenticated smoke before opening traffic. After 093 replaces curriculum tables, all older school binaries are prohibited; keep maintenance active and repair forward if acceptance fails. Restoring a recovery point requires a reviewed procedure accounting for later writes.

### Academic Core Phase B cleanup and rollback boundary

Migration `045_academic_core_legacy_cleanup.sql` is the destructive Academic Core cleanup boundary.
It runs only through the centralized tenant migration runner while the school API is in maintenance.
The migration locks the affected schema, verifies the Phase A audit and current version-44
reconciliation marker, rechecks retained data and equivalent permission grants, and fails before any
drop when evidence is missing, stale, or inconsistent. It then removes the exact legacy manifest and
records the bounded `academic-core-v1-cleanup` audit. The one-time preflight command and mutable
Phase A reconciliation endpoint are retired and must not be restored.

For the Phase B deployment:

1. Confirm the protected pre-045 database snapshot exists, and do not delete it during deployment.
2. Dispatch the reviewed commit as a `full` school release. `/internal/migrate-all` applies every pending migration,
   including 045; do not invoke tenant migrations through another path.
3. Require `/internal/migration-status` to report every tenant at the repository's latest version
   with no pending, failed, or outdated tenant. Each tenant's `academicCoreCutover` must report
   migration version 45, `cleanupCompleted`, `passed: true`, and only passing bounded checks.
4. Require generated API and permission contracts, `/ready`, and the authenticated read-only smoke
   in the selected `academic_core_smoke_subdomain`. Credentials come only from `SMOKE_USERNAME` and
   `SMOKE_PASSWORD`; the workflow reaches backend-school through VPS loopback and exposes no public
   maintenance bypass.
5. Treat approval to dispatch the reviewed full release as the go/no-go decision. The workflow keeps
   maintenance active on any failure and opens the normal proxy only after every acceptance gate
   passes. Record the first accepted write as the snapshot rollback boundary.

Any migration, cleanup-audit, readiness, contract, or smoke failure keeps maintenance active. Before
the first accepted write, rollback means restoring the protected snapshot and the matching pre-045
release together. After the first write, do not deploy the old app against the new schema; keep
traffic closed and repair forward with a reviewed migration/application artifact. Never edit
`_sqlx_migrations`, an applied migration, cleanup audit, or tenant data to force a green result.

### Personnel data cutover

Historical migrations 081/082 standardized ranks/degrees, references and retired status. The current [083 expansion](../backend-school/migrations/083_staff_personnel_simplification_expand.sql) copies every position into the system-owned catalog with stable UUIDs, status, ordering and timestamps, and moves referenced education names into person-owned text. [084 cleanup](../backend-school/migrations/084_staff_personnel_simplification_cleanup.sql) freshly reconciles the locked source and target against the expansion audit before retiring the generic catalog and reference-ID columns. Custom/inactive current positions remain readable and filterable; new assignments require active selectable standard positions. The former reference CRUD API and management route are retired. Ranks/degrees remain codes; subject groups derive from current active organization memberships.

The one-time cutover completed on all active tenants at version 84, with preservation checks and source/target schema verified before retiring its personnel preflight endpoint, runtime audit readers and deploy helpers. Keep the applied SQL files and durable audit rows unchanged. Their preservation and refusal tests remain under `school-staff` through private test-only readers. Production reads and writes use the canonical owner.

Current releases call `/internal/migrate-all` through the centralized migration runner and verify `/internal/migration-status` against actual SQLx history for every active tenant. The release requires complete tenant coverage at the repository's latest version, no pending/failed/outdated tenants, and the remaining academic and gradebook domain audits. Maintenance stays enabled if verification fails. Readiness, authenticated smoke, menu synchronization and matched Worker promotion remain required before reopening the proxy.

For any future data replacement, retain a protected provider snapshot for at least seven days and rehearse the centralized migration runner on disposable copies of all affected active tenants before promotion. If provider quotas prohibit another protected snapshot or branch, preserve the existing protected snapshot, retain an unmodified no-compute pre-cutover branch for at least seven days, and supplement it with encrypted custom-format dumps of every affected tenant. Store archives and the encryption key outside the repository with restrictive permissions; verify complete decryption and `pg_restore --file=/dev/null` before promotion. Record the quota limitation and recovery branch ID privately; an unprotected branch is not a protected snapshot. Archive recovery requires the retained key and an explicitly reviewed restore procedure. Emit only bounded counts, versions and check codes; keep connection credentials outside arguments and logs.

Migrations 083/084 retain their own locked-source checks, fingerprints, exact preservation evidence and canonical schema checks before replacing FKs or removing retired owners. New tenant provisioning still applies the full immutable migration chain through the central runner. After a tenant applies 084, older binaries expecting the generic catalog or education UUIDs are prohibited. Keep maintenance active on failure and repair forward through a new reviewed migration/application release. Restore a protected snapshot only through a reviewed procedure accounting for later writes. Never edit applied migrations, SQLx history, reconciliation checks or personnel rows to force acceptance.

The initial matched frontend preserved verifiable owner-qualified v2 drafts through a bounded migration-only importer with their original 30-minute expiry. After both target frontends accepted the canonical release and that lifetime elapsed, the verified frontend cleanup retired the importer and v2 schema. The career release writes only owner-qualified v4 drafts. Both target frontends accepted the career release and the full original 30-minute lifetime elapsed before retirement of the v3 importer and schema. Obsolete ownerless and current-owner v2/v3 keys are now removed without reading or importing their contents. Invalid saves preserve the existing canonical draft, and reading never renews its expiry. Focused tests enforce scoped cleanup, canonical career preservation, refusal of retired fields, and absence of migration writes. No legacy API lookup or runtime schema fallback is allowed.

Rank milestone reads use the canonical current history and existing profile scopes. The versioned system policy is owned by [the staff calculator](../backend-school/crates/school-staff/src/rank_milestones.rs); a policy amendment requires a new reviewed version and calendar/state tests. The ordinary date is a planning milestone, and the conditional reduced date never confirms a reduction right or submission eligibility. No schema migration, persisted eligibility, review approval, notification scheduler or new permission is introduced. Deploy the backend before the matched frontend so the independent overview endpoint and history response are available; earlier frontends can ignore the additional history field. Verify the scoped endpoint and both tenant frontends after the matched release.

[Migration 085](../backend-school/migrations/085_staff_career_history.sql) establishes the canonical career-history owner and deferred current-projection guards. Import preserves existing position/rank values, UUID relationships and unrelated staff fields; effective dates, order dates/numbers and import actors remain unknown. Personnel type stays unspecified until explicitly recorded. Current changes and reasoned corrections run through `StaffCareerService` in one transaction with optimistic revision/current-reference checks and audit records. Historical append never advances the current pointer. Public profiles do not expose history or these dates.

After a tenant applies 085, binaries writing position/rank directly into `staff_info` are prohibited. Deploy migration, backend, generated API contracts and frontend as one coordinated full release under maintenance. Rehearse the central runner on every affected active-tenant copy, preserve a current recovery point using the seven-day procedure above, and require actual SQLx-version equality before opening the proxy. Keep maintenance active on failure and repair forward with reviewed artifacts. Never bypass the guards, modify an applied migration or SQLx history, or assume an old binary is a safe rollback. Recovery must account for writes after the retained point. This release records career facts; it does not calculate eligibility or a next submission date.

### Delivery and timetable version cutover

The coordinated school release applies migrations
[`086`](../backend-school/migrations/086_academic_delivery_versions_expand.sql),
[`087`](../backend-school/migrations/087_academic_delivery_versions_backfill.sql), and
[`088`](../backend-school/migrations/088_academic_delivery_timetable_cutover.sql) with the new backend,
generated contract and matching frontend while school-api remains in maintenance.
Rehearse the central runner on protected copies of every affected active tenant before consumer
cutover, retain the current recovery point, and use the disposable fixture recipe in `TESTING.md`.

086 only expands the schema. 087 locks and captures immutable opening graphs, maps exact source IDs,
and records fifteen identity/relationship fingerprints. Consecutive equal opening graphs share a
source; an A/B/A transition retains three dated opening versions. Pure placement drafts remain
independent; mixed revisions retain their pending opening commands and the separately pinned
placement draft. 088 requires fresh passing evidence before removing joint lifecycle columns,
migrates persisted placement coverage, and independently reconciles the canonical result.
Recorded direct inclusions without edit items become deterministic opening commands only when
the journal and draft agree on the published source, retain every original target, and provide
explicit added offering IDs and weekly targets. Early publication captures groups existing by
the recorded Bangkok effective boundary. Later imports require exact active placement IDs and
one dated assignment for every placed instructor, matching teacher ID, group ID and role at the
effective date across all placements. All historical instructors are reconciled against captured
episodes. Missing or overlapping episodes, changed targets without commands, missing journals,
and conflicting sources still stop reconciliation.
Missing, conflicting or ambiguous source evidence stops the migration without partial writes.
Repair evidence through a reviewed operation on protected data; never infer relationships from
names, discard a draft, weaken a guard, edit an applied migration, or change SQLx history.

`/internal/migration-status` reports `deliveryTimetableCutover` using the delivery-owned aggregate
audit reader. The full release keeps maintenance active until every tenant has the exact repository
SQLx version, `migrationVersion: 88`, `status: cutoverCompleted`, `passed: true`, and all thirty
checks from migrations 087/088 passing. It returns bounded codes/counts, without snapshot rows or
teacher/student data. Once 087 captures reconciliation, legacy writes invalidate that evidence;
keep traffic closed until 088 and coordinated backend/frontend acceptance complete. After 088,
older binaries using the joint opening/table lifecycle are prohibited. Recover by rolling forward
with a reviewed migration/application artifact; an old binary rollback is unsafe.

Acceptance verifies existing table/block/resource IDs, pinned historical opening facts, independent
opening publication, editing with the latest published source, review-marked retained placements,
autosave errors, standalone table publication, and deletion limited to an unreferenced draft.
Check read-only and scoped users through the deployed proxy and retain the feature branch and
recovery artifacts until migration, authenticated smoke, menu registration and acceptance pass.

### Opening draft dates and permanent deletion

[Migration 089](../backend-school/migrations/089_delivery_draft_publication_and_deletion.sql)
separates a draft's internal reference date from its nullable publication date. Published graphs,
identities and effective dates remain unchanged. The matched backend and frontend must deploy
under the coordinated school release, with the centralized all-tenant runner and exact schema-head
checks. Binaries expecting a non-null draft publication date are prohibited after 089. Rehearse on
protected copies of every affected active tenant and retain the current recovery point for seven
days; recover forward after cutover, accounting for subsequent writes.

Ordinary DELETE operations require existing resource-management scopes, current revisions and
confirmed child counts. Drafts and cancelled versions may be deleted together with exclusively owned,
unreferenced children. Published versions and historical references are protected. A foreign-key
conflict rolls back the whole operation; never detach references or disable immutability guards.

The application-owned [cancelled-version cleanup CLI](../backend-school/src/bin/cleanup_cancelled_academic_versions/main.rs)
is a separate, explicitly scoped operation after migration, authenticated smoke and browser acceptance.
It resolves one school through backend-admin, verifies its stable tenant identity and term/year,
requires an active staff actor with both school-wide management capabilities, and preflights every
cancelled opening/table version before deleting anything. Active drafts are outside this operation.
Set `ACADEMIC_CLEANUP_SUBDOMAIN`, `ACADEMIC_CLEANUP_TENANT_ID`,
`ACADEMIC_CLEANUP_ACADEMIC_YEAR_ID`, `ACADEMIC_CLEANUP_ACADEMIC_TERM_ID` and
`ACADEMIC_CLEANUP_ACTOR_ID` from the reviewed scope; supply `BACKEND_ADMIN_URL` and the existing
caller secret through private environment configuration. Do not print connection URLs, secrets or
staff records. Without `ACADEMIC_CLEANUP_PREVIEW_HASH` the CLI preflights and rolls back. Applying
requires the exact returned hash; changed revisions/counts or any protected reference stop the
entire batch. Audit entries and deletes commit together, and the report verifies that published
fingerprints remain unchanged. Verify actual cancelled-row disappearance, active draft preservation,
source selection and the deployed version lists afterward. Do not broaden a failed cleanup scope.

### Gradebook and results cutover

Migration `060_gradebook_results_and_learner_evaluations.sql` is the Release 2 boundary for Gradebook,
learner evaluations, locked results, and result corrections. The deployment gate audits durable
schema, seed, and generated-permission invariants only. Its bounded report may contain check codes,
counts, migration versions, and pass/fail state; it must not expose student identity, scores,
outcomes, credentials, or source rows. Editable school grading configuration is not a cutover
invariant.

For the Release 2 deployment:

1. Create and retain a protected snapshot before dispatching the release.
2. Use one reviewed commit for the migration, backend, contracts, frontend, and deployment gate.
   Dispatch the manual Neon compatibility workflow first; it must use a fresh disposable child
   branch and its direct non-pooled endpoint to run the migration-060 schema and status-audit tests.
3. Deploy through the centralized runner and apply every pending migration through the repository's
   latest version. Do not apply migration 060 or later files manually.
4. Require `/internal/migration-status` to report repository-version equality for every tenant and
   `gradebookResultsCutover` at migration version 60 with `cutoverCompleted`, `passed: true`, and only
   passing checks. Any missing, failed, pending, or unavailable audit keeps maintenance active.
5. Require generated API and permission contracts, route/menu synchronization, `/ready`, and the
   authenticated read-only smoke to pass. The smoke samples at most two canonical term contexts and
   includes Gradebook subjects, learner-evaluation subjects, and result readiness.
6. Treat the reviewed `full` release dispatch as the explicit go/no-go decision. The workflow opens
   traffic automatically after every gate passes. Record the first accepted write as the
   protected-snapshot rollback boundary.

Before the first accepted write, rollback restores the protected snapshot together with the matching
pre-cutover application. After that boundary, keep traffic closed and repair forward with a reviewed
migration/application artifact. Never alter tenant rows, an applied migration, cutover checks, or
`_sqlx_migrations` to force deployment success.

### School font library cutover

Migration `040_school_font_library.sql` is an intentional empty-state cutover. Before entering the all-tenant migration gate, verify every active tenant has zero legacy certificate font assets, zero `certificate_template_font` staging rows, and zero text elements whose `fontSource.type` is `asset`. The migration enforces the same prerequisite and stops with `legacy certificate template fonts must be empty before migration 040` if any old row remains. Do not silently convert, copy, or delete a non-empty tenant during deployment; stop and use a separately reviewed data-removal or migration procedure.

Deploy migration 040, the backend routes, generated API contract, and the matching frontend together while school-api remains in maintenance mode. If any tenant fails, keep maintenance active, retain the new image, correct the tenant state through an approved operation, and fix forward through the centralized migration gate. Once any tenant has applied migration 040, never deploy an older backend that expects template-owned font columns, edit the applied migration, or alter `_sqlx_migrations` checksums.

## Permission and Menu Synchronization

Permission definitions originate in `contracts/permissions.json` and are materialized into generated registries plus tenant DB data. Deploy the contract artifacts and any new sequential permission migration together.

After permission changes:

1. verify the generated permission contract;
2. apply the new tenant migration;
3. verify expected permission/grant rows;
4. invalidate affected permission caches and confirm `permission_changed` refresh behavior;
5. rebuild tenant frontends when route metadata changed.

Frontend route metadata is synchronized explicitly after each successful tenant frontend
deployment. Provide matching backend and workflow `DEPLOY_KEY` values plus `SUBDOMAIN`.
Synchronization is transactional: frontend route identity is marked `managed_by =
'frontend'`, cleanup is limited to stale frontend-owned rows, and school-owned names,
icons, placement, active state, ordering, and custom menu records remain unchanged. Treat a
failed synchronization step as a failed deployment and fix the scan or backend error before
rerunning it.

## Encryption and Key Rotation

National IDs use application-side AES-256-GCM through `backend-school/src/utils/field_encryption.rs`. Search uses keyed HMAC-SHA256 blind indexes in `*_national_id_hash` columns.

`ENCRYPTION_KEY` and `BLIND_INDEX_KEY` must remain stable after data is written. Rotation requires a dedicated, reviewed job that decrypts with the old key, re-encrypts with the new key, rebuilds blind indexes, verifies counts/samples safely, and provides rollback. Do not switch either key independently without migrating existing data.

Do not use legacy PostgreSQL `pgcrypto`, `ALTER ROLE`, or database session settings for application field encryption. Do not log plaintext values or keys during migration.

## File Storage

Backend-school owns a provider-neutral File Platform. Business modules and frontends store a logical file ID; they never store an R2 key, bucket, provider URL, or signed URL. The platform selects storage from the registered purpose:

- `R2_PUBLIC_BUCKET_NAME` contains only public purposes such as school branding. `R2_PUBLIC_URL` is the delivery base for this bucket.
- `R2_PRIVATE_BUCKET_NAME` contains profiles, achievements, admissions, question-bank images, documents, and `school_font` originals. It must have no public custom domain or `r2.dev` access.
- `R2_PRIVATE_BUCKET_NAME` allows browser delivery only through short-lived signed `GET`/`HEAD` requests from `https://*.schoolorbit.app`. The backend-school deployment applies and verifies this CORS policy without making the bucket public.
- The two bucket names must be present and different. `R2_BUCKET_NAME` and `CDN_URL` are not compatibility fallbacks.

Object keys are immutable and server-generated:

```text
tenants/{tenant_id}/{domain}/{purpose}/{file_id}/v{version}/original.{ext}
tenants/{tenant_id}/{domain}/{purpose}/{file_id}/v{version}/derivatives/{variant}.{ext}
```

The `domain` and `purpose` segments come only from the purpose registry. This looks like folders in R2, but the database remains the source of metadata and lifecycle state. A future document system should reference file IDs and add its authorization relationship; it must not invent another key layout.

Required runtime configuration:

- R2: `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_REGION`, `R2_PUBLIC_BUCKET_NAME`, `R2_PRIVATE_BUCKET_NAME`, and `R2_PUBLIC_URL`.
- Scanner: `CLAMD_ENDPOINT`, `CLAMD_CONNECT_TIMEOUT_MS`, `CLAMD_WRITE_TIMEOUT_MS`, `CLAMD_READ_TIMEOUT_MS`, `CLAMD_MAX_CHUNK_BYTES`, `CLAMD_MAX_RESPONSE_BYTES`, and `CLAMD_MAX_CONCURRENT_SCANS`.
- Lifecycle: `FILE_PRIVATE_GRANT_TTL_SECONDS`, `FILE_RECONCILE_LEASE_SECONDS`, `FILE_RECONCILE_BATCH_SIZE`, `FILE_RECONCILE_MAX_ATTEMPTS`, `FILE_RECONCILE_RETRY_BASE_SECONDS`, and `FILE_RECONCILE_RETRY_MAX_SECONDS`.

Configuration fails closed at startup for missing, placeholder, shared-bucket, or out-of-range values. `/health` remains process liveness. `/ready` requires backend-admin, both R2 buckets, and a clean clamd probe; deployment must gate traffic on `/ready`.

### Bucket and scanner rollout

1. Preserve the existing public bucket and set it as `R2_PUBLIC_BUCKET_NAME`.
2. Check the configured public and private bucket names directly with `HeadBucket`, without requiring account-wide bucket-list access or printing credentials. Create `R2_PRIVATE_BUCKET_NAME` only when that exact private name is absent.
3. Do not attach a public domain, public bucket policy, or `r2.dev` access to the private bucket. Verify `HeadBucket` succeeds for both buckets. Apply the private-bucket CORS policy for `https://*.schoolorbit.app` with `GET` and `HEAD`, then read the policy back before deployment continues.
4. Start the pinned `docker.io/clamav/clamav-debian` runtime. Persist `/var/lib/clamav`, expose no host port, and wait for its healthcheck before backend-school.
5. For a backend-only release, select the `backend` scope and wait for `/ready`. For
   the File Platform contract cutover, the coordinated school release first places
   school-api in maintenance mode, starts the cutover image, waits for
   `/ready`, migrates every active tenant, verifies every tenant reached the
   latest migration, and only then restores the normal proxy.

The school release workflow performs exact-name checks before creation through the pinned AWS CLI image, validates and promotes the canonical production Compose definition, starts `schoolorbit-clamd` when required, and recreates `schoolorbit-backend-school`. It does not restart backend-admin or create a second production topology.

To diagnose private browser delivery, request a fresh typed grant through the authenticated file-download endpoint and keep `data.url` in memory. Fetch that URL separately with the tenant `Origin`, credentials omitted, and referrer disabled. Confirm the R2 response includes a matching `Access-Control-Allow-Origin`; never print, persist, or paste the grant URL because its query string is a temporary bearer credential.

### File Platform contract cutover

Migration `032_file_platform_contract_cutover.sql` is the clean boundary from
the path-based compatibility schema to the final provider-neutral schema. Its
transactional preflight refuses to drop legacy columns when a logical file has
no version, a ready file has no matching current version, or a legacy profile
or achievement path lacks its file-ID replacement.

The coordinated school release workflow performs this cutover while the school
API returns a CORS-safe `503` maintenance response. It starts the new image,
waits for `/ready`, calls the internal all-tenant migration endpoint, and
restores normal traffic only when every active tenant reports the same latest
migration version with no failures. The raw migration response is kept in a
mode-`0600` temporary file and must never be printed because it can contain
tenant-specific failure details.

If readiness, migration, or proxy restoration fails, leave maintenance mode
and the cutover image in place, inspect only safe aggregate/error information,
and fix forward. After any tenant applies migration `032`, never run a
backend-school image older than commit `1bdeb0c5`; those binaries still query
columns that no longer exist.

### Durable lifecycle and recovery

Uploads scan and inspect bytes before reserving public delivery. Originals and derivatives use immutable versions. Delete revokes delivery in metadata first, then removes objects. Provider or metadata failures leave durable operations for the background reconciler; retries use leases, bounded exponential backoff, and a terminal attempt limit.

When reconciliation is unhealthy:

1. keep the backend running only if `/ready` is healthy;
2. inspect safe reconciliation counters and error codes, never raw keys or signed URLs;
3. restore scanner or bucket access before retrying terminal work;
4. compare file/version/derivative rows with object counts using file IDs and tenant/purpose aggregates;
5. do not manually delete metadata rows or reuse object keys.

### School font ownership and recovery

`school_font` is a private, scanned File Platform purpose. A new upload is staged either for the central school-font manager or for one exact certificate template; attaching a reviewed batch promotes the files into the school-owned library. The promoted font is not owned by a campaign or template, and campaign purge removes only its template references. Delivery remains grant-based and must never expose a bucket name, object key, provider URL, or signed URL in logs or persistent client state.

Deletion is reference-safe. `DELETE /api/school-fonts/{font_id}` returns `409` with the authoritative reference count while any template layout uses the font. Remove or purge those template references through the supported certificate workflow, re-list the library, and retry central deletion only after the count reaches zero. Never bypass this check by deleting `school_fonts`, `certificate_template_font_references`, File Platform metadata, or provider objects manually.

A successful central delete revokes file delivery in metadata before object cleanup. If provider deletion is delayed, leave the durable file operation intact and let the File Platform reconciler retry it with its normal lease, backoff, and terminal-attempt rules. Diagnose with safe file IDs, purpose totals, reconciliation counters, and error codes only. Restore scanner or bucket access before retrying terminal work; do not recreate the font row, reuse its object key, or purge the private bucket.

### Permanent certificate campaign purge

Certificate campaign purge is the only controlled exception that permanently removes File Platform metadata together with business data. It requires either the school-wide delete permission or the exact owner-unit delete permission, an exact campaign-name confirmation, and an unchanged impact snapshot. Starting it moves the campaign to `purging`; normal campaign reads and mutations then remain unavailable until the durable job finishes.

The purge first revokes file delivery and deletes every recorded object through the File Platform. Only after all inventory entries report deletion does the guarded database finalizer remove the campaign, templates, candidates, issue requests, issued and revoked certificates, audit rows, purge inventory, and file metadata in one transaction. Certificate counters are outside this deletion boundary and must never be reduced or reused.

When a purge is delayed or failed:

1. inspect only the API phase, deleted-file count, total-file count, and safe error code;
2. restore provider or database availability before retrying;
3. retry with `POST /api/certificates/campaigns/{campaign_id}/purge/retry` and continue polling the status endpoint;
4. treat status `404` as completion only after the purge was observed or accepted;
5. never delete campaign, purge-job, inventory, file-version, object, or file-metadata rows manually, and never print object keys, signed grants, raw provider errors, or recipient data.

If the campaign remains `purging`, leave the durable records intact so the reconciler can resume safely. Repair forward; do not restore the campaign to an editable status or bypass the finalizer.

The `rollback` image tag is advanced only after readiness and every active
tenant migration succeeds. For releases after the contract cutover, it may be
used only when its source commit is `1bdeb0c5` or newer. Keep migration `032`,
the private bucket, and the scanner volume; never restore a pre-cutover image
or move private objects into the public bucket. Roll frontend-school back
separately if its file-ID API contract is not compatible.

## Focused Troubleshooting

- Process unavailable: check container state and `/health`.
- Process healthy but not serving traffic: inspect `/ready` and its dependency, then service-network DNS/URLs.
- Backend-school cannot resolve tenants: check `BACKEND_ADMIN_URL`, internal secret/caller headers, request origin/subdomain consistency, and backend-admin readiness.
- Menu changes missing: confirm the post-deployment `Synchronize menu routes` step,
  `DEPLOY_KEY`, `SUBDOMAIN`, and backend route-registration response.
- Permission changes stale: verify migration rows, generated registry versions, cache invalidation, and the `permission_changed` client refresh.
- Migration checksum failure: restore the original migration file; add a new migration for the intended change.
- National IDs unreadable or unsearchable: stop writes and verify key/version configuration. Do not guess keys or overwrite ciphertext/blind indexes.
- File Platform not ready: inspect the safe `filePlatform` readiness field, then verify both bucket `HeadBucket` calls and `clamdcheck.sh` inside `schoolorbit-clamd`.
- Upload failure: verify R2 credentials, distinct bucket names, scanner health, purpose limits, and durable reconciliation state.

Use structured logs with correlation context, but redact secrets, cookies, national IDs, request bodies, and raw realtime query strings.

## Date calendar and activity request rollout

Migration [098](../backend-school/migrations/098_calendar_requests_and_date_context.sql) makes events independent of the academic header selection. It preserves existing event facts and timestamps, saves former year/term context in event provenance, and retains audience enrollment scope separately. It also creates private staff activity requests and grants `calendar.request.own` to staff roles and active organization units. Approval requires `calendar.manage.school`; pending and rejected requests never appear in calendar event reads.

Deploy the backend, generated permission/API contracts and frontend together behind the usual maintenance and all-tenant migration gates. Migration 098 removes academic context columns from events, so an older backend cannot be restored against the migrated schema. Recovery requires a compatible forward fix or a reviewed database restore; never alter migration history. Run the calendar database suite before rollout and verify request submission, manager approval and the resulting date-calendar entry on an isolated tenant.

Calendar request notifications reuse the central `NotificationService` through the application adapter in [request_workflow.rs](../backend-school/src/modules/calendar/services/request_workflow.rs). New requests notify active staff with both calendar read and manage permissions, resolved through the same role, organization and delegation grants as actor authorization. Decisions notify the active requester independently of the event audience notification option. Notifications are stored for the bell, published through tenant/user-scoped realtime delivery and sent to opted-in Web Push subscriptions using the existing VAPID configuration. Links lead to the pending review queue or the requester's approved/rejected history; rejection reasons remain in the private request details.

Delivery follows the existing calendar post-commit behavior: failures are logged with request/recipient identifiers and do not invalidate a saved request or decision. There is no durable notification retry queue; a delivery failure or process interruption after the mutation commits may leave a notification unsent. Investigate the calendar request notification failure messages before considering an explicit retry. Never retry the request approval mutation to resend a notification, and never submit test notifications to real tenant users.
