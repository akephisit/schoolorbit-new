# Staff boundaries, File contracts, and release cache implementation plan

## Status and execution contract

Candidate implementation input for the approved five-part scope in the
[design](../specs/2026-11-11-staff-contract-cache-design.md). The Senior and independent Reviewer
must accept the design and this plan before runtime edits. The user selected the existing GitHub
four-CPU runner and a cross-run cache trial; detailed cache/setup tasks remain provisional until
the Senior reconciles Pipeline's proposal and the independent method review.
No additional user approval is needed for work already within the approved scope.

Base: `431ef887bc94f608df6684b8a1f777a570c81a40`, tree
`0af566f919b5986eb7f14fb2c5092101d9d1a3b0`, branch
`perf/staff-contract-cache-pipeline`, shared checkout `/workspace/schoolorbit-new`.
All boxes are pending; no implementation or verification result is implied by this plan.

## Role and path ownership

| Owner | Writable paths after the relevant grant | Boundaries |
| --- | --- | --- |
| Senior | Git/index/checkpoints/PR/main/production operations | Reads, diagnoses, resolves interfaces and accepts evidence; never takes over tracked edits. |
| Backend | `backend-school/**` except Dockerfile/scripts; `.rules`; `backend-school/README.md`; `docs/TESTING.md`; these two workflow files | Owns source and Cargo manifest/lockfile decisions, domain/transport/contracts consumers and backend test/guard repairs. No frontend or generated-artifact hand edits. |
| Pipeline | `backend-school/Dockerfile`, `backend-school/scripts/**`, `scripts/**`, `.github/**`, `docs/OPERATIONS.md` | Owns all heavy Cargo execution, rustfmt, resolution, generators and external measurement harness sequentially; generated writes need explicit Senior assignment. |
| Independent Reviewer | Read-only complete candidate | Reviews rules, source ownership, transaction/security/lifecycle preservation, test rationale and measurement validity. |
| Independent QA | Read-only candidate; owned disposable verification resources | Selects and runs checks independently; coordinates heavy execution through Pipeline and reports failures to Senior without source edits. |

Current grant permits Backend to write only these two workflow input candidates. Before any
shared-file/generator work, Senior confirms ownership and all dependent writers are paused.
Never stage/commit, switch branches, stash/reset/clean, edit the prior evidence directories, or
start a second Rust target. At checkpoints every writer acknowledges pause; resume only after
Senior supplies the saved HEAD. Final review/QA uses a completely frozen candidate. Timed Cargo
uses isolated frozen archive inputs; live edits may proceed after the explicit phase grants below.

## 1. Approve inputs, archive baseline, and grant execution

- [ ] Senior and Reviewer reconcile the precise provider APIs, HTTP public paths, no-cycle graph,
  scope and provisional Pipeline sections against the actual tree.
- [ ] Pipeline confirms current free space, four-CPU allocation, pinned toolchain/configuration,
  three Cargo jobs, source/target path and all 531 external lock and actual artifact feature/config
  identities for normal release and fresh API separately. Retain opt-level
  3, CGU 16 and root-only local ThinLTO disablement. Proposed fixed source is
  `/workspace/staff-contract-cache-benchmark/app`, target is the existing
  `/workspace/schoolorbit-new/backend-school/target`, and toolchain is Rust 1.98.1.
- [ ] Confirm 3 GiB before graph prime, an output-size forecast and an initial 2 GiB compilation
  floor with measured stops/Senior reassessment. Initial free space was approximately 1.36 GiB;
  exact hash-ledger debug retirement has left approximately 3.39 GiB. Do not remove prior evidence,
  duplicate targets or begin graph population before resource acceptance.
- [ ] Archive and hash all baseline source, manifests, API, timed inputs, harness and configuration
  before runtime writes. Isolate frozen inputs at the canonical comparator path. Preserve all five
  old proof directories named in the design; new evidence has a separately owned external path.
- [ ] Senior obtains independent design/method approval, confirms immutable archive isolation, and
  explicitly grants Pipeline baseline measurement and Backend live implementation phases. Those
  phases may overlap; baseline timing completion is not required before live edits. Each Cargo
  invocation consumes stable isolated inputs; no live build/generator, heavy Node or database work
  runs during timing. No Cargo, formatter, generator, install or tests run before the relevant grant.
- [ ] Pipeline baseline includes independent graph prime, unchanged reuse, at least two source-changing
  warm samples for Staff HTTP, Leave, FileTransfer runtime and File Platform implementation,
  plus an application control, forced whole-workspace rebuild with reused externals, binary size
  and canonical API/TypeScript. Graph population reuses externals and is not true cold.
- [ ] Per-stage cleanup retires only exact new unused archive-build cache outputs after complete
  stage probes/API, with Senior hash-ledger approval. Preserve pristine base/candidate binaries,
  records and all five old proof directories; never run a blanket clean.

## 2. Introduce the single File contract owner

- [ ] Add `school-file-contracts` to workspace membership/path dependencies, root dependencies and
  `api-docs` forwarding only where required. Use existing centralized versions/lints.
- [ ] Add the explicit bounded Files-owned contracts exception to `.rules`' narrower-interface
  admission criterion. Its scope is exactly the moved DTO/enums, `PlatformFile` and authorization
  values/port; the Academic realtime exception is not a general authorization. Keep implementation,
  namespace and trusted constructors outside this exception, with schema and compile evidence.
- [ ] Move `platform_types.rs` unchanged to contracts; move exact `PlatformFile` to `file.rs` and
  exact `FilePolicyAction`/`FileAuthorizationPort` to `access.rs`, adjusting only definition imports.
- [ ] Keep repository operations/errors/work items, FilePlatform, provider/namespace/trusted
  constructors, inspection/rendering/R2, lifecycle/reconciliation and cleanup in File Platform.
- [ ] Cut all moved-definition consumers to canonical contracts imports. Keep implementation
  imports where real storage/lifecycle operations are needed; do not introduce a catch-all facade.
- [ ] Cut Staff license and Leave file policy to their four required contract values. Remove
  Budget's direct implementation dependency if its verified production uses are contract-only,
  while documenting the retained Workflow transitive dependency.
- [ ] Backend freezes manifest decisions; Pipeline alone resolves the lockfile after that freeze.
  Verify external versions/checksums/features and production optional-schema exclusion.

## 3. Establish Staff providers and extract Leave

- [ ] Add `services/staff_identity_query.rs` and the two public connection-bound providers in the
  design. Move exact active-Staff name and effective personnel-type SQL from Leave/FileTransfer;
  preserve nullable results, parse error, caller dates and complete winner ordering.
- [ ] Keep missing-name mapping in each consumer so existing Forbidden messages remain unchanged.
  Providers acquire no pool connection, transaction, clock or extra lock.
- [ ] Apply only the three intentional SQL operation-label moves listed in the design; retain
  FileTransfer's observed-checkout label and other existing instrumentation/logging semantics.
- [ ] Create `school-staff-leave`; move every non-HTTP Leave module and its package integration
  target. Consume `StaffPersonnelType` and Staff provider functions directly on `&mut **tx`.
- [ ] Retain Workflow process/signature/notification operations and exact transaction ordering,
  current cancellation routing, quota/replay/receipt, evidence snapshot and rollback behavior.
- [ ] Move Leave preflight tests from Staff directory tests to Leave, splitting mixed exhausted-pool
  assertions by owner without losing coverage or adding a Staff-to-Leave dev edge.
- [ ] Change Files HTTP Leave providers and root Leave schema imports to the new owner. Remove old
  Staff Leave modules/re-exports and forwarding, preserving all routes/contracts.
- [ ] Add or retain focused provider cases for inactive/non-Staff/missing names, effective-date
  ties/NULLs and same-transaction visibility/rollback. Exercise existing Leave tests rather than
  duplicating its full business suite in Staff.

## 4. Extract FileTransfer runtime and all Staff transport

- [ ] Create `school-staff-file-transfer` with exact models, runtime and runtime tests. Runtime
  imports no Staff query/trait, Auth HTTP, File Platform or Axum.
- [ ] Preserve generation-fenced stream guards, timeouts, pairing consent, admission/queue bounds,
  host-only signaling, close receipts and same-tenant/user/session ownership.
- [ ] Create `school-staff-http` with `http` Staff handlers/state/routes, `achievement` handlers/routes,
  and concrete `leave`/`file_transfer` transport modules. Keep Achievement domain/policy in Staff.
- [ ] Preserve separate concrete Staff/Leave/FileTransfer states, route factory return types and
  canonical Auth HTTP context/cache. Retain post-commit notification and file cleanup ordering.
- [ ] FileTransfer HTTP performs active-Staff lookup through Staff's public provider, including SSE
  revalidation. Retain session/permission failure handling, logical lease release, disconnect/drop,
  typed encoding, body limit and no-store/no-buffering/keepalive behavior.
- [ ] Move HTTP units with transport. Retain the HTTP fixture's registered device ID rather than
  widening the runtime guard field for a moved test.
- [ ] Update application `app.rs`, `http_state.rs`, `main.rs`, `api_contract.rs`, permission route
  fixture, Staff FileTransfer/career/Academic fixtures, and Auth/School/Supply runtime state fixtures.
  Create one process runtime and preserve `Arc` identity across every HTTP projection.
- [ ] Remove Staff production Workflow, Notifications, HTTP, Auth HTTP and File Platform edges;
  retain Auth name authority and contracts. Put File Platform fixtures in dev dependencies only.
- [ ] Reconcile all architecture/source/lifecycle guards with new canonical paths, including
  forbidden reverse edges and no old active owner. Record why each path-based test changes while
  preserving its observable requirement; never bless changed business outputs.

## 5. Investigate same-CPU persistence and implement justified setup changes

This task is Pipeline-owned and method review is pending. The user selected the existing GitHub
four-CPU runner and a cross-run trial; no host or CPU/spend change is authorized. Existing runner/
variable API access was HTTP 403 and established no persistent host.

- [ ] Keep the selected GitHub four-CPU runner for the cross-run source/target snapshot trial.
  Do not introduce a host, add spend or silently change CPU/job/profile comparisons.
- [ ] Add separate `cargo-snapshot-backend-school` Actions artifact schema v1 with the exact
  producer/snapshot/identity fields in the design. Keep the prepared-release schema unchanged.
- [ ] Production resolves a pinned OCI digest only from an API-verified successful trusted main
  Pipeline run and successful backend preparation, checking repository/head repository, workflow,
  ref/event, run/attempt/SHA, artifact origin and producer binding to that run's prepared backend
  input hash. Then validate OCI digest, manifest/archive/source SHA256 and allowlist. The cached
  source hash may differ current; build configuration and external identities must be compatible.
- [ ] Independently retrieve actual API run/attempt metadata to establish origin; never trust the
  locator's supplied identity fields as authority. Test mismatched run/attempt/artifact bindings.
- [ ] Never authorize restore from a mutable registry tag. Missing/corrupt/deleted/untrusted or
  mismatched state uses chef fallback. Guard main snapshot publication by explicit main ref and
  trusted event while preserving existing branch manual image behavior and honestly retaining its
  `packages: write` capability. Branch snapshot trials use run-owned scratch only, with no local
  receipts, signatures or test-attestation system.
- [ ] Preflight archive metadata before extraction: normalized allowed roots, no absolute/traversal/
  duplicate/type-conflicting/special paths, unsafe symlink/hardlink targets or privileged modes.
  Use private empty staging, no-follow synchronization and manifest/content/type/mode validation
  before promoting coupled source/target. Add rejection fixtures for unsafe archive entries.
- [ ] Restore paired source/target at `/app`; accept incremental reuse only when maximum saved
  source/target `lstat` mtime, including directories, is strictly older than now minus two seconds.
  Include the root `build.rs` migrations-directory watch. Retain hash-and-mode-equal
  file times; copy changed/new/renamed paths without incoming mtime, stamp them and verify actual
  readback strictly exceeds cached maximum. Future/equal/coarse/ambiguous timestamps or file types
  reject the incremental root to chef fallback. Delete vanished inputs; never edit fingerprints.
- [ ] Close/sanitize Cargo environment to public build inputs and include config/toolchain/options
  in identity; changed environment requires fallback. Handle watched directory additions, changes
  and deletion in the synchronization/invalidation cases.
- [ ] Prove fresh-run restoration and required invalidation for equal-content/mtime retention,
  changed content with forged mtime, future/equal/coarse cached timestamps, new/missing/deleted/
  renamed inputs, `build.rs` and its rerun/environment inputs, `include!`, Cargo environment/graph
  and config/toolchain changes. Incompatibility must use the current chef fallback safely.
- [ ] Run counterbalanced repeated same-CPU trials; report pull/restore/sync, compile/link,
  export/push, cache errors/fallback and complete net wall time separately. Production adoption
  requires repeatable net improvement; retain current production cache if the trial loses.
- [ ] Preserve six School job secret/matrix boundaries. Run Buildx setup only for an actual build
  after OCI lookup; registry-alias metadata uses available imagetools CLI without builder bootstrap.
  Preserve existing branch manual image publication and prove action ordering/CLI availability.
- [ ] Preserve private preflight, release provenance, maintenance, migrations, readiness/proxy,
  dependency joins and accepted-baseline rules. Add meaningful shell/Node/Bats/YAML fixtures for
  the affected actual execution paths and fallback behavior.
- [ ] Clarify Operations' current no-compiler-snapshot statement only if production adopts the
  reviewed snapshot cache; an isolated completed experiment does not change production policy.
- [ ] Treat Pipeline's ten passing ignored metadata-prototype cases as preliminary algorithm
  evidence only; obtain actual Cargo invalidation and fresh-run restoration evidence before adoption.

## 6. Verify and measure the integrated candidate

Pipeline executes heavy tools one at a time on the frozen source with the compatible shared target;
QA independently selects scope and inspects results. Use canonical
[Testing](../../TESTING.md), especially Backend School, API Contract, School session authentication
and Staff direct file transfer. Do not repeat a broad suite after valid focused evidence unless
new inputs/risks require it.

- [ ] Run the documentation-policy check for these input-only files. For implementation, update
  canonical `.rules`, backend README, testing and operations owners to describe the final graph
  and actual cache decision; avoid completion reports/new documentation owners.
- [ ] Check formatting, `cargo check --workspace --all-targets --locked` in default and `api-docs`
  modes, production binary deny-warning check, production dependency graph, and both production
  Clippy invocations using `--workspace --lib --bins --exclude school-test-db --locked --no-deps`
  with exactly `unwrap_used`, `expect_used`, `panic` and `unreachable` denied.
- [ ] Run `static_architecture` and `database_execution_architecture`; affected pure/runtime/HTTP
  units include Staff HTTP, FileTransfer, Auth HTTP, contracts and File Platform. Use the disposable
  database runner for Staff core/providers, new Leave package, Files HTTP and affected root fixtures.
- [ ] Preserve all Leave policy/evidence/approval/cancellation/replay/quota/denial tests, Staff
  personnel/career/license/name cases, validation-before-checkout and exhausted-pool behavior.
  Include provider transaction cases and FileTransfer session/tenant/owner, SSE lost-channel,
  revalidation, disposal, CSRF, body-limit and proxy-hint cases.
- [ ] Generate/check API through its canonical exporter and generators; compare fresh complete
  OpenAPI and generated TypeScript bytes with the archived base. Run API/schema/permission tests
  and verify permission artifacts plus both migration timelines remain byte-identical. Never
  hand-edit an export or generated consumer to achieve equality.
- [ ] Run affected frontend Staff/Leave/files/auth static guards, API generator tests and synthetic
  loopback Playwright discovery. Select native browser flows only if the reviewed impact requires
  execution; discovery alone does not establish browser correctness.
- [ ] Run Pipeline's changed shell/YAML/Node/Bats checks, including local-runner package wiring,
  test-target discovery and fail-on-zero matching as needed for new packages. Keep local tests out
  of Actions and do not upload local receipts as release certification.
- [ ] Repeat candidate probes on the baseline's source path/target/toolchain/configuration with
  separate graph primes/pristine resets and at least two warm samples per source family. Record
  actual rebuilt units, per-compiler wall/child CPU without summing overlapping wall intervals,
  Cargo/executable/link time, forced workspace rebuild, binary size/hash and fresh API equality;
  separate cache-transfer totals from Cargo invalidation evidence. Source probes append harmless
  public constants to scratch inputs only, using an excluded pristine reset build between owners.
- [ ] Disclose base-first/candidate-second ordering and never describe those samples as
  counterbalanced. Record resource/oom events, source reset and unchanged compiler freshness.
  Independently approve any transparent jobserver-preserving timer before using it. Run fresh
  API/TypeScript outside timed samples on the same target, with no concurrent database/heavy Node.
- [ ] Retain normal-release and fresh-API external artifact feature/configuration identity evidence;
  equal 531 lock tuples alone are insufficient. Do not compare absolute LLVM results from old
  compiler/profile experiments with these source-boundary measurements.
- [ ] Senior reconciles repeated gains and regressions honestly. No faster-runner, stale-target,
  same-builder-only, changed-profile or changed-external-identity sample proves the requested gain.

## 7. Review, checkpoints, and delivery

- [ ] Senior pauses all writers/tools, checks the actual agreed paths/diff, saves useful checkpoint
  commits and opens/updates the draft PR. Checkpoints report unrun work accurately.
- [ ] Freeze complete candidate for independent Reviewer and QA. Review ownership, public surface,
  no-cycle graph, exact transaction/provider semantics, async lifecycle, test-change rationale,
  contract equality, pipeline gates and reproducible measurement attribution.
- [ ] Return findings through Senior to the owning writer; refreeze and rerun affected checks after
  fixes. No Senior/QA tracked-file repairs and no concurrent writes during final acceptance.
- [ ] Senior refreshes origin/main and performs required latest-main integration/gate checks. Any
  new base/source/environment invalidates affected prior evidence and receives the required rerun.
- [ ] Complete actual selected-main build/deployment/readiness/proxy/acceptance after merge under
  Senior ownership. Local checks or a cache experiment do not replace this result.
- [ ] PR records final behavior/graph, executed evidence and limitations; remove these temporary
  workflow inputs after that outcome is recorded. Report any unavailable external gate explicitly.

## Completion criteria

All approved work areas have a concrete reviewed result: singular Staff/Leave/FileTransfer/HTTP
ownership and narrow File contracts; byte-equal external artifacts and unchanged security/data/
lifecycle behavior; repeated same-CPU invalidation/whole-rebuild evidence; a completed cache trial
with adoption only on demonstrated net gain; justified setup savings with unchanged release
boundaries; required local review/QA; and the selected-main production acceptance result.
