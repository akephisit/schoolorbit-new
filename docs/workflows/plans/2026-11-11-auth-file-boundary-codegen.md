# Auth/File Boundary and Codegen Implementation Plan

The companion [design](../specs/2026-11-11-auth-file-boundary-codegen-design.md) governs this
architectural change. Starting commit is `d705f6ead9ee03b26086efe6420d7d8f1933eb17`, tree
`d5843306cd14525c09f63707686b851458acc022`, on
`perf/auth-file-boundary-codegen-units` in the shared repository checkout.

## 1. Freeze and measure the starting source

Owner: Pipeline, coordinated by the Senior. Backend may write these two workflow inputs, but does
not change Rust until the immutable baseline source archive is complete and the Senior/Reviewer
acknowledge the concrete boundary design. That archive is the only baseline-capture prerequisite:
Pipeline may prime and benchmark its archive while Backend implements in the live checkout after
the Senior's explicit grant.

- Preserve previous audit/experiment evidence without modification.
- Capture baseline source outside the repository and establish the one sequential Cargo owner,
  compatible target, toolchain, dependency state and measurement command.
- Compare only `school-staff` package CGU default 16, 32 and 64 with six timed public-constant
  Staff source probes in order `16, 32, 64, 64, 32, 16`. Exclude configuration primes and pristine
  resets. Preserve root binary and external-crate CGU, optimization level 3 and root binary
  `lto=off`. Capture complete Cargo wall time and the actual three-package rebuild closure:
  Staff, Files HTTP and root. Report failures, missing metrics and resource limits.
- Capture two baseline Files implementation source probes at Staff CGU 16 for the later boundary
  comparison; candidate source will receive two matching probes on the same canonical path with
  primes excluded. Record the baseline-then-candidate stage-order limitation.
- Require a repeatable Cargo wall-time gain of at least 5% plus actual production runtime/output
  equivalence for each workload, with twelve interleaved controls and bootstrap confidence
  intervals. A 3% screen is not automatic acceptance; retain default 16 for uncertainty or a
  regression. Do not infer a winner from incomplete data. No other package knob, Student Care
  change, paid runner, opt-level change or additional extraction is in scope.

## 2. Cut over the application-owned profile command

Owner: Backend, after the Senior confirms the file assignment and capture/design prerequisites.

- Add `ProfileUpdatePort` in `crates/school-auth-http/src/http/state.rs` with the design's exact
  typed signature; replace the File Platform state field with `Arc<dyn ProfileUpdatePort>`.
- Add `src/adapters/auth_profile.rs` and register it in `src/adapters.rs`. Move the old profile
  transaction, error mapping, commit/read/cleanup order and observation label without changing
  providers or behavior. Include focused unit coverage of the relationship-error mapper.
- Bind `ApplicationAuthProfile` from the existing shared File Platform in `src/http_state.rs`.
  Route the existing handler through the injected command and retain role lookup/response mapping.
- Remove the old orchestrator file and module declaration. Remove Auth HTTP's Files dependency and
  `api-docs` forwarding; add workspace `async-trait`. Do not change external dependency versions,
  root Files feature forwarding, DTOs, route metadata, SQL, migrations or permission artifacts.
- Update the approved dependency graph and lifecycle owner guard in
  `backend-school/tests/static_architecture.rs`; add a durable guard against Files coupling or a
  second request-context implementation in Auth HTTP/application composition.

## 3. Verify the actual adapter and authenticated route

Owner: Backend for test source, then independent QA for execution/acceptance. All databases are
disposable; tests retain separate named fixture pools. The shared ready-image helper remains
available to existing Timetable consumers.

- Strengthen `src/integration_tests/auth/profile_integration_tests.rs` to call the production
  adapter through its public command port instead of duplicating the orchestration in the test.
- Verify new attachment plus profile fields commit together; omitted image preserves; same image
  performs no deletion; replacement and explicit null detach through Files lifecycle cleanup.
- Verify foreign-owned, not-ready and wrong-purpose files return the exact validation error and
  leave profile/file facts unchanged. Trigger Auth rejection after promotion and prove both the
  promotion and profile mutation roll back with no cleanup.
- Observe successful cleanup only after the new profile is committed. Inject a disposable
  repository deletion failure and verify the adapter reports the existing post-commit error
  while the accepted profile/new-image promotion remain committed and the old file remains
  available for recovery through the normal owner.
- Extend the production-router fixtures in
  `src/integration_tests/auth/session_http_tests.rs` for the profile route: session/CSRF/tenant
  refusal before mutation and accepted update using only the authenticated actor's tenant pool.
  Reuse canonical session middleware and application `FromRef`; add no feature-local identity
  helper or copied authentication.

Selected existing commands, executed only by the assigned Cargo/QA owner after source freeze:

```bash
./scripts/test_backend_school.sh integration_tests::auth::profile_integration_tests -- --nocapture
ENCRYPTION_KEY=$(openssl rand -hex 32) ./scripts/test_backend_school.sh integration_tests::auth::session_http_tests -- --test-threads=4 --nocapture
./scripts/test_backend_school.sh --package school-file-platform profile_relationships -- --nocapture
./scripts/test_backend_school.sh --package school-auth profile_ -- --nocapture
```

From `backend-school`, the selected package/workspace/build checks are:

```bash
cargo test --locked -p school-auth-http
cargo test --locked --test static_architecture
cargo check --workspace --all-targets --locked
cargo check --workspace --all-targets --features api-docs --locked
cargo test --locked --bin backend-school --features api-docs api_contract::tests -- --nocapture
```

The verification owner also applies the production panic-form Clippy gate from the owning local
runner and checks formatting of affected Rust. API output is expected byte-identical; run the
existing offline contract check through the single generator owner, inspect any difference and
never hand-edit generated output.

From `frontend-school`, retain the unchanged browser/contract boundary with:

```bash
node --test tests/static/session-auth-contract.test.mjs tests/static/account-security.test.mjs tests/static/auth-session-state.test.mjs tests/static/file-platform-contract.test.mjs tests/static/api-response-contract.test.mjs tests/static/documentation-policy.test.mjs
E2E_SESSION_USERNAME='dedicated-disposable-account' E2E_SESSION_PASSWORD='synthetic-discovery-only' npx playwright test --list tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
```

Discovery is not browser execution. Required deployed authenticated/proxy smoke uses the canonical
owning recipe when the selected environment and runtime credentials are available; report any
unrun acceptance explicitly. Do not mutate a real account to replace disposable coverage.

## 4. Measure the isolated boundary and decide the compiler setting

Owner: Pipeline, after the Senior freezes Backend source.

- Regenerate `Cargo.lock` from the approved manifest under sole generator ownership; inspect for
  only the expected Auth HTTP dependency-list changes and no external package updates.
- Prove Auth HTTP has no normal or `api-docs` path to File Platform and test-only dependencies stay
  out of the release graph. Keep the single context/session owner and expected downstream graph.
- Finish the isolated Auth boundary comparison before any Staff profile decision or adoption:
  two baseline Files source probes followed by two candidate Files probes at Staff CGU 16 on
  both sources, same canonical path, toolchain, target, dependency state and machine, with primes
  excluded. Capture dirty/fresh packages and Cargo wall times, restore the owned perturbation,
  and state the stage-order caveat. Attribute removed invalidation separately from CGU effects.
- Recommend default 16/32/64 only after the boundary comparison and complete Staff-only CGU and
  production runtime/output measurements. Apply the at-least-5% repeatable Cargo wall-time gain
  and per-workload equivalence criteria, twelve interleaved controls and bootstrap confidence
  intervals. Treat the 3% screen only as a screen; keep default 16 for uncertain or regressing
  results.
- If evidence supports adoption, Backend changes only `codegen-units` under
  `[profile.release.package.school-staff]` in `backend-school/Cargo.toml`; Pipeline updates only
  directly affected experiment/guard/operations owners. Rollback removes that package override.
  Root binary/external-crate CGU, optimization level 3, root `lto=off`, other package settings and
  runtime/API contracts remain fixed.

## 5. Reconcile documentation, freeze and deliver

Owners: Backend for `.rules`, `backend-school/README.md` and the focused profile-adapter recipe in
`docs/TESTING.md`, including its stale application-owned login/profile-handler wording, and any
accepted Staff package profile override in `Cargo.toml`; Pipeline for `docs/OPERATIONS.md` and
executable build/experiment policy; Senior
for Git and delivery. Shared documentation ownership is serialized by the Senior.

- Document the single Auth HTTP request-context owner, injected application profile command and
  Files/Auth provider authority without duplicating the implementation inventory.
- Run documentation policy, inspect the complete diff and unchanged migrations/permission/API
  artifacts, and have the Senior run `git diff --check` and `git status --short`.
- Pause all writers and mutating tools for the Senior's checkpoint/final freeze; acknowledge the
  pause and resume only after the Senior supplies the accepted checkpoint.
- Independent Reviewer covers the whole integrated diff, transaction/error behavior, authority,
  source/dependency/feature cuts, exact contract preservation and test integrity. Independent QA
  selects and executes proportional checks and assesses regression detection separately.
- Repair findings through their designated writers, refreeze and repeat affected checks. Record
  actual tested candidate/inputs, failed/unrun checks and measurement limitations in the PR.
- The Senior owns draft checkpoints, latest-main reconciliation, readiness, integration and any
  selected production Pipeline/acceptance. Remove these temporary workflow artifacts after the
  implementation PR records their completed outcome. Rollback follows the coherent source/profile
  reverts described in the design; no data or session-key rollback is needed.
