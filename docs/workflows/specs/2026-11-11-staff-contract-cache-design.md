# Staff boundaries, File contracts, and release cache design

## Status and approved scope

Candidate workflow input for Senior and independent Reviewer approval. The user approved all five
work areas: extract Staff HTTP; split genuine large Staff subdomains; separate narrow File
contracts from implementation; investigate a persistent compiler cache/builder on the same CPU
allocation; and remove justified release setup overhead. That scope does not authorize behavior,
API, permission, migration, or production-gate changes. Runtime implementation starts only after
the Senior accepts this design and its [implementation plan](../plans/2026-11-11-staff-contract-cache.md).

The branch is `perf/staff-contract-cache-pipeline`, based on
`431ef887bc94f608df6684b8a1f777a570c81a40` (tree
`0af566f919b5986eb7f14fb2c5092101d9d1a3b0`). The accepted base has completed full local QA and
the main production release. Those results establish the starting point, not verification of this
candidate. Pipeline's detailed method and setup proposal still require Senior/Reviewer reconciliation;
the user has selected the existing GitHub runner and a cross-run cache trial.

These two files are temporary workflow inputs under [the documentation policy](../../../.rules).
Durable architecture belongs in `.rules` and `backend-school/README.md`, test recipes in
`docs/TESTING.md`, and operational/cache procedures in `docs/OPERATIONS.md`. Record completed
evidence in the PR and remove the workflow inputs after the implementation PR records the outcome.

## Working agreement and invariants

The application remains one backend process and image. It owns startup, full `AppState`, `FromRef`
projections, global prefixes, middleware, OpenAPI composition, schedulers, rollout/profile cutover,
and cross-domain adapters. Providers keep authoritative SQL and typed operations; consumers call
those operations on the existing caller connection where an atomic transaction already exists.
Transport uses canonical `school-auth-http` context and the existing shared identity/permission
caches. Domain crates never import their HTTP owner or the executable.

Preserve all business decisions, SQL/date semantics, lock/transaction ordering, permission and
denial behavior, response/error/schema names, session/tenant boundaries, notification ordering,
file lifecycle rules, async disposal and WebRTC protocol. Keep both migration timelines,
permission artifacts, OpenAPI and generated TypeScript byte-identical. Do not hand-patch generated
artifacts. No new service, database, relay, persistent FileTransfer state, dependency upgrade, paid
runner, host, or credential assumption belongs to this change.

Production compiler settings remain optimization level 3, 16 codegen units and the existing
root-only local ThinLTO disablement. Preserve the 531 external dependency identities, pinned
toolchain and existing native/compiler settings. Compare actual Cargo artifact feature/configuration
sets for external crates in normal release and fresh API builds as well as their lock tuples.
Previous 32/64 codegen-unit trials were rejected;
this work does not reopen them. More crates are a hypothesis about invalidation scope, not proof
of a faster build.

## Current ownership and source evidence

`school-staff` currently combines personnel/career/organization/role services, Achievement, all
Staff transport, Leave, and FileTransfer. Its production dependency list therefore includes
Workflow, Notifications, Auth HTTP, HTTP and File Platform. Exact sources and callers are:

| Boundary | Current source | Consumers and required treatment |
| --- | --- | --- |
| Staff/Achievement HTTP | `crates/school-staff/src/http*`, `achievement/http*` | Root `src/app.rs`, `src/http_state.rs`, `src/api_contract.rs`, permission coverage and Staff/Academic fixtures move to the transport owner. |
| Leave domain | `crates/school-staff/src/leave/` except `http.rs` | Root API schemas and Files HTTP use the new Leave owner; `tests/leave.rs` moves with the domain. |
| Staff name/career queries | `leave/repository.rs` functions `staff_name` and `category`; `file_transfer.rs::staff_display_name` | Move the SQL to Staff public providers. Leave keeps its denial mapping and same transaction; FileTransfer HTTP keeps its active-account mapping and observed checkout. |
| FileTransfer domain | `file_transfer/models.rs`, `runtime.rs`, `tests.rs` | Root owns the sole `Arc<FileTransferRuntime>`; HTTP consumes runtime public operations. The runtime currently has no Staff SQL/provider dependency. |
| File values and policy contract | File Platform `platform_types.rs`, `repository.rs::PlatformFile`, `access.rs` | Move those definitions to one canonical contract owner; update consumers to import that owner directly. |

Leave's approximately 2,400 domain lines contain configuration, policy, queries, persistence,
transactional workflow and evidence handling. FileTransfer has 793 runtime lines plus typed models
and extensive runtime tests for ephemeral discovery, pairing and signaling. These are cohesive
capabilities with independent state and tests. Achievement remains a small Staff capability under
the existing grouping rule; file size alone does not justify another domain crate.

## Intended crate boundaries and interfaces

Arrows below mean a production dependency from consumer to provider. Shared foundation dependencies
are omitted; existing legitimate dependencies are retained.

```text
application -> staff-http -> staff
                         -> staff-leave -> staff
                                        -> workflow -> file-platform
                         -> staff-file-transfer
                         -> auth-http / auth / notifications / file-platform
application -> files-http -> staff / staff-leave / file-platform
staff -> auth / file-contracts
staff-leave -> file-contracts
file-platform -> file-contracts
```

### Staff core and authoritative providers

`school-staff` retains personnel, career, rank milestones, organization, roles, policies,
Achievement models/services/policy and rollout/profile cutover. It retains `school-auth` because
`staff_name_service`, `staff_info_service` and `staff_service` use Auth's authoritative name
normalization/completeness operations. Auth has no production HTTP/File Platform path. Do not
duplicate those validators to manufacture a smaller dependency graph.

Add `src/services/staff_identity_query.rs` and publicly expose only these concrete providers
through `school_staff::services`:

```rust
pub async fn active_staff_display_name_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: uuid::Uuid,
) -> Result<Option<String>, school_errors::AppError>;

pub async fn effective_personnel_type_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: uuid::Uuid,
    date: chrono::NaiveDate,
) -> Result<Option<school_staff::career::StaffPersonnelType>, school_errors::AppError>;
```

The displayed self-crate path describes the public type; implementation uses its ordinary local
path. The name query preserves `user_display_names`, exact user ID, `user_type='staff'` and
`status='active'`. Missing/ineligible users return `None`; Leave maps that to its existing
`denied()` and FileTransfer HTTP to its existing `Forbidden("Active staff account required")`.
Database errors propagate unchanged. The career query preserves `kind='personnel_type'`,
`effective_date <= date OR effective_date IS NULL`, and exact ordering
`effective_date DESC NULLS LAST, created_at DESC, id DESC LIMIT 1`. Preserve NULL flattening and
the existing `ValidationError("ประเภทบุคลากรไม่ถูกต้อง")` for an invalid persisted personnel type.

Both providers use `DbSession` on the supplied connection, without checkout, transaction, clock,
cache, or new locks. Leave supplies `&mut **tx` from its current transaction and retains all date
calculation/validation in its existing caller. FileTransfer HTTP checks out through the existing
observed pool boundary before the name query. Query labels follow their authoritative Staff owner;
query text, values and call ordering remain unchanged. The intentional label changes are:

| Existing SQL operation label | New provider operation label |
| --- | --- |
| `staff.leave.repository.staff_name` | `staff.services.staff_identity_query.active_staff_display_name_on_connection` |
| `staff.file_transfer.staff_display_name` | `staff.services.staff_identity_query.active_staff_display_name_on_connection` |
| `staff.leave.repository.category` | `staff.services.staff_identity_query.effective_personnel_type_on_connection` |

FileTransfer HTTP retains its existing `staff.file_transfer.staff_display_name` observed-checkout
label; only SQL attribution moves to the authoritative provider. Preserve other operation labels,
query observations and logging behavior, avoiding incidental observability changes.

### Leave

Create `school-staff-leave` with `models`, `policy`, `files`, `service`, and private configuration,
lifecycle, query and repository modules. Move the existing non-HTTP implementation and
`crates/school-staff/tests/leave.rs`; imports of `StaffPersonnelType` and the two authoritative
queries point to Staff. Remove the old active `school_staff::leave` implementation without a
production re-export back to Leave, which would create a dependency cycle.

Keep Workflow's process operations, notification acceptance and private-signature snapshot
provider. Leave remains transitively coupled to File Platform and Auth HTTP through Workflow;
do not remove legitimate operations to claim complete cache isolation. Files HTTP changes its
Leave authorization/upload/delete calls to `school_staff_leave::files` while retaining its direct
Staff policies for profiles, Achievement and licenses.

Move Leave-specific closed/exhausted-pool preflight assertions out of Staff's
`staff_directory_query.rs` test module. Preserve their validation-before-checkout guarantees in
Leave tests; Staff retains its own directory/license/career assertions. Do not add a Staff-to-Leave
dev dependency merely to retain the old test location.

### FileTransfer runtime and transport

Create `school-staff-file-transfer` with `models`, `runtime` and the existing runtime tests. Its
runtime keeps current identity `(tenant_id, user_id, session_id)`, bounded queues/admission,
consent, host-only signaling, timeouts, closed-pair receipts, generation fences and stream guard
drop/disconnect semantics. It needs no Staff query or invented provider trait.

The root creates one process runtime in `main.rs` and injects that same `Arc` into HTTP state.
Preserve registration/snapshot/removal/pair operations and SSE's session revocation and permission
signals, periodic tenant/session/active-Staff revalidation, logical pool lease release, runtime
refresh, no replay, typed encoding errors, keepalive and no-store/no-buffering headers. Keep
current failure handling on lost signal channels and disposal. The HTTP unit fixture retains its
registered device ID rather than making the runtime guard's currently restricted `device_id`
field public solely for a moved test.

### Staff HTTP

Create `school-staff-http`. Preserve conventional `http/{state,routes,handlers}` for Staff,
`achievement/{handlers,routes}` for Achievement transport, and `leave.rs`/`file_transfer.rs` for
their current concrete transport modules. The public consumer paths are
`school_staff_http::http`, `school_staff_http::achievement`, `school_staff_http::leave` and
`school_staff_http::file_transfer`. The root updates handler paths in OpenAPI composition.

Keep `StaffHttpState` (Auth runtime and File Platform), `LeaveHttpState` (Auth runtime and work
event channel), and `FileTransferHttpState` (Auth runtime and the runtime `Arc`) separate and
unchanged in meaning. Every factory returns its existing concrete `Router<OwnState>`. Root
`FromRef` projections, prefixes and the single shared session boundary remain authoritative.
Move the existing transport tests with their implementation and preserve API declarations and
response mapping exactly. No domain-to-HTTP edge or generic router over `AppState` is introduced.

### Narrow File contracts

Create `school-file-contracts` with unchanged `platform_types.rs`, the exact `PlatformFile` value
in `file.rs`, and the exact `FilePolicyAction`/`FileAuthorizationPort` in `access.rs`. It owns
provider-neutral DTO/enums and the authorization contract, with only the foundation/external
dependencies actually required by those definitions. Optional `api-docs` remains optional and
forwards only schema support needed by these types; normal production excludes Utoipa.

This Files-owned contract crate is a new explicit bounded exception to `.rules`' requirement that
a crate's public interface be materially narrower than its implementation. The existing Academic
realtime exception does not authorize it generally. Update `.rules` in the implementation to name
only these existing File DTO/enums, `PlatformFile` and authorization values/port: independent policy
consumers require their shared types without storage, renderer or SDK implementation. Preserve
contract serialization/schema tests and measured invalidation evidence. This exception does not
permit arbitrary protocol, utility or runtime crates, additional provider interfaces, or namespace
and trusted-constructor extraction.

`school-file-platform` keeps `SqlFileRepository`, repository operations/errors/work items,
`FilePlatform`, storage providers/R2, namespace/trusted constructors, inspection, rendering,
purpose registry, lifecycle, retention, reconciliation and cleanup. Update all definition consumers
to canonical contract imports, including mixed consumers that still need implementation APIs.
Avoid permanent old-path facades; any demonstrated compatibility re-export needs a specific
consumer and removal rationale. Preserve derives, serde and schema names, fields and values.

Staff license and Leave file policy each require exactly `FilePolicyAction`, `FilePurpose`,
`FileVisibility` and `PlatformFile` from contracts. File Platform used by Staff test fixtures
becomes a dev dependency. After all moves, Staff core removes production Workflow, Notifications,
HTTP, Auth HTTP and File Platform edges. Budget can likewise remove its direct File Platform
edge where only these values are used, but retains its legitimate transitive Workflow coupling.

## Same-CPU cache and setup work: method pending review

The user selected the existing GitHub four-CPU runner with a cross-run paired source/target cache
trial. Keep that runner and CPU allocation; do not provision a host or add spend. Prior runner/
variable inspection returned HTTP 403 and established no persistent host. Local same-builder reuse
is a separate control and cannot prove cross-run benefit.

The snapshot restores paired frozen source and Cargo target state at the same `/app` path across
fresh image builders, separately from the local Cargo comparator's fixed path below. A separate
`cargo-snapshot-backend-school` Actions artifact uses schema version 1; the existing prepared-release
schema version 1 remains unchanged. Its exact field contract is:

| Section | Fields and values |
| --- | --- |
| `producer` | `repository`, `workflowPath`, `runId`, `runAttempt`, `sha`, `event`, `ref` (`refs/heads/main`) |
| `snapshot` | `ghcrRepository`, `immutableDigest` (`sha256`), `platform` (`linux/amd64`), `sourcePath` (`/app`) |
| `identity` | `buildConfigurationHash`, `externalLockIdentityHash`, `sourceManifestSha256`, `backendInputHash` |

The production consumer resolves a pinned digest only from an API-verified prior completed,
successful trusted main Pipeline run: expected repository and head repository, exact workflow
path, trusted event, main ref, run ID/attempt and SHA, plus successful backend preparation for that
same run/attempt/SHA. Resolve run and attempt origin independently through the API; identity fields
supplied by the locator are claims to verify, not authority. Validate artifact API origin and producer binding, including
`backendInputHash` equality with that run's existing prepared-backend input hash. It then validates
the OCI digest, snapshot manifest/archive, source manifest SHA256 and archive/source allowlist.
The cached source hash may differ from current source; that is the reuse case. Build configuration
and external dependency identities must be compatible with the current build.

A mutable registry tag never authorizes production restore. Missing, deleted, corrupt, mismatched
or untrusted run/artifact/snapshot state falls back to the current cargo-chef path; actual compiler
errors still propagate. Main snapshot publication has explicit main-ref/trusted-event guards.
Preserve existing branch manual image publication behavior: a branch with `packages: write` still
has that capability, and an `if` guard is not a token-level restriction. Branch snapshot trials use
run-owned scratch references only; production locators use verified main artifacts. No local test
receipt, signature or test-attestation system is introduced. No secrets or runtime data enter the
snapshot. Pipeline owns the concrete implementation and canonical operational instructions.

Before extraction, preflight archive metadata against normalized allowed source/target roots.
Reject absolute/traversal/duplicate/type-conflicting/special paths, unsafe symlink or hardlink
targets and privileged modes. Extract into private empty staging, validate manifest/content/type/
mode, then promote source and target as a coupled snapshot. Synchronization never follows restored
source symlinks; ambiguous metadata rejects the snapshot rather than reaching outside its roots.

Accept a snapshot for incremental synchronization only when the maximum saved `lstat` mtime across
all restored source and target entries, including directories, is strictly less than the current
clock minus two seconds. Directory metadata matters because root `build.rs` watches `migrations`.
Future, equal or insufficiently old timestamps reject the incremental root and use chef fallback.
Hash-and-mode-equal files retain saved mtimes. Changed/new/renamed paths are copied without incoming
mtimes, stamped after the check, and read back to prove their actual mtime is strictly newer than
the entire cached maximum. Equal/coarse-resolution or other ambiguous results reject the snapshot.
Delete vanished paths; handle added/changed/deleted directories and build-input watches, and reject
ambiguous type/symlink states. Do not edit Cargo fingerprints. Sanitize Cargo environment to closed,
public build inputs; their config/toolchain/options identity changes require fallback.

Only if production adopts snapshots, Pipeline clarifies Operations' current statement that GitHub
does not maintain compiler snapshots. An isolated trial does not change production cache policy.
Pipeline's ten passing ignored metadata-prototype cases are preliminary algorithm evidence, not
actual Cargo invalidation or fresh-run restoration proof.

Prove a fresh-run restore, not only a second build in the same builder. Cases must include changed
content with forged prior mtime, future/equal/coarse cached timestamps, missing/deleted input,
rename, `build.rs` and its environment/rerun inputs, `include!` inputs, manifests/lock/config,
Cargo environment and incompatible toolchain/config identity. A stale target must never suppress
a required compile.
Measure pull, restore/sync, compilation/linking, export/push and complete net time separately.
Only repeated net gain justifies production adoption; otherwise retain current production cache
and record the completed experiment honestly.

Six School release jobs retain real secret/matrix boundaries; do not combine them to remove runner
allocations. Set up a Buildx builder only on an actual build after OCI lookup. Accepted registry-alias
metadata uses imagetools with a verified available CLI and no builder bootstrap. Preserve existing
branch manual image behavior, exact release provenance, private preflight, maintenance, migrations,
readiness, proxy checks and acceptance joins. Pipeline verifies the actual action ordering and CLI
capability through focused fixtures.

## Verification and performance acceptance

Pipeline exclusively owns heavy Cargo, formatters, lockfile resolution, generators and the
external measurement harness; heavy tools execute sequentially after the Senior's explicit phase
grant, with any later QA ownership handoff explicit. Archive and hash all baseline source,
manifests, timing inputs, harness and configuration first. Once those immutable inputs are isolated
at the comparator source path and independent design/method review passes, the Senior may grant
Pipeline baseline measurement while Backend edits the live checkout. Completing every baseline
timing first is not a prerequisite for isolated live edits. Every source/input consumed by a Cargo
invocation stays stable; nobody builds or generates from live source, or runs heavy Node/database
work, during timing. Final integrated Reviewer/QA acceptance still waits for all writers and a
complete candidate freeze.

Initial free space was approximately 1.36 GiB; the Senior's exact hash-ledger retirement of obsolete
mutable historical debug outputs has left approximately 3.39 GiB. Require 3 GiB before graph prime,
an output-size forecast and an initial 2 GiB compilation floor with measured stops/Senior
reassessment as necessary. Do not duplicate targets. Per-stage cleanup may retire only exact new,
unused archive-build cache outputs after that stage's complete probes/API export, with Senior
hash-ledger approval; preserve pristine base/candidate binaries and all records. No blanket clean.
Prior
proof directories `/workspace/rust-compiler-audit`, `/workspace/rust-opt-level-experiment`,
`/workspace/backend-build-scope-benchmark`, `/workspace/auth-file-codegen-benchmark` and
`/workspace/schema-cache-benchmark` remain immutable.

Pipeline's proposed fixed source is `/workspace/staff-contract-cache-benchmark/app`, with existing
target `/workspace/schoolorbit-new/backend-school/target`, Rust 1.98.1, four CPUs, three Cargo jobs,
fixed release flags and the same 531 external identities, including actual compiler artifact
feature/configuration sets. Compare normal release and fresh API feature configurations separately.
Prime each workspace graph separately
with reused externals; this is not a true empty-target cold build. Pristine resets/reset builds
and graph priming are untimed. Run at least two harmless public-constant source probes per family:
Staff HTTP, Leave domain, FileTransfer runtime and File Platform implementation, plus an application
control. Edit frozen scratch sources only. Base runs first, then candidate; disclose this order
limitation and do not call it counterbalanced. Record unchanged pristine reuse separately.

Include a forced whole-workspace rebuild with reused externals and pristine binary size/hash.
The candidate probe paths relative to `backend-school` are:

| Family | Candidate source |
| --- | --- |
| Application control | `src/main.rs` |
| Staff HTTP | `crates/school-staff-http/src/http/handlers/staff.rs` |
| Leave implementation | `crates/school-staff-leave/src/service.rs` |
| FileTransfer runtime | `crates/school-staff-file-transfer/src/runtime.rs` |
| File implementation | `crates/school-file-platform/src/platform_service.rs` |

Record actual dirty compiler units/packages, per-compiler wall intervals and child CPU, Cargo and
executable/link times, resource/oom events and source-reset confirmation. Do not sum overlapping
compiler wall intervals or add link time to a binary unit that already includes it. A transparent
jobserver-preserving timing wrapper needs independent method approval. Fresh API/TypeScript
generation runs outside timed samples, sequentially on the same target; do not create a separate
docs target or run database/heavy Node work during timing. Cache transfer/export time is separate
from warm Cargo comparisons. The base currently exports 543 paths and 713 operations; these counts
are informative, while complete JSON/TypeScript byte equality is the acceptance criterion.
Do not compare absolute LLVM timings from old compiler/profile experiments with this candidate.

Accept ownership changes only with singular providers, the intended acyclic graph, unchanged
behavior/contracts and measured invalidation results. Report per-family samples/median, whole
rebuild and size tradeoffs; do not claim a universal speedup or mask regressions. The Senior
reconciles regressions against measured benefit before delivery. Cache adoption additionally
requires repeatable net fresh-run gain and all invalidation/fallback cases passing.

Verification selects all moved/touched owners and direct consumers: Staff core and providers,
Leave and evidence/workflow transactions, FileTransfer runtime and HTTP/SSE lifecycle, Staff HTTP,
File contracts/platform, Files HTTP and affected Auth HTTP/root fixtures. Preserve denials,
validation-before-checkout, rollback and current-date/career winner cases. Run default and
`api-docs` workspace/all-target checks; exact production panic Clippy gates; formatting; backend
architecture/database-execution guards; fresh offline API export/generation/check and permission
artifact checks. Frontend guards/discovery cover the affected Staff, Leave, file and auth/session
consumers; native browser execution is selected only where impact requires it. Pipeline shell,
YAML, Node and Bats checks cover its actual changes. Recipes stay in canonical testing docs.

Independent Reviewer and QA inspect the complete frozen candidate. QA chooses proportional checks
from `.rules`, reports exact executed commands/tree/environment and identifies unrun gates.
Tests stay local; Actions do not receive local-test receipts. The Senior owns Git/checkpoints,
PR/readiness/main and actual selected-main build/deployment acceptance. No successful local
benchmark or draft checkpoint substitutes for those production gates.
