# SchoolOrbit Technical Backlog

This file is the single active backlog for verified, unfinished technical work that spans services. It records what remains to be improved without replacing [`.rules`](./.rules), executable contracts, migrations, tests, or operational procedures.

## Maintenance Rules

- Work in priority order unless an incident or user-facing failure requires otherwise.
- Re-read [`.rules`](./.rules) and revalidate the referenced implementation before starting an item; this backlog records findings, not immutable runtime truth.
- Split the selected item into the smallest coherent change with its own tests and review.
- Never edit an applied migration. Add a new sequential migration for every database change.
- Never copy secrets, credentials, national IDs, database URLs, production data, or raw private payloads into this file.
- Keep only unfinished work here. Remove an item after its pull request records the implementation, verification, rollout, and any follow-up work.
- Update [Testing](./docs/TESTING.md) or [Operations](./docs/OPERATIONS.md) only when a durable command or procedure changes.

## P0 — Security and Privacy Blockers

- [ ] **SEC-001: Remove plaintext national IDs from control-plane admin authentication.**
  - Replace plaintext storage and lookup introduced by [admin migration 002](./backend-admin/migrations/002_create_admin_users.sql) with a username/email identity or encrypted value plus a keyed blind index.
  - Use new forward migrations with dual-read/dual-write, audited backfill, count verification, and a later plaintext-column removal. Do not edit applied migrations.
  - Ensure JWT claims, API responses, errors, logs, fixtures, and CLI output contain no national ID.
  - Done when plaintext values and lookup paths are gone, migration/backfill tests pass, and existing admins can authenticate through the replacement flow.

- [ ] **SEC-002: Retire fixed bootstrap credentials and unsafe database utilities.**
  - Rotate or disable credentials introduced by [admin migration 005](./backend-admin/migrations/005_seed_admin_user.sql) without recording their values.
  - Replace fixed credentials with an expiring, single-use bootstrap or invite flow that forces the administrator to set a password.
  - Keep destructive repair/reset utilities out of production artifacts and require explicit safeguards for intentional use.
  - Done when no reusable credential is committed or printed and existing affected accounts have a documented reset path.

- [ ] **SEC-003: Remove tenant database credentials from API contracts and storage exposure.**
  - Replace serialization of the full [school model](./backend-admin/src/models/school.rs) with explicit public and internal DTOs that exclude `db_connection_string` by default.
  - Split internal endpoints by caller so deployment/status consumers receive only the fields they need.
  - Prefer a secret-provider reference or envelope-encrypted credential with rotation support over plaintext storage.
  - Add contract tests that fail if credential-like fields reach public responses, errors, or logs.

- [ ] **SEC-004: Protect privileged admin migration endpoints and internal secrets.**
  - Require an authenticated administrator and exact permission on the frontend-admin migration proxy routes under [frontend-admin migration APIs](./frontend-admin/src/routes/api/migration).
  - Add service identity, rate limiting, immutable audit events, and network restrictions for internal migration operations.
  - Move internal secrets out of tracked configuration and fail deployment when required values are missing or placeholders.
  - Done when anonymous callers cannot read migration status or trigger migrations in direct and reverse-proxy tests.

- [ ] **SEC-006: Stop persisting credentials and personal identifiers in browser storage.**
  - Remove full-form local storage from [staff creation](./frontend-school/src/routes/%28app%29/staff/manage/new/+page.svelte).
  - Remove national ID and birth date session storage from the [public application status flow](./frontend-school/src/routes/%28public%29/apply/status/+page.svelte).
  - Replace resumable sensitive workflows with an opaque, expiring server-side token or secure HttpOnly cookie.
  - Done when a browser-storage inspection after each workflow contains no password, national ID, birth date, or private document identifier.

- [ ] **SEC-007: Replace predictable student and guardian passwords.**
  - Remove password derivation from student codes and phone numbers in the [admission application service](./backend-school/src/modules/admission/services/application_service.rs).
  - Use random, expiring, single-use activation tokens and require the user to set a policy-compliant password.
  - Force a safe reset for existing accounts created by predictable schemes.
  - Add rate limiting, progressive delay or lockout, enumeration-resistant errors, and MFA for control-plane administrators.
  - Done when no account type receives a deterministic initial password and reset/revocation tests pass.

- [ ] **SEC-008: Restrict notification creation to authorized server workflows.**
  - Remove or protect the generic authenticated notification creation route in the [notification handler](./backend-school/src/modules/notification/handlers.rs).
  - Use server-owned templates, validate recipients, and allowlist internal links.
  - Done when an ordinary authenticated user cannot send arbitrary content to another user.

- [ ] **SEC-009: Stop build-time route synchronization from deleting school-owned menus.**
  - Add explicit ownership such as `managed_by` or `source` to distinguish system routes from school-managed menu records.
  - Replace unscoped cleanup in [route registration](./backend-school/src/modules/system/services/route_registration_service.rs) with a transactional desired-state diff limited to frontend-owned rows.
  - Run synchronization as an explicit deployment step, fail visibly on partial scans, and test preservation of custom placement and labels.

## P1 — Identity, Data Integrity, and Durable Operations

- [ ] **AUTH-001: Introduce revocable sessions and consistent active-user enforcement.**
  - Add session or token revision, unique session identifiers, logout/revocation, password-change revocation, and shorter access-token lifetime.
  - Check active/suspended state through the shared request context for every authenticated operation.
  - Align cookie lifetime with token/session lifetime and add CSRF/origin protection for cookie-authenticated mutations.

- [ ] **AUTH-002: Minimize the current-user response.**
  - Remove decrypted national IDs and unnecessary personal fields from the default `/api/auth/me` response.
  - Provide separate PII endpoints guarded by exact permissions, resource policy, audit logging, and step-up authentication where appropriate.
  - Fail closed and redact when encrypted-field decryption fails; never return ciphertext as user data.

- [ ] **ADM-001: Replace repeated national-ID-plus-birth-date admission access with a portal session.**
  - After throttled verification or OTP, issue a short-lived opaque session for read, update, confirm, upload, and delete operations.
  - Add masked default responses plus separate admission PII view/export permissions.
  - Use one Bangkok-time round-state policy for visibility, application windows, submission, editing, and result publication.

- [ ] **ADM-002: Add applicant privacy, retention, and guardian accountability.**
  - Record versioned privacy-notice receipt, purpose/legal basis, guardian identity where required, retention deadline, and deletion/legal-hold state before an applicant has a normal user account.
  - Keep consent separate from other lawful bases and obtain DPO/legal review before claiming compliance.
  - Ensure admission audit events redact identifiers and private document metadata.

- [ ] **ADM-003: Make enrollment transaction-safe and idempotent.**
  - Replace `MAX + 1` student-code generation with a sequence or locked counter.
  - Lock or compare-and-swap the application state so concurrent retries cannot enroll twice.
  - Stop swallowing critical database errors and add an idempotency key for enrollment side effects.
  - Verify parent identity before linking an existing account by a shared or reused phone number.

- [ ] **ADM-004: Correct admission round ordering and state constraints.**
  - Calculate round ordering before filtering to one round in the [application service](./backend-school/src/modules/admission/services/application_service.rs).
  - Add focused integration tests for second and later rounds.
  - Add forward database constraints for valid round/application statuses and application-window invariants.

- [ ] **OPS-001: Convert provisioning and deletion into durable state machines.**
  - Split provider clients, orchestration, persistence, and progress delivery out of the current school service.
  - Persist idempotency keys, exact provider resource IDs, exact GitHub run IDs, leases, attempts, next retry, terminal state, and compensation state.
  - Mark a school active only after deployment readiness verification, not after workflow dispatch.
  - Preserve tombstones and cleanup metadata until Cloudflare, Neon, and deployment resources are reconciled.

- [ ] **DB-001: Repair immediate schema invariants through forward migrations.**
  - Resolve contradictory `users.status` checks so every frontend-visible state has one canonical database contract.
  - Add a primary key to `organization_permission_grants`.
  - Backfill then make defaulted boolean columns non-null where tri-state semantics are not intentional.
  - Add database checks for admission states and stable JSON object/array types.

- [ ] **API-001: Generate and enforce the backend-admin API contract.**
  - Add explicit camel-case request/response DTOs and one response envelope.
  - Generate or validate frontend-admin types from the backend contract instead of manual casts.
  - Add contract tests for school list/detail pagination and secret-field exclusion.
  - Fix the frontend-admin authentication initialization race and make lint pass without suppressions.

## P2 — Scale, Maintainability, Database Efficiency, and Delivery

- [ ] **SCALE-001: Make realtime events, permission invalidation, and scheduled jobs multi-replica safe.**
  - Replace process-local-only delivery with a shared event transport or authoritative revision polling.
  - Use leader election, advisory locks, or leased durable jobs so scheduled work executes once.
  - Define retry, ordering, deduplication, and reconnect behavior for notification, timetable, and permission signals.

- [ ] **SCALE-002: Establish tenant connection and job capacity limits.**
  - Model maximum tenant pools and database connections across replicas.
  - Add pool acquisition, eviction, queue depth, provider latency, retry, and failure metrics.
  - Introduce concurrency limits and fair scheduling when looping over tenants.

- [ ] **DB-002: Optimize indexes from measured workload.**
  - Revalidate foreign-key index candidates with `pg_stat_statements` and `EXPLAIN (ANALYZE, BUFFERS)` before adding indexes.
  - Verify usage before removing non-unique indexes duplicated by unique constraints.
  - Use an online rollout procedure appropriate to the migration runner and monitor write amplification and lock time.

- [ ] **DB-003: Tighten long-lived data models.**
  - Replace money, GPA, scores, credits, and other exact decimal values currently represented as floating point with agreed `numeric(p,s)` contracts.
  - Normalize mature admission guardian/address structures while preserving a controlled aggregate boundary and migration compatibility.
  - Add versioned types or validation to stable JSONB payloads and index only demonstrated query paths.
  - Standardize `updated_at`, archival, retention, legal hold, and hard/soft deletion behavior.
  - Plan growth management for audit logs and notifications, including retention and partitioning when measurements justify it.

- [ ] **DB-004: Remove File Platform compatibility columns after the verified rollback window.**
  - Confirm every tenant and consumer uses opaque file IDs, with zero legacy locator reads or writes.
  - Reconcile file metadata and object counts, retain rollback evidence, then add a new forward-only migration that drops the legacy locator columns.
  - Do not edit an applied migration or remove compatibility columns while any deployed version still depends on them.

- [ ] **FE-001: Reduce school frontend data waterfalls and auth flashes.**
  - Move session bootstrap and suitable route-specific loading toward SvelteKit server hooks/layouts/loaders in incremental slices.
  - Keep backend authorization authoritative and add deep-link, expired-session, and denied-route browser tests.
  - Fail closed when required backend configuration is absent instead of using a production fallback.

- [ ] **FE-002: Split oversized frontend workspaces and add component tests.**
  - Decompose timetable, supervision, and subjects pages into focused controller/state, data-access, and presentation units.
  - Replace effect-driven state synchronization with derived state or explicit event transitions where the Svelte analyzer identifies risks.
  - Add unit/component tests for state transitions, permissions, loading, error, empty, and mutation paths.

- [ ] **FE-003: Define frontend performance and browser-support budgets.**
  - Set route and lazy-chunk budgets, measure on CI, and keep heavy PDF/spreadsheet/image libraries user-action loaded.
  - Move the large Thai-school dataset to paginated/searchable delivery or another measured compact representation.
  - Define the supported school-device browser matrix before choosing a JavaScript target.
  - Add CSP, HSTS, frame restrictions, referrer policy, permissions policy, and production guards for debug routes.

- [ ] **CI-001: Deploy only verified immutable artifacts.**
  - Gate deployment on required backend, frontend, contract, static, migration, and smoke checks.
  - Build once, deploy an immutable commit SHA or digest, wait for readiness, and retain an automatic rollback path.
  - Make deploy-all fail when tenant discovery or any required deployment fails.
  - Move menu/permission synchronization and migrations into explicit observable deployment phases.
  - Add staging or canary coverage before all-tenant rollout.

- [ ] **CI-002: Add supply-chain and dependency controls.**
  - Pin third-party actions and mutable package sources to immutable versions.
  - Add Rust advisory scanning, JavaScript audit policy, CodeQL or equivalent static analysis, secret scanning, SBOM generation, and artifact signing.
  - Define how vulnerabilities are triaged, waived with expiry, and verified after upgrades.

- [ ] **OPS-002: Add production observability and repair stale verification tooling.**
  - Propagate request/correlation IDs across frontends, services, provider calls, jobs, and deployment history.
  - Add metrics, traces, dashboards, alerts, SLOs, and runbooks for authentication, tenant resolution, database pools, file access, jobs, and deployments.
  - Update or retire the clean-baseline readiness script whose one-migration assumption no longer matches the active migration timeline.

## P3 — School Product Roadmap

Complete discovery with representative government, private, and international schools before fixing contracts for these domains.

- [ ] **SCH-001: Attendance, leave, lateness, and guardian acknowledgement.**
  - Cover daily and period attendance, corrections, reason evidence, teacher assignment, notifications, audit, reports, and a safe offline/mobile workflow.

- [ ] **SCH-002: Gradebook, results, promotion, and Thai academic documents.**
  - Cover weighted assessment results, grading rules, GPA, incomplete/fail states, promotion, graduation, transcripts, report cards, and ปพ. workflows.

- [ ] **SCH-003: A useful student and guardian portal.**
  - Present authoritative timetable, attendance, results, behavior, announcements, documents, acknowledgement, and communication using scoped access.

- [ ] **SCH-004: Student welfare, behavior, safeguarding, and health.**
  - Model incidents, rewards, interventions, confidential cases, health information, support plans, home visits, ownership, escalation, and retention.

- [ ] **SCH-005: Fees and school finance.**
  - Discover invoices, receipts, scholarships, discounts, refunds, reconciliation, approval, audit, and accounting export requirements before implementation.

- [ ] **SCH-006: HR and operational resources.**
  - Discover leave, attendance, payroll integration, professional development, library, assets, procurement, maintenance, transport, and canteen boundaries.

- [ ] **SCH-007: Learning workflows.**
  - Discover assignments, submissions, rubrics, materials, feedback, announcements, and communication boundaries before deciding whether to build or integrate an LMS.

- [ ] **SCH-008: Thai government data exchange.**
  - Validate DMC, SGS, OBEC CARE, and required education-document schemas with current official specifications and real school exports.
  - Build versioned import/export adapters with validation, reconciliation, permission, PII minimization, and audit rather than coupling external formats directly to core tables.

## Verification Baseline to Restore and Keep Green

- [ ] Backend-admin formatting, check, and all tests pass.
- [ ] Backend-school formatting, check, static architecture tests, and the full test suite pass without timing-dependent retry failures.
- [ ] Frontend-school lint, Svelte check, static tests, permission contract tests, API contract tests, and production build pass.
- [ ] Frontend-admin lint, Svelte check, tests, and production build pass.
- [ ] Fresh PostgreSQL migration tests pass for every active migration without editing applied migration files.
- [ ] JavaScript and Rust dependency advisory checks run in CI with an explicit blocking policy.
- [ ] Authenticated smoke and Playwright flows run against a disposable or staging environment using secrets supplied outside the repository.
