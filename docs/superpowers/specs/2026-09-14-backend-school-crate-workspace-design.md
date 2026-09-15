# Backend School Crate Workspace Foundation

## Status

The design was approved in chat on 2026-09-14. The written specification was approved in chat on
2026-09-15. Checkpoint 1 was implemented and verified on 2026-09-15.

## Problem

`backend-school` is one binary-only Rust package with approximately 213,000 lines across HTTP
composition, tenant infrastructure, permissions, migrations, authentication, file storage,
certificates, academic workflows, and the remaining school domains. A source edit therefore
invalidates one large application compile unit even when most domains are unrelated. The package
also has three binary targets; `migrate_tenant_schema` and `seed_sandbox` currently reuse the
permission and migration implementation through `#[path]`, causing the same source to be compiled
as part of multiple targets instead of through one library artifact.

The current module tree provides useful source organization but does not enforce dependency
direction. Feature code can reach `crate::AppState`, another feature's internals, global utility
modules, and generated permission paths directly. Existing cycles such as academic with students or
supervision, and auth with staff or files, make a one-step feature-crate extraction unsafe.

The repository's deployment measurements also show that source-changing backend-school release
builds are dominated by the final application crate and link. Cargo-chef already preserves external
dependency work, but a single application crate leaves no reusable SchoolOrbit library units.

## Goals

- Establish an internal Cargo workspace without changing the deployed service topology.
- Give backend-school code explicit, acyclic dependency boundaries that future changes must follow.
- Compile shared permission and migration code once for the application and utility binaries.
- Preserve all API, database, permission, session, realtime, security, and deployment behavior.
- Add a durable rule for deciding when future capabilities should become crates rather than modules.
- Measure clean, warm, incremental, and multi-target compilation before and after the split.
- Create a safe foundation for later extraction of File Platform, certificates, auth/tenancy, and
  academic subdomains.

## Non-goals

- Do not split backend-school into independently deployed services or additional containers.
- Do not include `backend-admin` in this workspace.
- Do not change endpoints, DTOs, OpenAPI output, permission codes, grants, database schemas, data,
  migrations, session or realtime behavior, environment variables, ports, or proxy routes.
- Do not extract all feature domains in this first subproject.
- Do not add a catch-all `common`, `shared`, or `utils` crate.
- Do not re-enable compiler caching or change the current final-crate `-C lto=off` release setting
  without separate timing evidence.
- Do not commit Cargo timing output or create a completion report.

## Program Relationship

The complete backend-school split is too large and security-sensitive for one implementation plan.
This specification is limited to the first workspace-foundation checkpoint. The complete target
architecture and ordered checkpoint sequence are governed by
[`2026-09-14-backend-school-crate-architecture-design.md`](./2026-09-14-backend-school-crate-architecture-design.md).

Every checkpoint must keep the application releasable. Timing evidence from this checkpoint is an
input to the next and does not pre-approve a changed boundary.

## Workspace Architecture

The workspace root remains `backend-school/Cargo.toml` and remains a non-virtual workspace root: it
contains both the existing `backend-school` package and the workspace declaration. This preserves
the current Docker build context, `Cargo.lock`, binary name, local working directory, Rust cache
scope, and independent deployment from backend-admin.

The first subproject creates this structure:

```text
backend-school/
├── Cargo.toml
├── Cargo.lock
├── build.rs
├── migrations/
├── src/                         application composition and remaining modules
└── crates/
    ├── school-permissions/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── registry.rs
    │       └── registry_generated.rs
    └── school-migrations/
        ├── Cargo.toml
        ├── build.rs
        └── src/lib.rs
```

External dependency versions and common lint policy live in the workspace manifest. Members opt in
through workspace dependencies and lints while retaining the current dependency features. The root
package remains the default application package and keeps `backend-school` as its default run
target. Existing commands remain valid; workspace-wide verification uses explicit `--workspace`
and `--all-targets` where coverage of every member is required.

Dependency direction is one-way:

```text
backend-school application/composition
    ├── future feature and capability crates
    ├── school-migrations ──> school-permissions
    └── school-permissions
```

Only the application composition layer may assemble the Axum router, complete `AppState`, cron
scheduler, OpenAPI document, and all feature implementations. An internal crate must not depend on
the root application package, use the root package as a path dependency, or accept the complete
`AppState`. Cross-domain use goes through a narrow public type or operation owned by the provider.

## Permission Registry Ownership

`school-permissions` owns `PermissionDef`, the generated Rust registry, `codes`, and
`ALL_PERMISSIONS`. Its public path is `school_permissions::registry`. The existing generator writes
the Rust artifact directly to
`backend-school/crates/school-permissions/src/registry_generated.rs`; the frontend artifact and
contract lock remain in their current locations.

All backend call sites change directly from `crate::permissions::registry` to
`school_permissions::registry`. The old root files and module declaration are removed in the same
change. There is no permanent compatibility re-export, duplicate registry, raw-string replacement,
or manually edited generated file.

The generator tests, cross-stack static tests, permission workflow path filters, API workflow path
filters, `.rules`, and canonical documentation are updated to recognize the new canonical owner.
Permission codes, metadata, ordering, contract digest, lock content, and generated frontend output
must remain byte-for-byte equivalent.

## Tenant Migration Ownership

`school-migrations` owns:

- the active SQLx migrator over `backend-school/migrations/`;
- `run_tenant_migrations`;
- `MigrationTracker`; and
- permission synchronization after migrations.

It depends on `school-permissions`, SQLx, Tokio, DashMap, and tracing. The root PoolManager,
provisioning and migration handlers, test helpers, schema tests, `migrate_tenant_schema`, and
`seed_sandbox` call its public functions directly. The current `src/db/migration.rs` and
`src/utils/permission_sync.rs` owners are removed after their call sites move.

The runtime sequence is unchanged:

```text
PoolManager or explicit migration command
    -> MigrationTracker when per-tenant once-only coordination is required
    -> run every pending active migration from backend-school/migrations/
    -> synchronize school_permissions into the tenant database
```

The root `build.rs` continues tracking `migrations/` because existing root schema tests embed that
timeline. `school-migrations/build.rs` separately tracks `../../migrations` for the crate's embedded
migrator. Relative `migrate!` and `include_str!` paths are adjusted only to preserve the same
canonical files. No SQL migration file, including its comments, is edited.

The two utility binaries replace their current `#[path]` module construction with the same
`school-migrations` dependency used by the application. Production source sharing through `#[path]`
is prohibited after the cutover.

## Error and Behavior Preservation

The first subproject does not redesign `AppError`, HTTP error envelopes, or migration error text.
Moved functions retain their current public results and bounded logging. Database URLs, secrets,
permission data, tenant identifiers, and request contents receive no new logging.

The application remains one process and one image. Its session and permission caches, realtime
channels, file runtime, and certificate limiter remain owned by the existing application state.
There are no transitional runtime reads, dual writes, aliases, feature flags, or database cutover.

## Durable Crate-Admission Rule

`.rules` gains a backend workspace section with these requirements:

- Evaluate crate placement when adding a capability or materially expanding an existing one.
- Create a crate only when it has cohesive ownership, a narrow public interface, acyclic and
  intentionally directed dependencies, meaningful focused tests, and expected compile-scope value.
- Keep small code or code sharing the same owner and lifecycle as a current domain as a module in
  that owner instead of creating a package for naming alone.
- Prefer the smallest number of crates that preserves clear ownership. Do not create catch-all
  shared crates or move unrelated code into a lower layer to resolve a dependency cycle.
- Keep router and complete application-state composition in the application package. Library crates
  consume only the minimum state, data, or interface they require.
- Centralize dependency versions and lints at the workspace root, and verify all affected workspace
  members and targets.
- Support a new boundary with before-and-after compile evidence. A crate split is not justified by
  file count alone and must not weaken runtime, contract, migration, security, or test guarantees.

The existing permission source-of-truth rule is updated to point to the new generated registry.
Backend verification changes from an implicit root-only check to an explicit workspace/all-targets
check while preserving focused tests and the database runner.

## Architecture Enforcement

`backend-school/tests/static_architecture.rs` expands its source inventory from the root `src/` tree
to both `src/` and `crates/`. Durable tests enforce:

- workspace members participate in the shared lint policy;
- internal crates do not depend on the root `backend-school` package;
- only approved one-way path dependencies exist for the first workspace graph;
- generated permission ownership and wrapper shape are canonical;
- production targets do not use `#[path]` to share implementation; and
- existing backend architecture checks continue to cover files after ownership moves.

The dependency allowlist represents durable layers, not a temporary crate count. A future crate is
added only with its reviewed dependency direction and corresponding guard update.

## Compile Measurement

Before source movement, record a baseline on the same branch base, Rust toolchain, machine, Cargo
configuration, and dependency state. Compare at least three runs and use the median for:

- a rebuild of local application packages with external dependencies already warm;
- a no-change warm check;
- a controlled, reversible Rust-only change to migration implementation, never to a SQL migration;
- a multi-target check/test compilation; and
- Cargo timing output plus the local packages Cargo reports as rebuilt or fresh.

Repeat the scenarios after the split. Use task-specific temporary target/output locations and do not
commit timings. The first subproject must show that migration implementation is compiled once in
`school-migrations` and reused by the application and utility binaries, and that changing
`school-migrations` leaves `school-permissions` fresh. A median clean local-package result more than
10 percent slower than its comparable baseline blocks completion until the cause is corrected or
the design is reviewed again.

Later subprojects add domain-specific invalidation gates: a certificate-only change must not rebuild
academic crates, and a results-only change must not rebuild timetable crates. Compiler caching for
school library crates is reconsidered only after at least two ordinary warm CI builds show useful
cacheable work and lower total duration, consistent with the existing operational rule.

## Verification

Focused and matrix verification for the first subproject includes:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test -p school-permissions
cargo test -p school-migrations
cargo test --bin backend-school
cargo test --test static_architecture

cd ..
./scripts/test_backend_school.sh

cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
npm run check:docs

cd ..
podman build -f backend-school/Dockerfile backend-school
podman run --rm -v "$PWD:/repo" -w /repo docker.io/rhysd/actionlint:1.7.7
git diff --check
git status --short
```

The final review also confirms that:

- generated permission codes, digest, lock, and frontend output are unchanged;
- the tracked OpenAPI document is unchanged;
- all three binaries compile against the shared crates;
- cargo-chef builds the workspace and the runtime image still contains the same executable and
  migration directory;
- no applied migration changed; and
- the final diff contains no compatibility owner or unrelated refactor.

## Impact and Rollout

| Area | Effect |
|---|---|
| Backend | Cargo workspace, imports, and permission/migration code ownership change |
| Frontend | No runtime change; static tests follow the generated Rust registry's new path |
| Database | No schema, data, or migration change |
| Permissions | No semantic change; only the Rust artifact owner path changes |
| API/OpenAPI | No endpoint, DTO, response, or generated contract change |
| Session/realtime | No behavior or state-ownership change |
| Security/PDPA | No PII, national-ID, encryption, secret, or logging change |
| Deployment | One binary, port, image, and service; Docker builds the internal workspace |
| Tests | Existing checks remain and crate/dependency-focused checks are added |

The normal coordinated backend-school release remains the rollout path and retains readiness,
all-tenant migration/status audit, authenticated smoke, rollback-tag advancement, and proxy gates.
The deploy workflow already watches `backend-school/**`; narrower permission and API contract
workflow filters are expanded to watch the new crate sources.

If embedding, generation, Cargo, or Docker verification fails, the change does not merge. Because
there is no schema or external contract change, the previous image remains compatible with every
tenant database and is a safe rollback candidate under the existing release process.

After acceptance, compile evidence and boundary quality are reviewed before the next program
checkpoint begins. A disappointing result does not justify combining unrelated domains or weakening
verification; it triggers a revised boundary or build-cache design.
