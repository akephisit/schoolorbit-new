# Testing

Use focused tests while implementing, then run every applicable section below. Commands that need databases, credentials, browsers, or deployed services must be reported explicitly when the required environment is unavailable; do not present an unrun check as passing.

## Reporting Verification

Record:

- the exact command that ran;
- whether it passed, failed, or was skipped;
- the relevant failure when it did not pass;
- the missing environment variable or external dependency when it could not run.

Do not replace a failed check by disabling it or by running a narrower command that misses the failure.

## Every Change

From the repository root:

```bash
git diff --check
git status --short
```

Review the final diff and run focused tests for the behavior changed before broad checks.

## Backend School

From `backend-school`:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
```

Run `cargo test -p <package>` for each changed internal crate and focused unit or integration tests
for changed application modules. For API-contract work:

```bash
cargo test api_contract::tests -- --nocapture
```

Foundation ownership can be checked independently:

```bash
cargo test -p school-errors
cargo test -p school-http
cargo test -p school-authorization
cargo test -p school-auth -- --test-threads=8
cargo test -p school-academic-core -- --test-threads=8
cargo test -p school-academic-delivery -- --test-threads=8
cargo test -p school-academic-timetable -- --test-threads=8
cargo test -p school-academic-assessment -- --test-threads=8
cargo test -p school-academic-results -- --test-threads=8
cargo test -p school-academic-lifecycle -- --test-threads=8
cargo test -p school-workflow
cargo test -p school-question-bank -- --test-threads=8
cargo test -p school-admission -- --test-threads=8
cargo test -p school-supervision -- --test-threads=8
cargo test -p school-students -- --test-threads=8
cargo test -p school-staff -- --test-threads=8
cargo test -p school-calendar -- --test-threads=8
cargo test -p school-tenancy
cargo test -p school-file-platform
cargo test -p school-crypto
cargo test -p school-fonts -- --test-threads=8
cargo test -p school-certificates -- --test-threads=8
cargo test -p school-test-db
```

`school-test-db` is a development dependency only. Database-backed root tests import its helpers
directly and still run through `scripts/test_backend_school.sh`; production code and the runtime
dependency graph must never depend on it. Permission-authority tests must preserve the application
ordering of session-identity invalidation, permission-cache invalidation, and only then realtime
notification.

`school-tenancy` owns backend-admin retry behavior and tenant pool/lazy-migration coordination.
Its package tests include disposable-database coverage through its dev-only `school-test-db` edge;
normal release dependency trees must expose neither `test-support` nor `school-test-db`.

`school-auth` owns session credentials, policy, persistence, throttling, audit events, the identity
cache, user-profile domain operations, and the root-independent auth runtime. Its package tests use
dev-only database and test-support edges. The application package retains Axum handlers,
cookie/CSRF and origin adapters, File Platform profile-image orchestration, and deliberate
cross-domain tests such as staff soft-delete followed by session invalidation.

Personnel rank milestones share a pure versioned calendar calculator in `school-staff`. Run its state/date and bounded scoped database cases with `./scripts/test_backend_school.sh --package school-staff rank_milestone -- --nocapture`, and the authorized HTTP checks with `./scripts/test_backend_school.sh modules::staff::career_integration_tests -- --nocapture`. The fixture covers 53 canonical histories, chronological 50-row pages, own access, missing staff information, all profile scopes, status filters and denied access. Against a ready production preview, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/personnel-workflow.spec.ts tests/e2e/staff-career-workflow.spec.ts tests/e2e/rank-milestones-workflow.spec.ts --project=chromium --workers=2` from `frontend-school`; milestone coverage includes conditional-review wording, independent errors/retry, superseded status, paging and mobile/desktop light/dark layouts.

Delivery and timetable versions have independent publication lifecycles. Rehearse their canonical
migration and concurrent writers against disposable PostgreSQL using:

```bash
./scripts/test_backend_school.sh --integration delivery_versions -- --nocapture
./scripts/test_backend_school.sh --integration delivery_versions zero_course_periods_preserve_delivery_and_require_removing_existing_lessons -- --exact --nocapture
./scripts/test_backend_school.sh --integration delivery_draft_lifecycle -- --nocapture
./scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::services::timetable_ -- --nocapture
./scripts/test_backend_school.sh modules::academic::lifecycle::services::term_preparation_tests -- --nocapture
./scripts/test_backend_school.sh modules::system::handlers::migration::tests -- --nocapture
./scripts/test_backend_school.sh --package school-academic-delivery
./scripts/test_backend_school.sh --package school-academic-timetable
```

The migration fixtures cover stable identities, a delivery graph shared by multiple tables,
dated A/B/A source chains, independent placement drafts, mixed opening/placement drafts,
explicit legacy target inclusions, early publication boundaries, unmappable sources/targets/historical
groups, fresh reconciliation gates, and safe retry. Runtime
cases cover separate opening publication, exact instructor handoff, source reconciliation,
semantic content comparisons, publication receipts, deletion rollback, and serialized writers.
Against the ready local production preview, run the opening/table workflows from `frontend-school`:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/homeroom-delivery-workspace.spec.ts \
  tests/e2e/delivery-offering-route-loading.spec.ts \
  tests/e2e/academic-teacher-change-handoff.spec.ts \
  tests/e2e/timetable-version-workspace.spec.ts \
  tests/e2e/timetable-board-templates-region-loading.spec.ts \
  tests/e2e/timetable-drag-board.spec.ts \
  tests/e2e/timetable-teacher-board.spec.ts \
  tests/e2e/timetable-whole-school-overview.spec.ts \
  tests/e2e/timetable-dense-board-layout.spec.ts --project=chromium --workers=2
```

The draft-lifecycle fixtures also verify migration 088→089 with unchanged published facts,
nullable draft dates, DELETE-only cancelled guards, optimistic counts/revisions, protected references,
and whole-batch rollback, including an unknown downstream foreign key. Delivery service tests cover
candidate-date hashes, fresh references, dated instructors, idempotency and concurrent writers.
The opening workspace browser fixture exercises repeated finding keys, eager names and before/after
differences, scoped retry, date-change invalidation, deletion refusal/success, read-only controls,
keyboard confirmation and mobile/desktop in both themes. Use protected tenant copies for operational
rehearsal, and keep production cleanup separate from routine disposable-schema tests.

Fixtures use synthetic staff/resources and generated API shapes. A published opening never
implicitly creates or publishes a timetable or student roster. Opening snapshots retain their
course/group/teacher facts; dated student memberships remain on their separate roster lifecycle.

The six `school-academic-*` crates own Core, Delivery, Timetable, Assessment, Results, and
Lifecycle respectively. Lifecycle and Supervision expose dev-only integration-support features
only for root tests that intentionally span domain owners; the normal release graph must keep
those features disabled. The application supplies external readiness, timetable consequence,
result-lock, notification, cache, and realtime adapters without duplicating crate-owned SQL.

`school-workflow`, `school-question-bank`, `school-admission`, `school-supervision`,
`school-students`, `school-staff`, and `school-calendar` own their domain models, policies where
applicable, persistence, business rules, and focused tests. Root keeps HTTP/OpenAPI composition,
cross-domain deletion and side-effect orchestration, Calendar notification scheduling, and Parent
aggregate views. Keep database-heavy package suites at no more than eight threads when they share
a disposable PostgreSQL instance.

`school-file-platform` owns provider-neutral file inspection, purpose/object-key policy, repository,
storage and malware-scanner implementations, lifecycle operations, runtime configuration, and
reconciliation. Its package tests exercise canonical migrations through a dev-only
`school-test-db` edge. Axum handlers, wire models, tenant/actor resolution, cross-domain
relationships, and cross-domain file authorization remain in the application package.

`school-crypto` is the only application-field encryption and blind-index implementation.
`school-fonts` owns the school font library and its typed staging relationships.
`school-certificates` owns certificate policy, layout, issuance, rendering, verification, rate
limiting, and purge behavior. Their Axum handlers, request context, OpenAPI composition, and
cross-domain deletion orchestration remain in the application package. Use at most eight test
threads for the database-heavy font and certificate package suites so concurrent migration
fixtures do not exhaust the disposable PostgreSQL instance.

The static architecture suite owns backend-only boundaries such as tenant request context, thin handlers, service/database separation, permission invalidation, structured logging, and realtime identity.

### School session authentication

Session changes require the schema, repository, service, HTTP/middleware, and realtime
boundaries—not only a login happy path. From the repository root, run the crate-owned schema,
repository, cache, and service tests against disposable PostgreSQL:

```bash
./scripts/test_backend_school.sh --package school-auth session_schema_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_repository_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_cache_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_service_tests -- --nocapture --test-threads=8
```

From the repository root, use the disposable local PostgreSQL runner for application-owned HTTP,
realtime, profile/File Platform, and cross-domain behavior:

```bash
./scripts/test_backend_school.sh modules::auth::session_http_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::profile_integration_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::staff_integration_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::websockets::security_tests -- --nocapture
```

From `frontend-school`, verify the browser security boundary and client state:

```bash
node --test tests/static/session-auth-contract.test.mjs \
  tests/static/account-security.test.mjs \
  tests/static/auth-session-state.test.mjs
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test --list tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
```

The destructive `session-security.spec.ts` account must be dedicated and disposable because the suite intentionally revokes a selected browser and then logs out every session. Never fall back to `SMOKE_*`, normal `E2E_USERNAME`/`E2E_PASSWORD`, or an operator account for that file.

## Backend Admin

From `backend-admin`:

```bash
cargo fmt --all -- --check
cargo test
cargo check
```

Use focused test filters first when changing a handler, client, or service.

## Frontend School

`npm ci` installs the app toolchain (TypeScript 6) and the private `tools/api-contracts` workspace (TypeScript 5 required by `openapi-typescript`). The API generator resolves its CLI from that workspace; keep its peer dependency isolated when upgrading app tooling.

`runed`, used by the UI primitives, still declares an optional Kit 2 peer for its separate Kit utilities. The scoped override selects Kit 3 for the portable utilities used by Bits UI and Svelte Toolbelt, which do not import `runed/kit`. The Kit-specific utilities still use retired Kit APIs; do not import `runed/kit` into the applications until upstream supports Kit 3. Verify the installed graph with `npm ls @sveltejs/kit svelte vite typescript --all` and run the mocked browser acceptance after dependency changes.

From `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:route-loading
npm run test:static
npx tsc --project src/service-worker/tsconfig.json
```

Spreadsheet fixtures cover displayed values, zero-padded IDs, literal formula text, CSV quoting, template widths, and multi-sheet rejection. From `frontend-school`:

```bash
node --test tests/static/spreadsheet.test.mjs tests/static/certificate-importer.test.mjs
PUBLIC_BACKEND_URL=http://127.0.0.1:4173 PUBLIC_VAPID_KEY=test npx playwright test tests/e2e/certificate-import-review.spec.ts --project=chromium --workers=1
```

Against the production preview described below, run `admission-final-region-loading.spec.ts` and `admission-exam-room-region-loading.spec.ts` for student-ID import, template downloads, and room exports. These use disposable mocked data and do not mutate a tenant.

The public school homepage uses disposable API fixtures, including streamed loading, regional retries, empty academic context, and mobile/desktop organization disclosures. Its Rust tests use isolated tenant schemas and the real router to verify anonymous access, tenant isolation, current enrollment/room movement, staff position counts, and current members of every position. Run:

```bash
# Repository root
./scripts/test_backend_school.sh modules::school -- --nocapture
# frontend-school (the browser specs start their own local servers)
node --experimental-strip-types --test tests/runtime/public-school-organization.test.ts
node --experimental-strip-types --test tests/runtime/public-school-seo.test.ts
npx playwright test tests/e2e/landing-page.spec.ts tests/e2e/admin-landing-page.spec.ts --project=chromium --workers=1
```

The homepage spec also checks the first HTML with JavaScript disabled, unique metadata and H1, safe School JSON-LD, a three-second identity timeout, concurrent sibling reads, retries, and crawler endpoints. The public logo resolver covers current-crest-only selection, cookie omission, and missing/unavailable files. SEO runtime cases cover tenant-specific canonical/sitemap URLs, the configured base domain, and exclusion of sandbox/local/preview hosts. After release, fetch each production homepage without executing JavaScript and verify its school name, one title/description, its own HTTPS canonical, and its sitemap; check `noindex` on sandbox and login, and fetch its `/school-logo` without Origin or Referer to verify anonymous crawler delivery.

Login and app-header layout fixtures also start their own local servers. They cover small and short viewports, optional school branding failures, pending/rejected login, and the remaining header controls:

Keep these self-hosted dev-server specs on one worker when combining them: Kit's generated files share one project directory, so concurrently starting independent Vite servers is not a valid test environment. The production-preview mocked route gate below can use two workers.

Run self-hosted browser specs after `test:static` and `check` have finished, and before starting a build. The static academic-context cases also start Vite in the same project; overlapping them can rewrite generated public environment modules and hot-reload a running browser fixture with the wrong API origin.

```bash
npx playwright test tests/e2e/login-layout.spec.ts tests/e2e/header-layout.spec.ts --project=chromium --workers=1
```

During implementation, run the relevant static file directly:

```bash
node --test tests/static/<area>.test.mjs
```

`test:route-loading` requires every authenticated page to have a completed data-owner review and a primary loader when route-owned. It rejects retired migration allowances and primary mount reads, follows local mount helper calls, and records browser subscription reasons and focused test owners in the inventory. It also guards focused invalidation, safe preload and the absence of generic page-view endpoints.

The staff tools QR workflow uses synthetic staff/student/parent sessions and public branding fixtures. Against the local production preview below, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-tools-qr.spec.ts --project=chromium --workers=2` from `frontend-school`. It decodes downloaded PNGs with the test-only `jsQR` dependency, verifies exact 1024-pixel output and preview/download equality, checks logo validation/retry/supersession, and captures mobile/desktop light/dark states. Pure input validation and catalog menu registration run in `node --test tests/static/qr-code.test.mjs`. Run `npx playwright test tests/e2e/file-download-cors-cache.spec.ts --project=chromium --workers=1` separately for the self-hosted HTTP-cache regression: it first reproduces a CORS failure from a cached branding image, then verifies a fresh byte download succeeds without request interception.

For the Academic Delivery region-loading browser gate, build and preview the frontend with a local API origin in one terminal, then run the mocked Chromium spec in another:

```bash
PUBLIC_BACKEND_URL=http://127.0.0.1:4173 PUBLIC_VAPID_KEY=test npm run build
PUBLIC_BACKEND_URL=http://127.0.0.1:4173 PUBLIC_VAPID_KEY=test npm run preview -- --host 127.0.0.1 --port 4173
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/route-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-catalog-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-foundation-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/curriculum-delivery-alignment.spec.ts tests/e2e/curriculum-publications.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-year-collections-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/delivery-offering-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/personal-daily-timetable-region-loading.spec.ts tests/e2e/timetable-daily-overview-layout.spec.ts tests/e2e/staff-own-timetable-grid.spec.ts tests/e2e/staff-own-timetable-pdf.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/timetable-board-templates-region-loading.spec.ts tests/e2e/timetable-teacher-board.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/exam-schedule-lists-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/exam-schedule-detail-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/question-bank-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/gradebook-workflow.spec.ts tests/e2e/result-preparation-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-result-locking.spec.ts tests/e2e/result-corrections-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-term-aggregates.spec.ts tests/e2e/academic-annual-results.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-promotion-policies.spec.ts tests/e2e/academic-promotion-runs.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-term-lifecycle.spec.ts tests/e2e/academic-year-lifecycle.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-round-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-application-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-exam-room-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-score-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-home-region-loading.spec.ts tests/e2e/staff-role-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/supervision-management-region-loading.spec.ts tests/e2e/supervision-queues-region-loading.spec.ts tests/e2e/supervision-detail-region-loading.spec.ts --project=chromium --workers=2
```

Run the cross-domain mocked acceptance together against a ready local production preview:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  '.*(region-loading|route-loading)\.spec\.ts$' \
  tests/e2e/gradebook-workflow.spec.ts \
  tests/e2e/curriculum-delivery-alignment.spec.ts \
  tests/e2e/academic-term-aggregates.spec.ts tests/e2e/academic-annual-results.spec.ts \
  tests/e2e/academic-promotion-policies.spec.ts tests/e2e/academic-promotion-runs.spec.ts \
  tests/e2e/academic-term-lifecycle.spec.ts tests/e2e/academic-year-lifecycle.spec.ts \
  --project=chromium --workers=2
```

Mocked route suites block service workers so same-origin requests reach Playwright interception. Confirm preview readiness and warm the server before starting the command; do not edit product files or run another Playwright invocation concurrently.

For deployed readonly route acceptance, use the ignored `frontend-school/.env.e2e.local` (mode `0600`) with HTTPS `E2E_BASE_URL`, `E2E_API_URL`, and pairs `E2E_STAFF_USERNAME/PASSWORD`, `E2E_STUDENT_USERNAME/PASSWORD`, `E2E_PARENT_USERNAME/PASSWORD`. The parent account must have a linked child with academic history in the same tenant. Supply values locally or through CI secrets, never in a chat, command argument or Git. Node 24 loads this file explicitly; Playwright does not load it automatically.

```bash
E2E_ROUTE_LOADING_LIVE=1 node --env-file=.env.e2e.local \
  node_modules/@playwright/test/cli.js test tests/e2e/route-loading-live-acceptance.spec.ts \
  --project=chromium --workers=1
```

The readonly live spec also accepts `SMOKE_USERNAME/PASSWORD` for its staff login when deliberately exported from the trusted Bash-owned `.env.smoke.local`. Do not load a Bash env file containing shell escapes with Node `--env-file`; encode literal values in the dedicated dotenv file instead. It navigates only, then logs out its own newly created session; it never opens mutation workflows or revokes other sessions. It discards one warm-up and records medians from exactly five warm runs with fixed role, permissions, academic context, Chromium and default network, using client navigation with preload disabled. Output contains route templates, region names, durations, bytes and available server phases only. Browser artifacts are disabled. Missing credentials or academic context are explicitly unrun. A missing baseline or query plan prevents claims of production speedup or SQL improvement.

The Delivery spec uses five warm mocked navigations to verify sanitized timing and size summaries. Use the same helper for credentialed, representative environment measurements; keep those results in release evidence rather than committing tenant-specific output.
The catalog spec checks the three route-owned list regions, focused retry, local group patching, and lazy history after create.
The foundation spec checks route-first setup/curriculum regions, focused retry, lazy period reads, and edition creation without owner/grade lookups surviving a stale overview response.
The publication spec covers a published edition opening an amendment, adding term two, publishing the whole edition, reading the baseline unchanged, stale-edition refusal, read-only permissions, independent history retry and superseded selection. It also checks permanent amendment discard, keeping a draft from the confirmation dialog, unchanged publication history and a fresh preview after a conflict. History and discard confirmation layouts run at desktop/mobile sizes in light/dark themes. Native Academic Core service tests additionally verify baseline preservation, live requirement references, concurrent draft reuse, stale tokens, atomic validation failure, immutable snapshots and deferred completeness. Discard cases restore the complete graph and removed live requirement IDs, remove new resources, restore the latest publication, refuse a changed preview or a never-published edition and roll back when an external reference prevents deletion. The delivery apply test verifies source metadata in previews, applied runs and snapshots while retained legacy offerings keep unknown provenance.

Run the pure publication token/source and semantic comparison checks with `cargo test -p school-academic-core curriculum_publication --lib` from `backend-school`. Run the native database cases with `./scripts/test_backend_school.sh curriculum_publication -- --test-threads=4` from the repository root; the CI Academic Core consumer gate includes them in the full service suite. The same workflow runs `BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh canonical_seed_is_idempotent_across_student_year_and_placement -- --test-threads=1` to verify one visible publication, stable placement identities and preservation of an open staff amendment during repeat seeding.

The curriculum detail spec checks independent edition/level reads, regional retry, invalid or mismatched level refusal, read-only alignment, covered-grade level creation, plan creation and copying a selected published program with conflict handling. Copy and later-year room-selection scenarios cover desktop/mobile in light/dark themes. Database tests additionally verify whole-edition atomic publication, source preservation, semantic term-slot copying, uncovered-grade and stale-revision refusal, migration retry and permission-grant metadata preservation.

For coordinated school release behavior, run:

```bash
node --test tests/runtime/maintenance-controller.test.mjs \
  tests/static/school-release-deployment.test.mjs
node --test ../scripts/tests/school-release-scope.test.mjs \
  ../scripts/tests/school-release-replay.test.mjs
node --test ../scripts/tests/worker-release-candidates.test.mjs
```

The push worker browser harness reads the production-built `service-worker.js` unchanged and serves it on an isolated local origin. Build `frontend-school` first, install Playwright Chromium and WebKit with their Linux dependencies, then run:

```bash
npx playwright test tests/e2e/service-worker-production.spec.ts --project=chromium --workers=1
```

This harness launches both browser engines for fresh direct navigation, reload, offline recovery and replacement of an old intercepting worker without losing entered data. Its Push test uses Chromium's full browser channel; Linux WebKit does not expose Playwright's worker evaluation API. Opening activity fixtures in `homeroom-delivery-workspace.spec.ts` block workers and cover central/grouped activation separately. Do not build while a production preview or browser suite is running; restart the preview after changing its build output.

## Frontend Admin

From `frontend-admin`:

```bash
npm run lint
npm run check
npm run test:unit
npm run build
```

## Container Runtime, Installer, and Production Topology

When the VPS installer, canonical Compose runtime, Nginx templates, deployment workflows, or their durable documentation changes, run from the repository root:

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
node --test scripts/tests/school-release-scope.test.mjs \
  scripts/tests/school-release-replay.test.mjs \
  scripts/tests/worker-release-candidates.test.mjs
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/prune_runtime_images.sh scripts/clamd_runtime_matches.sh \
  scripts/test_backend_school.sh scripts/resolve_school_release_scope.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/prune_runtime_images.sh scripts/clamd_runtime_matches.sh \
  scripts/test_backend_school.sh scripts/resolve_school_release_scope.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test scripts/tests/prune-ghcr-versions.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
podman run --rm -v "$PWD:/repo" -w /repo docker.io/rhysd/actionlint:1.7.7
```

The deployment static guard renders a proxy template into a temporary target, rejects invalid domains without replacing existing output, enforces the single production Compose owner, and confirms backend workflows verify the selected origin rather than the public hostname. Report an unavailable Bats, Podman, or Podman Compose dependency as unrun; do not replace its check with a narrower command.

For focused deployment-runtime work, run:

```bash
bats scripts/tests/installer/runtime_image_retention.bats \
  scripts/tests/installer/deployment_timing.bats \
  scripts/tests/installer/clamd_runtime_matches.bats
node --test scripts/tests/prune-ghcr-versions.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

The retention tests use only fake Podman and a loopback HTTP server. They must not contact the
production VPS, delete a live package version, or replace the manual GHCR dry-run review. Dockerfile
changes also require Podman runtime-target builds and config inspection:

```bash
podman build --target runtime -t schoolorbit/backend-admin:verification backend-admin
podman build --target runtime -t schoolorbit/backend-school:verification backend-school
podman image inspect schoolorbit/backend-admin:verification \
  --format '{{.Config.User}} {{json .Config.Cmd}}'
podman image inspect schoolorbit/backend-school:verification \
  --format '{{.Config.User}} {{json .Config.Cmd}}'
```

Docker Buildx remains limited to the GitHub Actions backend-image build jobs; repository-local runtime, database tests, image inspection, and workflow linting use Podman without a Docker socket or compatibility alias. The guard also owns CI cache policy: backend-admin and backend-school use distinct BuildKit scopes, while API and permission contract jobs share a dependency-oriented backend-school Rust cache. Pull requests are restore-only, and only trusted `main` runs may save it. A cache miss must execute the complete workflow rather than bypassing a gate. API Contract keeps artifact generation/offline export, backend validation, and frontend validation in independent jobs without `needs`; the static guard owns this division and its single-writer Rust cache policy.

The installer Bats directory includes focused Cockpit coverage:

- `cockpit_provider.bats` exercises Tunnel creation/adoption, ingress, connector IP, CNAME drift, memory-only token handling, and management rollback without deleting Tunnels;
- `cockpit_remote.bats` executes the remote configurator against an isolated filesystem and checks loopback-only listening, root prohibition, mode-`0600` token storage, pinned amd64/arm64 cloudflared artifacts, and idempotency;
- `vps.bats` verifies separate SSH stdin streams for the tracked script and secret JSON plus a fresh verification session;
- `orchestration.bats` verifies phase ordering, dry-run mutation absence, publish journaling, resume, standalone rollback, and full migration rollback.

Do not replace these focused files with source-only assertions. The deployment static guard supplements them by preventing Compose, firewall, installer-entry-point, and canonical-document drift.

## Permission Contract

`contracts/permissions.json` is the handwritten permission source. The registries and lock are generated files; do not edit generated files directly.

After a permission definition changes, from `frontend-school`:

```bash
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Commit the contract, `contracts/permissions.lock.json`, backend registry, frontend registry, migration when DB permission data changes, and focused authorization tests together.

## API Contract

The API Contract workflow also runs curriculum migration preservation/refusal and Academic Core, activation, promotion, delivery and certificate consumer tests with the native rootless Podman database runner. Frontend validation includes full lint; all jobs remain independent and pull requests do not save the shared Rust cache.

Rust DTOs and OpenAPI annotations own the wire contract. The tracked output is `contracts/openapi/school-api.json`; generated TypeScript lives under `frontend-school/src/lib/api/generated/`. These are generated files; do not edit generated files directly.

After a documented DTO or endpoint changes, from `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Generation must work offline without database credentials or a running backend.

## Database and Migration Tests

Never edit an applied migration. Add a new sequential file and test it against isolated state.

Routine backend-school database tests run on the developer's computer. From the repository root:

```bash
# Complete root application binary suite; PostgreSQL runs in rootless Podman on this computer.
./scripts/test_backend_school.sh

# Focused database-backed test.
./scripts/test_backend_school.sh --package school-auth \
  session_repository_tests -- --nocapture

# Database-backed seed utility test against the same disposable PostgreSQL boundary.
BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh \
  tests::canonical_seed_is_idempotent_across_student_year_and_placement \
  -- --exact --nocapture --test-threads=1
```

The runner requires the local rootless Podman engine and rejects `CONTAINER_HOST` or `CONTAINER_CONNECTION`, so it cannot accidentally select a remote runtime. Cargo and its compilation cache stay on the computer, while PostgreSQL uses a fresh anonymous disk-backed volume so the complete migration-backed suite is not capped by a 5 GiB data tmpfs. Ensure the local Podman storage has sufficient free disk space. The runner removes its uniquely named PostgreSQL container and associated anonymous volume after success, failure, `INT`, `TERM`, or `HUP`; it never reuses a named volume or prunes unrelated resources. An uncatchable termination such as `SIGKILL` or a host crash can leave these test resources behind and requires exact-target cleanup. It replaces any inherited `TEST_DATABASE_URL` only for the Cargo child and never uses `DATABASE_URL`. Direct Cargo against a persistent Neon URL is not the routine test recipe.

Tests continue to isolate their schema/data within the disposable database. The local runner removes the whole database container and its anonymous data volume after the command, including on test failure.

`BACKEND_SCHOOL_TEST_BIN` defaults to `backend-school`; set it to another local package binary only
when that binary's tests require the disposable PostgreSQL instance. The value is passed as one
quoted Cargo `--bin` argument and never changes the container or connection boundary. Use
`--package <internal-crate>` instead to test a crate-owned suite; the runner validates the name
against `backend-school/crates/<internal-crate>/Cargo.toml` before starting Podman. A focused filter
must execute at least one test, otherwise the runner fails instead of accepting Cargo's zero-match
success.

Permission reconciliation regressions can be checked with
`./scripts/test_backend_school.sh --release --locked batch_ -- --test-threads=1`.
The focused tests count actual PostgreSQL upsert statements, preserve retired permission
references, verify rollback/retry, and exercise concurrent pool initialization. The centralized
migration runner owns the single permission reconciliation, including when no migration is pending.

Run `node --test --test-concurrency=1 scripts/tests/r2-cors.test.mjs` from the repository
root with Bash, Node and jq available. It executes the workflow CORS fragment against a
controlled AWS CLI boundary, covering unchanged/reordered policies, full-policy drift,
missing configuration and read/write/verification failures without contacting R2.

### Personnel migration and browser verification

Run the canonical personnel owner and its historical SQL migration fixtures against fresh rootless PostgreSQL:

```bash
./scripts/test_backend_school.sh --package school-staff -- --test-threads=1
./scripts/test_backend_school.sh modules::staff -- --test-threads=1
./scripts/test_backend_school.sh policies::staff_access_policy::tests -- --test-threads=1
```

The staff suite covers exact degree aliases, unmapped/ambiguous inputs, locked-source drift, retry, identity/license/employment preservation, person-owned text validation, preserved custom/inactive positions and deactivation races, nullable patches, scoped directory/aggregate equivalence, current group deduplication and missing-value buckets. Career tests additionally cover migration 085 preservation and deferred owner/kind/value guards, transactional current projections, reasoned corrections, stale revisions, concurrent writers, safe UUID retries, staff-scoped cursors, optional dates and writer-only acknowledgements. Root HTTP tests exercise own/unit/tree/school reads, denied writes and the production session boundary. Historical migration readers are private test-only helpers; production has no personnel preflight endpoint or cutover report.

For provider rehearsal, create a disposable copy and supply its direct non-pooled URL privately as `MIGRATION_SCHEMA_DATABASE_URL`. Set `MIGRATION_SCHEMA_NAME=public` and `MIGRATION_SCHEMA_ALLOW_PUBLIC=1` only for that disposable copy, then run `cargo run --manifest-path backend-school/Cargo.toml --bin migrate_tenant_schema`. This existing CLI calls the centralized runner; never apply individual SQL files manually. Read actual SQLx version and bounded preservation/current-pointer integrity checks afterward; for migration 085 require imported dates/orders/actors to remain unknown and unchanged existing staff fields. Retain the historical `staff_personnel_simplification_audit` checks where relevant. Do not emit credentials, names, source values or national IDs.

The 82→83→84 tests preserve Thai text, nulls, shared labels, complete position UUIDs/timestamps and unrelated staff fields; stale, duplicate or missing preservation evidence blocks cleanup atomically. Run `./scripts/test_backend_school.sh --package school-staff personnel_simplification -- --test-threads=1` for this boundary. The manual `backend-school-neon-compatibility.yml` workflow discovers every declared test selection before creating a branch, runs auth/file schema checks in their current crate owners, and rejects successful Cargo commands with zero passing tests. Run `node --test scripts/tests/neon-compatibility.test.mjs` to verify discovery and failure propagation. The workflow runs the staff migration fixtures and the remaining schema/status selections against a fresh disposable direct-endpoint child; cleanup and expiry stay with that workflow. Run `node --test scripts/tests/migration-completion-gate.test.mjs` to exercise the actual all-tenant release filter through its native Podman jq image, including healthy reports without retired personnel fields, all-tenant coverage, future migration versions, other domain audit failures, and the complete thirty-check delivery/timetable reconciliation. Missing, partial, failed or stale delivery cutover evidence must refuse promotion.

Against a production build running in local preview:

```bash
cd frontend-school
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/personnel-workflow.spec.ts \
  tests/e2e/staff-career-workflow.spec.ts \
  tests/e2e/staff-home-region-loading.spec.ts \
  tests/e2e/staff-directory-region-loading.spec.ts --project=chromium
```

These synthetic browser fixtures block service workers and exercise position UUIDs, direct education-text payloads, lazy options, create/edit draft ownership, retry/refresh, chart drilldown, calendar dates, independent history loading/error/pagination, historical append retries, reasoned corrections, draft preservation on conflicts, pending actions and mobile light/dark layout. Pure helper and bounded draft-migration tests run within `npm run test:static`. Run deployed read-only acceptance separately with existing runtime credentials and tracing, screenshots and video disabled.

### Academic Core migration rehearsal

Run the Academic Core chain against disposable local PostgreSQL from the repository root. These
focused commands prove the read-only legacy preflight and each sequential transition through 041,
042, 043, 044, and the separately gated cleanup at 045; run them one at a time so failures remain
attributable. The preflight implementation retained here is test-only fixture validation; the
one-time runtime CLI is retired after Phase B.

```bash
./scripts/test_backend_school.sh \
  modules::academic::cutover_test_preflight_database_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_041_maps_core_fixture \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::migration_042_maps_delivery_fixture \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_043_maps_all_consumers \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_044_exposes_the_clean_academic_core_runtime_contract \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_removes_legacy_schema \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_fails_closed_without_current_reconciliation_evidence \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_accepts_a_valid_marker_with_distinct_reconciliation_counts \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_rejects_a_mapping_delete_committed_after_the_marker \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_rejects_a_deleted_expanded_delivery_target \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_rejects_a_deleted_legacy_mapping_source \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_rejects_source_field_drift_committed_after_the_marker \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::system::handlers::migration::tests -- --nocapture --test-threads=1
```

Then run the complete disposable PostgreSQL suite:

```bash
./scripts/test_backend_school.sh -- --test-threads=1
```

The passing legacy fixture must report stable aggregate source counts without writes, every blocking
fixture must return its expected bounded finding code, and migrations 041-044 must preserve mapped
counts and checksums. Reconciliation must read the actual tenant migration version, reject anything
other than exactly 044, run all aggregate checks without invoking the migration runner, create no
success marker when a check fails, and preserve one idempotent version-44 success marker when every
check passes. Tests and reports may contain schema labels, finding/check codes, durations, aggregate
counts, and checksums only—never source rows, people, database URLs, or credentials.

A protected tenant clone rehearsal is an external, explicitly authorized gate. Use the secret-backed
tenant connection inventory, keep output outside the repository, apply 041-044, run authenticated
multi-year/multi-term reads, and record only duration, aggregate counts/checksums, finding codes, and
pass/fail. When a separately reviewed Phase B branch contains migration 045, rehearse that review
copy on the same disposable clone after a current success marker and verify its cleanup manifest.
Never commit clone data or its output. The manual Neon migration compatibility workflow remains a
separate credentialed external gate and is unrun when its repository secret/variables are unavailable.

For browser coverage, discovery does not equal execution. Discovery must work without tenant
credentials:

```bash
cd frontend-school
npx playwright test --list tests/e2e/academic-context.spec.ts \
  tests/e2e/academic-core-cutover.spec.ts
```

Execution requires an isolated deployed target plus the dedicated accounts and permissions expected
by those specs. Report Playwright execution, protected-clone rehearsal, manual Neon compatibility,
and deployment smoke as `unrun` with the exact missing account, credential, target, or authorization;
do not report discovery as a passing browser workflow and do not silently omit an external gate.

### Gradebook and results cutover rehearsal

Run the Release 2 schema and migration-status audit against disposable local PostgreSQL from the
repository root:

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_060 -- --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::system::handlers::migration::tests::gradebook_results_status \
  -- --nocapture --test-threads=1
```

The audit verifies only durable schema, seed, and permission invariants. Its response contains
bounded check codes, counts, versions, and pass/fail state; it must never contain student identity,
scores, outcomes, credentials, or source rows. A legitimate school edit to grading or evaluation
configuration is not a deployment invariant and must not keep the tenant in maintenance.

The manual Neon migration compatibility workflow runs the same focused checks on a fresh disposable
child branch through its direct non-pooled endpoint. Schema-isolated migration cases use four test
threads so network round trips do not serialize the entire gate, while each individual test pool
still has one connection. The workflow then deletes the branch. Repository secrets and variables
remain the only credential source, and workflow output must not expose the connection URI.
An authenticated academic smoke samples at most two canonical terms and reads the Gradebook subject
workspace, learner-evaluation subject workspace, and result-readiness summary for each selected term.
Response bodies remain in the smoke script's private temporary directory and are removed on exit.

### Manual Neon migration compatibility

[Backend School Neon Compatibility](../.github/workflows/backend-school-neon-compatibility.yml) is an explicit `workflow_dispatch` gate. Configure names only in repository settings; never put their values in source:

```text
Secret:    NEON_TEST_API_KEY
Variables: NEON_TEST_PROJECT_ID
           NEON_TEST_PARENT_BRANCH_ID
           NEON_TEST_DATABASE
           NEON_TEST_ROLE
```

The project and parent branch must be dedicated to testing and contain no production data. Each confirmed run creates a unique ordinary copy-on-write child branch from that parent, retrieves a direct non-pooled connection URI, and provisions `uuid-ossp` plus `pg_trgm` explicitly in the child's `public` schema before migration/schema tests. The tests then create isolated schemas and run the active migrations themselves, so the parent needs only the configured empty database and an owner role allowed to create those Neon-supported extensions. The create request deliberately omits `suspend_timeout_seconds` because the test account owns that setting and rejects attempts to modify it. Branch ownership outputs are published before connection retrieval so later failures still clean up; the two-hour expiration is a fallback if finalization cannot run. API failures expose only bounded, sanitized code/message fields. The gate never requests a pooled URI because transaction pooling can expose the wrong schema-local `_sqlx_migrations` state.

The manual workflow defaults to `test_scope=full`, which retains all staff, auth, file, and academic compatibility selections. Select `test_scope=course-zero-periods` for migration 090 and the zero-course opening/publication lifecycle, including rejection of negative targets, preserved curriculum/teachers/history, zero waiting demand, refused new placement and explicit removal of existing lessons. Select `test_scope=timetable-subject-groups` for the focused timetable report metadata regression; it exercises the real course and current staff affiliation queries after migration 060 and retains nonempty-test enforcement, disposable branch creation, expiry, and cleanup.

The backend static architecture suite validates that active migrations remain a contiguous timeline beginning at `001_baseline.sql`. Runtime rollout and all-tenant migration verification are documented in [Operations](./OPERATIONS.md).

## Encryption and PII

For changes to encryption, national IDs, blind indexes, or admission PII, from `backend-school`:

```bash
cargo test -p school-crypto
./scripts/test_backend_school.sh --package school-admission services::pii::tests
cargo check --workspace --all-targets
```

These focused tests use test-only keys. Never put a real national ID, `ENCRYPTION_KEY`, or `BLIND_INDEX_KEY` in source, fixtures, command output, screenshots, or logs.

## Smoke Tests

The repository smoke script checks frontend reachability, backend liveness/readiness, CORS and CSRF preflight, legacy-cookie rejection, opaque-session login, `/api/auth/me`, the active-session list, realtime SSE, optional private files, and current-session logout.

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='provided-at-runtime' \
./scripts/smoke_test.sh
```

Alternatively, copy `.env.smoke.example` to the ignored `.env.smoke.local`. The script loads that file by default; `SMOKE_ENV_FILE` can point elsewhere. Credentials must come from `SMOKE_*` environment variables or the ignored environment file and must never be committed.

When `SMOKE_RESOLVE_IP` pins the API hostnames directly to an origin that uses a Cloudflare Origin
CA certificate, set `SMOKE_CA_CERT` to the readable pinned CA file. The option applies only to
admin/school API requests; the tenant frontend request continues to use the normal public trust
store.

If credentials are absent, authenticated checks are skipped. Report that limitation rather than describing the smoke suite as fully passing.

Set `SMOKE_ACADEMIC_CONTEXT=true` only for an explicitly selected cutover tenant. After login, the
script reads the context options, selects at most two canonical year contexts and two canonical term
contexts, and verifies the Academic Core, offerings, assessment, timetable, exams, supervision,
admission, staff-dashboard, Gradebook subject, learner-evaluation subject, and result-readiness read
paths. It stores response bodies only in its private temporary directory and removes them at exit.
During a reviewed maintenance cutover, run this mode through the backend deployment workflow's
VPS-loopback smoke input; never add a public maintenance bypass or open traffic temporarily for the
test. The workflow alone sets `SMOKE_DIRECT_BACKEND=true`; that mode skips only Nginx-owned
CORS/preflight assertions while login, session, CSRF, readiness, and every selected academic read
remain required.

The API-domain cookie is `__Host-schoolorbit_session`; it is opaque and unavailable to frontend JavaScript. Login and authenticated `/api/auth/me` responses expose `X-CSRF-Token`. The smoke script captures that value only in a shell variable, updates it after rotation-capable responses, and sends it on every authenticated `POST`, `PUT`, `PATCH`, or `DELETE`. Never print, export, persist, or enable trace output for the CSRF value. A manual flow should use private temporary files:

```bash
cookie_jar=$(mktemp)
headers_file=$(mktemp)
chmod 0600 "$cookie_jar" "$headers_file"
# Capture X-CSRF-Token from login or /api/auth/me into csrf_token without printing it.
# Remove both files and unset csrf_token when the check ends.
```

### File Platform smoke

Run this only against an isolated tenant with no retained files. The authenticated account must be allowed to update school settings for the public-logo case; the private-profile case operates on the logged-in user. Supply a small valid PNG through `FILE_SMOKE_PNG`; the repository smoke script creates and removes its own private cookie jar. Never commit the PNG, cookie jar, or captured CSRF value.

1. Upload `school_logo` through `POST /api/files`; retain only the returned file ID.
2. Confirm authenticated metadata from `GET /api/files/{id}` contains `publicContentUrl` but no bucket, object key, storage path, provider URL, or signed URL.
3. Confirm anonymous `GET /api/public/files/{id}/content` redirects and delivers the PNG. Also request `GET /api/public/files/{id}/delivery`, retain `data.url` only in memory, fetch it as a separate credential-free request with the tenant `Origin` and no referrer, and confirm a non-empty PNG plus matching `Access-Control-Allow-Origin`. Never print or persist the delivery URL.
4. Upload `profile_image` through `POST /api/files`; confirm anonymous metadata/download fails.
5. Confirm authenticated `POST /api/files/{id}/download` returns a `200` typed grant without bucket, object-key, or provider details. Keep `data.url` in memory, fetch it separately with the tenant `Origin` and credentials omitted, and confirm it writes bytes to a temporary output file. Never print the response body or grant URL because the URL is a temporary bearer credential.
6. Delete each file with `DELETE /api/files/{id}`. Repeat delete through the owning domain workflow where supported, and confirm delivery remains revoked even if object cleanup is pending.
7. Search backend and proxy logs for leaked signed-query markers, object-key prefixes, filenames, or content. A file ID and safe error code are allowed.

Representative requests, with credentials and IDs supplied only at runtime:

```bash
curl -fsS -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  -F purpose=school_logo \
  -F "file=@$FILE_SMOKE_PNG;type=image/png" \
  "$SMOKE_API_URL/api/files"

curl -fsSL \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  "$SMOKE_API_URL/api/public/files/$PUBLIC_FILE_ID/content" \
  -o /dev/null

curl -fsS -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  -F purpose=profile_image \
  -F "file=@$FILE_SMOKE_PNG;type=image/png" \
  "$SMOKE_API_URL/api/files"

curl -fsS -X DELETE -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  "$SMOKE_API_URL/api/files/$FILE_ID" \
  -o /dev/null
```

Exercise private delivery through the typed frontend helper or an in-memory test harness that
parses the grant and provider response without writing or printing the URL. Do not pipe the
grant envelope through verbose shell output or enable HTTP tracing.

For scanner failure behavior in a non-production environment:

1. stop `schoolorbit-clamd`;
2. confirm `/health` remains `200` and `/ready` becomes `503`;
3. confirm a valid upload returns `503` and creates neither ready metadata nor an object;
4. restart clamd, wait for `clamdcheck.sh`, and confirm `/ready` returns `200`;
5. upload the standard EICAR anti-malware test file and confirm it is rejected without becoming ready or publicly deliverable.

Run the focused adapter tests as well:

```bash
./scripts/test_backend_school.sh --package school-file-platform runtime_config::tests -- --nocapture
./scripts/test_backend_school.sh --package school-file-platform malware_scanner::tests -- --nocapture
./scripts/test_backend_school.sh --package school-file-platform r2_storage_provider::tests -- --nocapture
./scripts/test_backend_school.sh --package school-file-platform platform_service::tests -- --nocapture
```

## Browser E2E

Session-cache and hidden-tab regressions can be exercised without a live tenant:

```bash
./scripts/test_backend_school.sh modules::auth -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::notification::handlers::tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::academic::websockets::security_tests -- --nocapture --test-threads=1
cd frontend-school
node --test tests/static/realtime-idle.test.mjs tests/static/notification-idle-runtime.test.mjs tests/static/auth-refresh-races.test.mjs tests/static/timetable-socket-runtime.test.mjs
```

Cache tests cover shared query work, tenant isolation, expiry/maintenance deadlines, uncached
previous tokens, invalidation during reads, and cache hits with an unavailable database. A passing
query-count test is not a Neon cost measurement. Browser execution and deployed-proxy smoke remain
separate required rollout checks.

From `frontend-school`:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_USERNAME='provided-at-runtime' \
E2E_PASSWORD='provided-at-runtime' \
npm run test:e2e
```

`SMOKE_TENANT_URL`, `SMOKE_SUBDOMAIN`, `SMOKE_USERNAME`, and `SMOKE_PASSWORD` are accepted fallbacks only for the non-destructive login spec and the explicitly enabled readonly route acceptance spec. The backend sets `__Host-schoolorbit_session` for the API domain, so assertions inspect Playwright browser-context cookies rather than only cookies visible for the tenant page domain.

Run destructive multi-context session coverage separately with a dedicated disposable account:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_API_URL='https://school-api.schoolorbit.app' \
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test tests/e2e/session-security.spec.ts
```

Set `E2E_OTHER_TENANT_URL` to another tenant when tenant-isolation proof is available; only that optional case skips when the variable is absent. The suite never changes the account password.

### School font library

Run central authorization, inspection, atomic attach, reference-safe delete, and File Platform relationship checks from the repository root:

```bash
./scripts/test_backend_school.sh --package school-fonts -- --nocapture --test-threads=1
./scripts/test_backend_school.sh --package school-certificates -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::certificates::handlers::tests -- --nocapture --test-threads=1
```

Run the generated-contract, reusable-uploader, manager-only route, upload retry/cleanup, and delete-conflict coverage from `frontend-school`:

```bash
node --test tests/static/school-font-library.test.mjs \
  tests/static/certificate-*.test.mjs --test-concurrency=1
npx playwright test tests/e2e/school-font-library.spec.ts \
  tests/e2e/certificate-editor.spec.ts \
  tests/e2e/certificate-renderer.spec.ts --workers=1
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

The non-live browser suites use local harnesses and do not mutate a tenant. The live certificate lifecycle is separate: it uploads one private `school_font` through an exact template context, attaches and renders with it, proves the font survives campaign purge in the central manager list with a zero reference count, and deletes it only through `DELETE /api/school-fonts/{font_id}` afterward. Do not substitute direct File Platform deletion for this cleanup sequence.

Run the complete certificate lifecycle against an isolated tenant with dedicated preparer, issuer, and student accounts. From the repository root, run the focused backend checks first:

```bash
./scripts/test_backend_school.sh --package school-certificates -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::certificates::handlers::tests -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml --test static_architecture certificate_runtime_keeps_handlers_thin_proofs_private_and_renders_ephemeral -- --exact --test-threads=1
```

Then, from `frontend-school`, run the static contract and browser discovery before the live lifecycle:

```bash
node --test tests/static/certificate-*.test.mjs --test-concurrency=1
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
npx playwright test tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

The live lifecycle requires all of these variables, supplied only at runtime:

- `E2E_CERT_PREPARER_USERNAME`
- `E2E_CERT_PREPARER_PASSWORD`
- `E2E_CERT_ISSUER_USERNAME`
- `E2E_CERT_ISSUER_PASSWORD`
- `E2E_CERT_STUDENT_USERNAME`
- `E2E_CERT_STUDENT_PASSWORD`

The three accounts must be distinct. The preparer needs exact-unit campaign/template/candidate/submit/delete access, `font.manage.school` for the post-purge central list/delete assertion, and no school issue access; the issuer needs school read, issue, revoke, and download access; and the student must be an active linked student account. The isolated tenant also needs a current academic year, a second active organization unit outside the preparer's owner options, and working private-file storage and scanning.

Do not print or retain credential values, recipient data, verification proofs, render receipts, or delivery grants. Run this destructive lifecycle only against an isolated tenant: the fixture permanently purges the campaign it creates through the guarded impact/start/status API, then verifies that its issued and replacement certificates, own-certificate rows, campaign-owned file objects, and file metadata are unavailable while the school font survives campaign purge. It then confirms the font reference count is zero and removes the font through the central delete API. A failed purge is retried only through the supported purge API and the fixture still attempts the same guarded cleanup from `finally`. If any required variable is unavailable, the `--list` command must still pass; report the live lifecycle as unrun, not passing or skipped coverage.

Use `npm run test:e2e:headed` only when interactive debugging is needed. Retain traces, screenshots, and videos only when they contain no sensitive data.

## Realtime Rollout Checks

When WebSocket identity, proxying, or authentication changes:

1. Verify the handshake without legacy query identity fields.
2. Search proxy/backend access logs for unexpected query parameters, while ensuring sensitive query strings are not logged.
3. Confirm no deployed client sends legacy query identity parameters: `user_id`, `name`, or `school_key`.
4. Confirm the backend derives actor and tenant identity from authenticated server context and fails closed when authorization is lost.
5. Confirm ping/pong heartbeat, stale-client cleanup, reconnect backoff, and permission-change disconnect/refresh behavior through the reverse proxy.

Do not log tokens, cookies, identity query values, or full request URIs during rollout checks.

## Ubuntu 26.04 Playwright

Until Playwright provides native Ubuntu 26.04 browser builds, install and run with:

```bash
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npx playwright install chromium
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npm run test:e2e
```

Do not rely on `npx playwright install-deps` on Ubuntu 26.04 when it requests unavailable Ubuntu 24.04 packages. Install required native packages explicitly when needed:

```bash
sudo apt install -y libnspr4 libnss3 libasound2t64 libxss1 fonts-liberation
```

The sandbox E2E workflow uses Ubuntu 24.04.

## Date calendar and activity requests

Run `cargo test -p school-calendar` for date/time validation and request authorization policy. Run `./scripts/test_backend_school.sh modules::calendar -- --nocapture` from the repository root for database coverage: own-request isolation, manager decisions, failed approval rollback, concurrent approval deduplication, and migration 098 preservation of existing events, audience scope, reminders and timestamps. The request notification cases compare recipient resolution with actor authorization across roles, organization positions, wildcard and delegated grants; exclude inactive users and expired/revoked grants; and verify stored notifications, tenant/user-scoped realtime delivery, concurrent decision deduplication and committed outcomes when notification storage fails. Use disposable PostgreSQL and synthetic fixtures; never point this suite at a tenant database.

Against a local frontend preview, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/calendar-region-loading.spec.ts tests/e2e/calendar-requests.spec.ts tests/e2e/calendar-pending-overlay.spec.ts tests/e2e/calendar-timed-display.spec.ts tests/e2e/calendar-event-popovers.spec.ts tests/e2e/calendar-embed-dialog.spec.ts tests/e2e/calendar-action-menus.spec.ts --project=chromium`. The fixtures use synthetic staff sessions and intercept calendar APIs. Action-menu cases cover header and selected-day creation menus, date inheritance, manager/requester/reader permissions, disabled catalog actions, settings tools and the absence of an extra empty-month box. They do not exercise a deployed database. The request tests cover selected-day defaults in a future month, timed defaults, 24-hour compact input normalization, invalid time refusal, minimal payloads, failure draft retention, own status tracking, approval and rejection, including desktop and mobile layouts. Timed-display cases inspect centered dates, continuous all-day bars, per-day timed dots, retained edit values and desktop/mobile light/dark layouts; pure helper tests cover time boundaries, week-spanning layout and per-day timed lane packing. Event popover cases verify full-width staff calendars, direct event selection, complete details, manager/reader actions, keyboard focus return, outside mouse/touch dismissal without reopening another day, context changes and mobile/desktop light/dark layouts. Region-loading cases verify stable paired navigation buttons and an immediately visible new-month grid during delayed event reads. Public and staff mobile cases hide grid times while retaining full times in details and on desktop. Pending overlay cases cover inline manager approval/rejection with local draft retry, lazy detail loading and cancellation, compact filter popovers, removed notices, lazy reads, multiday and timed markers, toggle cancellation, month/history ownership, independent errors/retry, explicit overflow, scope controls and mutation patches. The calendar database suite also verifies oldest-first review paging and ties with approved requests excluded before pagination, manager-only detail access, newest-first own history, pending-only date overlap, private scope and bounded overlay results.
