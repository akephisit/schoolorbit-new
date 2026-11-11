# Auth/File Dependency Boundary and Codegen Experiment

## Scope and starting point

The user authorized two changes: compare `school-staff` package `codegen-units` at its current
default of 16 with 32 and 64, and remove the production dependency from `school-auth-http` to
`school-file-platform`. Only Staff's package profile may vary; root binary and external-crate CGU,
optimization level 3 and the root binary's existing `lto=off` remain fixed. A compiler-profile winner
is not assumed before measurement. A further transport extraction or another package knob is
outside this change.

The coordinated checkout is `/workspace/schoolorbit-new`, branch
`perf/auth-file-boundary-codegen-units`, starting at commit
`d705f6ead9ee03b26086efe6420d7d8f1933eb17`, tree
`d5843306cd14525c09f63707686b851458acc022`. Only the immutable baseline source archive must finish
before Rust implementation changes. Once the Coordinator acknowledges that archive and the
design, live implementation may proceed while Pipeline primes and benchmarks its separate
baseline archive. Previous compiler-audit, opt-level and build-scope experiment directories are
immutable evidence, not working directories for this experiment.

This design records the authorized direction. The Coordinator and independent Reviewer assess the
concrete port and transaction design before the Backend writer edits Rust. This is an internal
coordination dependency, not an additional request for user authorization.

## Current ownership and cause

`school-auth-http` is the single cookie, origin, tenant and authenticated request-context adapter.
Numerous domain HTTP crates depend on it for that context. Its
`http/profile_orchestrator.rs` additionally imports `FilePlatform`, Files' profile relationship
provider and Files' deletion helper. `AuthHttpState` carries `Arc<FilePlatform>`, and its manifest
both depends on Files and forwards Files' `api-docs` feature. Consequently a Files implementation
edit invalidates the shared Auth HTTP dependency and its transport consumers.

`school-auth::services::update_profile` already owns profile validation, the user-row lock and the
user mutation. `school-file-platform::profile_relationships::promote_ready_owned_image` owns the
ready/owner/purpose check, file-row lock and retention promotion on a borrowed connection. Files'
`consumer_service::request_deletions` owns post-commit lifecycle deletion. The orchestration of
these two providers belongs to the application, as other cross-domain adapters do.

## Chosen boundary

Add the following narrow, object-safe command port in
`school-auth-http/src/http/state.rs`, using the existing workspace `async-trait` dependency:

```rust
#[async_trait]
pub trait ProfileUpdatePort: Send + Sync {
    async fn update_profile(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        payload: UpdateProfileRequest,
    ) -> Result<User, AppError>;
}
```

`AuthHttpState` retains its existing `AuthRuntime` and replaces `Arc<FilePlatform>` with
`Arc<dyn ProfileUpdatePort>`. Its `FromRef<AuthHttpState> for AuthRuntime` stays unchanged. This
port describes the complete command required by the profile route; it exposes no Files types,
storage locator, session credential, full application state or alternate tenant resolver.

`src/adapters/auth_profile.rs` owns `ApplicationAuthProfile`, which stores the existing
`Arc<FilePlatform>` and implements the port. The application-owned
`FromRef<AppState> for AuthHttpState` constructs that adapter from the same File Platform instance.
The handler supplies the pool and user ID from its existing
`current_user_tenant_context_from_session` result, invokes the injected command, and retains its
existing primary-role lookup and `ProfileResponse` conversion.

Move the existing orchestration and relationship error mapper into the adapter and remove the old
`profile_orchestrator` module and file. Remove `school-file-platform` from Auth HTTP's production
dependencies and `api-docs` forwarding. Add `async-trait` through the existing workspace version;
regenerate the lockfile through the sole assigned Cargo owner without updating external versions.
Root application Files dependencies and OpenAPI feature forwarding remain because their consumers
still require them. Domain providers keep their existing SQL, locks, validation and public APIs.

No new crate meets the admission criteria: this is one small application composition adapter with
one HTTP consumer and the same lifecycle as the existing application. Putting orchestration in
`school-auth`, moving the request-context owner, or adding a facade would retain coupling or add an
unnecessary authority. Existing application adapter/port conventions supply the needed boundary.

## Behavior that must remain exact

1. The shared router middleware authenticates the session and enforces tenant Origin/CSRF before
   the handler. The route, request DTO, response envelope, OpenAPI path and operation ID do not
   change. The actor and tenant pool come only from that session.
2. Begin one observed transaction on the caller's tenant pool. Preserve the existing static label
   `auth-http.profile_orchestrator.update_profile` so this ownership move does not change
   observation identity.
3. For `Some(Some(file_id))`, call Files' promotion provider on the same transaction before calling
   Auth's profile service. Preserve the provider's file-row lock, then Auth's user-row lock; add no
   pool checkout, retry, transaction split or SQL copy inside that sequence.
4. Omitted image input preserves the current relationship; explicit null clears it; the same image
   produces no replacement deletion. Existing title/name/date and profile-field semantics remain
   Auth-owned.
5. Map `InvalidFile` to exactly
   `AppError::ValidationError("ไฟล์รูปโปรไฟล์ไม่พร้อมใช้งาน".to_string())`; map a relationship database
   error to `AppError::DbError` with the original error. Propagate every other error unchanged.
6. Commit promotion and user mutation together. Any pre-commit error rolls both back and performs
   no replacement cleanup. No external provider call occurs while that transaction is open.
7. After commit, read the user through `services::find_user_by_id` using the same caller pool.
   Only after that successful read, request deletion for the replaced file through the unchanged
   Files helper. Return the same user after cleanup succeeds. A post-commit read or cleanup error
   remains an error after the mutation has committed; do not swallow it or manufacture rollback.
   Storage failures retain Files' existing durable retry behavior.

There are no migrations, data transformations, permissions, API contract changes, cache-policy
changes, deployment topology changes or new secrets. The existing single-process session owner
and all tenant/session identity types remain shared.

## Consumers and guards

The application projects Auth HTTP state in `src/http_state.rs`; its four Auth/Consent mounts in
`src/app.rs` require no route change. No other production constructor of `AuthHttpState` was found.
The root application fixture in `session_http_tests.rs` constructs `AppState`, so the production
projection remains exercised without adding a second identity path.

`tests/static_architecture.rs` owns the approved package graph and currently names the old profile
orchestrator for lifecycle cleanup. Update it to assert the new application owner, the absence of
Files types/dependencies/feature forwarding in Auth HTTP, the narrow injected state and continued
use of Files' relationship/deletion providers. Keep the existing lifecycle guard by changing its
owner path, not removing its requirement. Frontend static contracts reference the unchanged route
and DTOs; no frontend path relocation has been found necessary.

The existing application `profile_integration_tests` manually composes the lower providers. It
does not execute the old production orchestrator. Strengthen it to invoke the new production
adapter through `ProfileUpdatePort`, retaining the shared image fixture used by Timetable tests.

## Verification and evidence

Backend checks cover the affected packages and application consumers in both default and
`api-docs` configurations, the full static architecture target, profile/provider database cases,
and production session/tenant/CSRF transport fixtures. Keep OpenAPI bytes and permission/migration
artifacts unchanged. Run the relevant frontend auth/session, file and API contract guards plus
documentation policy. Exact owning commands and focused test cases are in the companion plan and
canonical `docs/TESTING.md`; the Coordinator assigns the sole Cargo owner and final QA scope.

Database cases must exercise the production adapter for successful promotion/profile mutation,
omitted/null/same/replaced images, foreign/not-ready/wrong-purpose rejection, rollback after
promotion when Auth rejects, post-commit cleanup and surfaced cleanup failure with the committed
profile preserved. A production-router case verifies missing session, missing/wrong CSRF and
cross-tenant refusal before mutation, and accepted mutation in the selected tenant only.

The Pipeline owner measures immutable baseline and candidate sources on the same machine,
toolchain, Cargo configuration, canonical source path, target and dependencies, sequentially. The
Staff-only CGU matrix has six timed public-constant Staff source probes in order
`16, 32, 64, 64, 32, 16`. Configuration primes and pristine resets are excluded from the timed
samples. The actual production rebuild closure is Staff, Files HTTP and the root application;
record its dirty/fresh packages and complete Cargo wall time, including downstream compile/link
work. Keep root binary and external-crate CGU, optimization level 3 and root `lto=off` fixed.

Adoption requires a repeatable improvement of at least 5% in Cargo wall time and actual production
runtime/output equivalence for each representative workload. Runtime evidence uses twelve
interleaved controls and bootstrap confidence intervals. A 3% screening threshold is not automatic
acceptance. Retain default 16 if results are uncertain or any measured workload regresses; neither
a package compile-time improvement nor synthetic output alone establishes production equivalence.

Measure the Auth boundary separately at Staff CGU 16 on both sources: two baseline Files source
probes followed by two candidate Files source probes on the same canonical path, excluding primes.
Report the stage-order caveat instead of claiming a counterbalanced comparison. Complete this
candidate boundary measurement before deciding or adopting any Staff profile override, so the
boundary result cannot be confounded by the profile. Capture wall time, dirty/fresh packages,
compile/link evidence, resource/disk constraints and actual sample order. A removed edge proves
reduced invalidation scope; timing claims require the measurements. All unchanged API/runtime/build
gates remain required.

## Owners, cutover and rollback

The Coordinator owns Git, checkpoints, candidate freezes, PR/integration and any later production
actions. Backend owns the port, adapter, state/handler cutover, focused tests, backend rules/docs,
these workflow inputs and any accepted Staff-only package profile override in `Cargo.toml`.
Pipeline owns compiler experiments, build/release scripts and workflow
files, `docs/OPERATIONS.md`, and all Cargo/lock/API generation while source is frozen. Independent
Reviewer and QA assess the frozen integrated candidate separately.

Cut over all state/handler consumers in one source change and retire the old orchestrator; no dual
runtime path or persisted reconciliation is needed. Rollback is a coherent revert of port,
adapter, state, dependency/lock and guard changes. If a Staff compiler profile is selected, Backend
sets only `codegen-units` under `[profile.release.package.school-staff]` in `Cargo.toml`; rollback
removes that package override, restoring default 16. Root compiler flags and other package profiles
remain fixed. Neither rollback changes tenant data or session keys. Deployment uses existing maintenance,
readiness and authenticated acceptance gates; local compiler evidence is not production latency
evidence or release acceptance.
