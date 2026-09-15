# Backend School Crate Architecture Program

## Status

The full-program design and multi-PR execution strategy were approved in chat on 2026-09-14, and
the written specification was approved in chat on 2026-09-15. Checkpoints 1 through 12 were
implemented and locally verified on 2026-09-15 on the same approved program branch. The completed
graph contains the application root and 24 internal crates. Final verification passed 529 workspace
package tests and 738 root database-backed tests, 180 backend static architecture tests, 657
frontend static tests, 12 menu synchronization tests, nine documentation tests, 25 permission
generator tests, and four API generator tests. Rust formatting, workspace/all-target and
deny-warning checks, frontend lint/type/build checks, generators, actionlint, and the production
runtime image build/inspection also passed. Generated permission/API artifacts and both migration
timelines remained byte unchanged from the approved base and across regeneration.

Independent final review findings were closed by making focused database test filters fail on zero
matches, moving HTTP response mapping out of `school-errors`, moving certificate resource-lock
ownership into `school-certificates`, and narrowing the Admission, Assessment, and Gradebook public
facades. The corresponding package, application, script, documentation, and architecture guards
passed after those corrections.

The production image retained one `backend-school` command, embedded migrations, and the non-root
`appuser`. School sccache remains disabled pending two ordinary post-split warm CI samples, as local
compile isolation does not prove remote-cache value. Destructive credential-backed Playwright
execution and deployed-proxy smoke remained unavailable because no dedicated account or isolated
deployed target was present; the previously completed session test discovery remains the bounded
evidence for those external-only gates.

## Purpose

Split the current backend-school binary-only Rust package into an internal Cargo workspace that
improves incremental compilation and enforces durable architecture boundaries while preserving one
deployed API process, one container, and every existing external contract.

This specification governs the complete crate-architecture program. Each checkpoint remains a
coherent, independently reviewed and verified change with its own focused implementation plan. The
workspace foundation checkpoint is specified in
[`2026-09-14-backend-school-crate-workspace-design.md`](./2026-09-14-backend-school-crate-workspace-design.md).

## Current Constraints

The current package contains approximately 213,000 Rust lines. The largest areas are approximately:

| Area | Rust lines |
|---|---:|
| Academic | 101,755 |
| Certificates | 27,575 |
| File Platform | 10,215 |
| Auth | 8,909 |
| Admission | 8,486 |
| Supervision | 8,231 |
| Staff | 5,954 |
| Calendar | 4,389 |

The application package owns three binary targets. Migration and permission source is included
through `#[path]` in two utility binaries. Feature handlers and services commonly depend on root
`AppState`, `AppError`, `ActorContext`, permission constants, and other feature internals.

Current production dependencies include academic with students and supervision, auth with files,
certificates with files and school fonts, parents with academic/calendar/students, and readiness or
workflow orchestration that crosses domain owners. Inside academic, core, delivery, lifecycle,
assessment, gradebook, learner evaluation, results, timetable, and legacy service groupings contain
multiple reverse dependencies. Moving current folders directly into packages would therefore create
cycles or an oversized catch-all crate.

## Goals

- Make the root application a composition boundary rather than the owner of all business logic.
- Make dependency direction explicit and acyclic in Cargo manifests and public Rust interfaces.
- Reduce the amount of SchoolOrbit source recompiled after a domain-local edit.
- Allow focused `cargo check` and `cargo test` by capability or domain.
- Reuse stable library artifacts across the API and utility binaries.
- Keep shared crates small, cohesive, stable, and intentionally low in the dependency graph.
- Complete the foundation, platform, auth, academic, eligible satellite-domain, and build-tuning
  checkpoints rather than stopping after the first extraction.
- Preserve current database, authorization, API, realtime, security, and deployment guarantees.

## Non-goals

- Do not create microservices, additional listeners, queues, containers, databases, or network
  calls between internal domains.
- Do not include backend-admin in the backend-school workspace.
- Do not change product behavior, routes, response shapes, OpenAPI, permission codes or grants,
  migrations, tenant data, environment contracts, session semantics, or realtime semantics.
- Do not force every current module or future feature into a crate.
- Do not create generic `common`, `shared`, `models`, or `utils` packages.
- Do not weaken tests, release gates, security checks, migration status, or smoke coverage to make a
  checkpoint pass.
- Do not promise clean-build or CI improvement without timing evidence. Final linking remains
  necessary even when library compilation is reused.

## Target Architecture

`backend-school/Cargo.toml` is the non-virtual workspace root and application package. The repository
root does not become a Rust workspace. The deployed executable remains `backend-school`.

```text
backend-school application/composition
├── startup, configuration, router, full AppState, schedulers
├── OpenAPI composition
├── HTTP/session middleware adapters
├── cross-domain orchestration adapters
│
├── school-permissions
├── school-migrations ─────────────> school-permissions
├── school-errors
├── school-http ───────────────────> school-errors
├── school-authorization ──────────> school-errors + school-permissions
├── school-tenancy ────────────────> school-migrations
├── school-auth ───────────────────> tenancy + authorization + errors
├── school-file-platform ──────────> errors + authorization where required
├── school-fonts ──────────────────> file-platform + errors + authorization
├── school-certificates ───────────> fonts + file-platform + errors + authorization
├── academic crates
└── eligible satellite-domain crates

school-test-db (dev dependency only) -> school-migrations
```

The root application is the only package allowed to depend on every feature. Internal packages
must not depend on the root package, construct the complete `AppState`, or reach another package's
private source path. Cross-domain behavior uses a public operation, typed data contract, or narrow
port implemented at the composition layer.

The root `src/` tree may retain HTTP handlers and integration orchestration when keeping them in the
application avoids a false feature dependency. A domain crate owns its models, validation,
repositories, services, policies, and focused tests. Once a feature has a genuinely independent
state, its router adapter may move with it; the feature receives that state rather than `AppState`.

## Stable Foundation Crates

### `school-permissions`

Owns `PermissionDef`, generated permission constants, and `ALL_PERMISSIONS`. It has no dependency on
application or feature crates. The cross-stack generator writes its Rust artifact directly into
this crate.

### `school-migrations`

Owns the active SQLx migrator, `MigrationTracker`, `run_tenant_migrations`, and post-migration
permission synchronization. It embeds the unchanged canonical `backend-school/migrations/`
timeline and depends on `school-permissions`.

### `school-errors`

Owns the application/service error taxonomy without Axum response construction and without any
feature model dependency. Domain services return this error or a domain-specific error that has an
explicit conversion at the application boundary.

The current certificate-specific structured conflict is removed from the shared dependency
direction without changing its wire response: certificate code owns the domain error and the HTTP
adapter maps it to the same status, error text, `code`, and optional request ID.

### `school-http`

Owns `ApiResponse`, error envelopes, empty/ID response types, the handler-facing HTTP error wrapper,
and `IntoResponse` mapping. Its local wrapper contains `school_errors::AppError`, which permits a
legal Axum `IntoResponse` implementation without coupling the domain error crate to Axum.

Handlers return the HTTP wrapper while `From<school_errors::AppError>` preserves `?` propagation.
Response status, headers, body shape, public messages, logging redaction, and OpenAPI schemas remain
unchanged.

### `school-authorization`

Owns `ActorContext`, exact/wildcard/module permission matching, permission requirements, effective
permission loading, and the permission cache. It depends on `school-permissions` and
`school-errors`, not on auth or application state.

The current permission cache must stop owning `SessionCache`. The application/auth composition owns
both caches and performs synchronous invalidation of each before returning or beginning fallible
follow-up work. This preserves the existing stale-fill and identity-invalidation requirements while
removing the authorization-to-auth reverse dependency.

### `school-test-db`

This dev-only package owns PostgreSQL test-database creation, isolated schema/pool setup, and running
the canonical migration crate. It never appears in a normal dependency or runtime image. Domain
fixtures and business assertions remain with their owning crates; this package must not become a
general test-helper dumping ground.

## Tenant and Authentication Boundaries

### `school-tenancy`

Owns the backend-admin internal client, bounded retry configuration, school database metadata,
tenant pool cache, pool creation, and school mapping. It depends on `school-migrations` for lazy
tenant migration coordination. Request origin/subdomain resolution remains an application HTTP
adapter because it combines headers, sessions, and the complete runtime.

The crate never exposes or logs database URLs except as the existing opaque value required to
construct a pool. Public errors retain bounded messages. Pool caching, expiration, connection
configuration, statement-cache behavior, and migration-on-first-access behavior remain unchanged.

### `school-auth`

Owns session configuration, crypto, policy, repositories, session cache, throttle repository,
authenticated-session types, auth runtime, events, and service logic. HTTP cookie parsing/writing,
CSRF enforcement, and route adapters may live in the application while they depend on Axum and
global middleware ordering.

Auth no longer calls staff or File Platform internals directly. Account/profile mutations return a
typed outcome describing required side effects; the application orchestrator invokes staff or file
operations after authorization. Side-effect ordering and synchronous cache invalidation remain
identical. No raw credential, previous-token acceptance, national ID, or secret enters a cache or
log.

Auth extraction triggers the complete session/realtime verification required by `.rules`, including
focused cache/rotation/revocation tests, frontend auth static tests, Playwright discovery or
execution with a disposable account, and deployed-proxy smoke when runtime credentials are
available.

## File, Font, and Certificate Boundaries

### `school-file-platform`

Owns provider-neutral file types, purpose registry, storage provider traits and R2 implementation,
malware scanner, runtime configuration, repositories, inspection, lifecycle/reconciliation, and
file-domain policies and services. It accepts tenant pools, actor context, and explicit runtime
dependencies rather than the complete application state.

HTTP upload/download handlers, tenant request resolution, event publication, and scheduled
all-tenant iteration remain application adapters until they can bind a narrow file state. Object
keys, private grants, provider errors, and scan details retain their current secrecy boundaries.

### `school-fonts`

Owns school-font models, validation, inspection, persistence, deletion-conflict outcomes, and tests.
It uses only the File Platform public interface. Certificate references are checked through a
narrow repository operation or typed port rather than by importing certificate internals.

### `school-certificates`

Owns certificate models, layout, templates, campaigns, candidates, requests, issuance, rendering,
verification, purge lifecycle, policies, rate-limiting logic, and focused/schema tests. It depends on
File Platform and font public interfaces. Public verification handlers and complete application
state remain at the HTTP boundary as needed.

The structured resource-lock conflict is owned by certificates and mapped at the HTTP boundary.
Its response remains byte-for-byte contract compatible. Permanent purge behavior, file lifecycle,
and public verification security headers remain unchanged.

## Academic Boundaries

Academic extraction corrects ownership before moving folders. Current folder names are inputs, not
automatic package boundaries.

```text
school-academic-core
├── school-academic-delivery ─────> school-academic-timetable
├── school-academic-assessment ───> school-academic-results
└── all required public operations ───────────────> school-academic-lifecycle
```

Arrows mean “is depended on by the item to the right”:

```text
core -> delivery -> timetable
core -> assessment -> results
core + delivery + assessment + results -> lifecycle
```

No lower academic crate depends on lifecycle. Lifecycle is the top-level orchestrator for opening,
closing, promotion, reopening, preparation, readiness, and cross-area transition workflows.

### `school-academic-core`

Owns canonical academic contexts, years, terms, curricula/catalogs, grade/program structures,
stable IDs/status values, and state guards intrinsic to those records. Core does not import
delivery, assessment, results, lifecycle, supervision, students, calendar, question bank, or
notification implementations.

### `school-academic-delivery`

Owns offerings, groups, rosters, teaching assignments, delivery workspaces, and delivery-specific
lifecycle checks. It depends on core public types and operations.

### `school-academic-timetable`

Owns timetable models, versions, scheduling services, templates, conflicts, publication, and
realtime-domain messages. It depends on core and delivery. Axum WebSocket connection state and
handshake adapters remain in the application until represented by a narrow timetable runtime.

### `school-academic-assessment`

Owns assessment configuration, phases, grading inputs, gradebook, learner evaluation, locks, and
corrections at the assessment boundary. It depends on core and uses explicit result-facing
contracts rather than importing result implementation.

### `school-academic-results`

Owns aggregates, readiness summaries, revisions, annual snapshots, result locking/publication, and
result correction consequences. It depends on core and assessment public operations and does not
depend on lifecycle.

### `school-academic-lifecycle`

Owns the top-level year/term preparation, activation, closure, promotion, reopening, impact review,
and transition orchestration. It may depend on every lower academic crate because nothing lower
depends back on it.

Supervision, calendar, student enrollment, question bank, and notification readiness or side effects
are expressed as typed provider/command ports. The application supplies implementations. A failed
provider remains a typed failed/not-ready outcome according to current behavior; it is not silently
ignored.

## Satellite Domains

After the foundation and academic dependency directions are stable, evaluate the remaining modules
with the crate-admission rule. Expected candidates are admission, supervision, staff/organization,
calendar, question bank, and work/workflow. Students and parents are evaluated together with their
ownership and academic/calendar dependencies; they are not combined merely to eliminate imports.

Small modules such as school profile, facility, consent, achievement, lookup, menu, notification,
and system remain application-owned unless direct analysis shows cohesive independent behavior,
meaningful focused tests, and compile-scope value. “Complete” means every module has an explicit
owner and every worthwhile boundary is enforced, not that every folder becomes a Cargo package.

Cross-domain dependencies must point from consumers to providers. Reverse readiness or side-effect
calls move into application orchestration or typed ports; unrelated code is never moved into a
lower package merely to satisfy Cargo.

## Checkpoint Sequence

The program proceeds through PR-sized checkpoints. Each checkpoint starts from the accepted prior
tree, has its own focused plan, and remains buildable and releasable:

1. workspace foundation, `school-permissions`, `school-migrations`, workspace lints/dependencies,
   generated paths, `.rules`, tests, and baseline timing;
2. `school-errors`, `school-http`, `school-authorization`, and dev-only `school-test-db`;
3. `school-tenancy` and tenant pool/migration consumers;
4. `school-file-platform` and its application adapters;
5. `school-fonts` and `school-certificates`;
6. `school-auth` plus composed cache invalidation and cross-domain auth side effects;
7. academic core ownership and removal of lower-to-lifecycle dependencies;
8. academic delivery and timetable;
9. academic assessment and results;
10. academic lifecycle orchestration and external readiness ports;
11. eligible satellite domains, one coherent owner group per checkpoint;
12. final composition cleanup, dependency guard consolidation, compile/CI measurement, and
    evidence-based compiler-cache tuning.

A checkpoint may be split further when implementation analysis exposes an independent high-risk
boundary. It may not silently absorb the next checkpoint or unrelated feature work. Checkpoint
commits may be used on a feature branch; accepted changes are integrated according to the
repository's squash-merge and exact-tree verification rules.

## Crate-Admission and Completion Rules

`.rules` is updated in checkpoint 1 so future backend work must evaluate crate placement. A crate is
admitted only when it has:

- one cohesive owner and lifecycle;
- a public interface materially narrower than its implementation;
- acyclic, intentional dependency direction;
- meaningful focused tests;
- no root `AppState` or root-package dependency;
- expected compile-scope benefit supported by before/after evidence; and
- no weakening of API, migration, permission, security, deployment, or test invariants.

Small additions that belong to an existing owner remain modules. Shared packages must have a named
purpose; generic catch-all packages are prohibited. Dependency versions and lints are centralized at
the backend-school workspace root.

The full program is complete only when:

- the root application contains composition, adapters, and cross-domain orchestration rather than
  feature SQL or business rules;
- internal Cargo dependencies are acyclic and guarded;
- internal crates do not accept full `AppState` or depend on the root package;
- production source sharing contains no `#[path]` workaround;
- permission and migration ownership is singular;
- each extracted crate has focused tests and explicit public interfaces;
- every remaining root module has been evaluated against the crate-admission rule;
- representative domain edits rebuild only the changed crate and its actual dependents;
- clean and warm compile measurements meet the checkpoint gates;
- one executable/container/API and all runtime invariants remain intact; and
- no temporary compatibility alias, duplicate owner, unused port, or migration scaffold remains.

## Compile and CI Strategy

Checkpoint 1 records three-run median baselines on one machine/toolchain for warm dependencies,
no-change checks, controlled Rust source changes, multi-target builds, and Cargo timing units. Every
later extraction adds a representative edit for its new boundary.

Checkpoint 2 used Rust/Cargo 1.98.1 and the checkpoint-1 target directory. A comment-only
`school-errors` edit produced 16.375, 13.085, and 13.115 second workspace checks (13.115 second
median). A comment-only `school-authorization` edit produced 13.332, 12.997, and 12.850 second
workspace checks (12.997 second median). These medians are respectively 77.9 and 78.1 percent below
the 59.445 second checkpoint-1 local-package baseline. Cargo marked only each changed package and
its actual dependents dirty; unrelated sibling packages remained fresh. Measurement probes were
removed after the runs.

Checkpoint 3 used the same toolchain and target directory. A comment-only `school-tenancy` edit
produced 14.964, 13.234, and 13.263 second workspace checks (13.263 second median), 77.7 percent
below the 59.445 second checkpoint-1 local-package baseline. Cargo rebuilt `school-tenancy` and the
application root while keeping the foundation sibling crates fresh. The probe was removed after the
runs.

Checkpoint 4 used the same toolchain and target directory. After one unmeasured cache warm-up, a
comment-only `school-file-platform` edit produced 13.529, 12.829, and 12.888 second workspace checks
(12.888 second median), 78.3 percent below the 59.445 second checkpoint-1 local-package baseline.
Cargo rebuilt `school-file-platform` and the application root while keeping the foundation and
tenancy sibling crates fresh. The probe and its temporary target directory were removed after the
runs.

Checkpoint 5 used the same toolchain and the restored main target directory after removing only
disposable build artifacts during disk-pressure recovery. A comment-only `school-fonts` edit
produced 14.001, 13.008, and 13.036 second workspace checks (13.036 second median), 78.1 percent
below the 59.445 second checkpoint-1 local-package baseline. A comment-only
`school-certificates` edit after one unmeasured warm-up produced 12.871, 12.892, and 12.836 second
workspace checks (12.871 second median), 78.3 percent below baseline. Cargo rebuilt the changed
crate and its actual consumers while keeping unrelated workspace crates fresh. Both probes were
removed after measurement.

Checkpoint 6 used the same toolchain, machine, Cargo configuration, dependency state, and main
target directory. A comment-only `school-auth` edit produced 14.590, 11.901, and 11.743 second
workspace checks (11.901 second median), 80.0 percent below the 59.445 second checkpoint-1
local-package baseline. Cargo marked `school-auth` and its actual application consumers dirty while
lower foundation and sibling feature crates remained fresh. The probe was removed after the runs.

Checkpoints 7 through 10 used Rust/Cargo 1.98.1, the same machine, Cargo configuration, dependency
state, and main target directory. Representative comment-only workspace checks produced these
three-run medians against the 59.445-second baseline: Academic Core 10.173 seconds (82.9 percent
lower), Delivery 9.069 seconds (84.7 percent lower), Timetable 8.339 seconds (86.0 percent lower),
Assessment 8.998 seconds (84.9 percent lower), Results 8.459 seconds (85.8 percent lower), and
Lifecycle 8.304 seconds (86.0 percent lower). Cargo rebuilt each edited crate and only its actual
consumers; in particular, Results and Timetable remained independent siblings. Every probe was
removed after measurement.

Checkpoint 11 used the same environment and measured every admitted satellite owner. Workflow ran
6.334, 5.707, and 5.702 seconds (5.707-second median; 90.4 percent lower); Question Bank ran 6.129,
5.798, and 5.798 seconds (5.798-second median; 90.2 percent lower); Admission ran 6.661, 6.610, and
6.632 seconds (6.632-second median; 88.8 percent lower); Supervision ran 6.710, 6.548, and 6.719
seconds (6.710-second median; 88.7 percent lower); Students ran 6.599, 6.547, and 6.523 seconds
(6.547-second median; 89.0 percent lower); Staff ran 6.809, 6.480, and 6.266 seconds (6.480-second
median; 89.1 percent lower); and Calendar ran 6.362, 6.089, and 6.275 seconds (6.275-second median;
89.4 percent lower). Reverted sources were warmed between owner probes. Cargo rebuilt only each
edited crate and its actual root or domain consumers; unrelated packages stayed fresh. Every probe
was removed.

Checkpoint 12 measured the completed graph after cleaning only SchoolOrbit workspace packages and
retaining third-party artifacts. Three `cargo check --workspace --all-targets` runs took 46.650,
49.438, and 44.581 seconds (46.650-second median), 21.5 percent below the checkpoint-1 comparable
local-package baseline. Final no-change warm checks took 0.325, 0.323, and 0.336 seconds
(0.325-second median). A final Core probe took 10.818, 10.173, and 9.582 seconds and confirmed the
expected Core-dependent academic and satellite subset without rebuilding unrelated owners. Cargo
timings were emitted outside the repository, and all measurement probes were removed.

At minimum, the completed graph must demonstrate:

- migration edits compile `school-migrations` once and reuse it across binaries;
- certificate edits do not rebuild academic crates;
- File Platform implementation edits rebuild only its actual consumers;
- auth implementation edits do not rebuild unrelated academic service crates;
- results edits do not rebuild timetable; and
- timetable edits do not rebuild results.

Each checkpoint blocks when its comparable median clean local-package build is more than 10 percent
slower without a corrected cause or a reviewed design change. Incremental invalidation is verified
from Cargo's rebuilt/fresh package evidence, not wall time alone.

The production Dockerfile retains cargo-chef, the pinned toolchain, timing artifact, and final
application `-C lto=off` while package boundaries are changing. School compiler caching is evaluated
only after stable library crates exist. It is enabled only if at least two ordinary warm CI builds
show useful cache hits and lower end-to-end build duration; otherwise plain Cargo remains canonical.
The completed local graph establishes compile isolation but cannot establish remote-cache value.
The only two available source-changing CI samples predate the workspace and reported zero useful
hits, so checkpoint 12 keeps school sccache disabled. Two ordinary post-split warm CI builds remain
the evidence gate for any later change; benchmark-only deployments are not permitted.

## Contract, Data, and Security Preservation

No checkpoint changes an applied migration. Internal ownership moves keep
`backend-school/migrations/` as the only active tenant timeline and the centralized migration runner
as the only production path.

Rust DTOs and the root OpenAPI composition remain the HTTP contract owner. Moving a DTO between
packages does not authorize a wire change. Every checkpoint checks the tracked OpenAPI output and
generated frontend types. Permission changes are not part of this program; moving the generated
Rust registry preserves the contract digest, lock, codes, and frontend registry exactly.

National IDs remain application-encrypted and blind-indexed. No crate receives broader PII than its
owner requires, and generic lookup/public interfaces remain minimized. Logging remains bounded and
must not expose national IDs, credentials, tokens, cookies, database URLs, encryption keys, object
keys, signed grants, raw request bodies, or provider payloads.

Session and permission cache invalidation remains synchronous before fallible follow-up work.
Realtime events remain invalidation signals, authenticate server-side, and keep heartbeat and
reconciliation behavior. Multiple replicas remain outside scope and still require a reviewed shared
invalidation design.

## Verification Strategy

Every checkpoint runs focused tests for moved logic and the applicable `.rules` matrix. The backend
baseline becomes workspace-aware:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --test static_architecture
```

Package tests use `cargo test -p <package>`. Root integration tests continue to exercise application
composition. Database-backed packages use `scripts/test_backend_school.sh` and `school-test-db`
through the rootless Podman test database. No normal test connects to a production or pooled Neon
endpoint.

Permission artifact moves run the complete permission generator/check/test set. Any DTO/OpenAPI
ownership move runs the complete API generator/check/test set. Frontend static checks run whenever
their contract tests or generated paths change. Auth and realtime checkpoints run the additional
session, browser, and deployed-proxy verification required by `.rules`. Docker/cargo-chef is rebuilt
whenever manifests, members, binary composition, build scripts, or migration embedding changes.

Architecture tests scan root and workspace source and enforce allowed path dependencies, workspace
lints, generated owners, no production `#[path]`, no internal dependency on the root package, no
complete `AppState` use in libraries, and the final academic dependency direction. Guards describe
durable boundaries rather than temporary file counts.

Every checkpoint ends with `git diff --check`, final diff review, `git status --short`, and exact-tree
verification appropriate to its integration. Missing credentials or an unavailable external gate
is reported explicitly and is not replaced by a weaker check.

## Rollout and Recovery

All checkpoints retain the single coordinated backend-school release. Readiness, all-tenant
migration/status audit, permission/menu synchronization when applicable, authenticated smoke,
rollback-tag advancement, and proxy restoration remain in their existing order.

Because the program changes no schema or external runtime contract, the accepted prior image remains
database-compatible throughout. Before each integration, confirm that no incidental contract or
migration diff entered the checkpoint. A failure before merge is corrected on the checkpoint branch;
a failed deployment follows the existing coordinated release recovery and can use the accepted prior
image subject to its normal evidence gates.

No temporary dual path survives a checkpoint. When a source owner moves, all consumers and tests
move to the canonical public path and the old owner is removed in the same accepted tree.
