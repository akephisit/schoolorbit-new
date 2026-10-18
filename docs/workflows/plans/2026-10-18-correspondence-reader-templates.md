# Correspondence reader and templates implementation plan

Status: Senior-approved [design](../specs/2026-10-18-correspondence-reader-templates-design.md); production edits follow coordinated assignments.
Base: `ec6436038f96f06e2374e45b0851181737174505`; branch `feat/correspondence-reader-templates`.
Use one shared checkout. Senior owns Git, generated contracts, dependency/tooling operations, and the shared Cargo/build lane.
Backend and Frontend writers own disjoint assigned sources; Reviewer and QA independently assess the frozen candidate.

## Phase 1: settle interfaces and persistence

- Confirm the complete nine requested outcomes and the design's formatting-only scope; no counter reset or manual number override.
- Recheck migration maximum before reserving additive migration 127 and inspect fixture migration registration/architecture requirements.
- Agree receipt DTO, nullable detail field, default-omitting settings extensions, annotation border, frozen register label, and actual identity context.
- Backend owns `school-workflow` correspondence/process/directed/work owners, HTTP handlers, migration, and focused Rust/database fixtures.
- Senior owns `backend-school/src/api_contract.rs` integration and generated JSON/TypeScript production, coordinating any writer edit to shared registration files.
- Frontend reads contracts/owners now; it starts dependent edits only after Senior confirms the generated interface and file ownership.
- Acceptance: concrete DTOs and ledger/backfill invariants are reviewable without a production write or hidden UI persistence workaround.

## Phase 2: backend owner changes

- Add the first-read ledger with book/user uniqueness and immutable-history backfill; unmappable/ambiguous Read rows refuse migration/cutover with aggregate findings and preserved audit.
- Implement authorized revision-bound POST acceptance and nullable detail receipt; repeated calls return the original receipt without workflow/history churn.
- Add the narrow process-owned read projection operation and bridge explicit Read; retain slot authority and first timestamps, without acknowledgment/completion changes.
- Extend typed register profiles with bounded formatting and stamp templates while preserving old saved profile values and old receipt serialization.
- Freeze new formatted labels on create without changing numeric reservation, register year, existing labels, or uniqueness.
- Add bordered text annotation composition using canonical rotated/cropped source geometry, existing fitting, and Thai shaping.
- Resolve identity through existing providers/narrow context (`actorDisplayName`, `signerTitle`); use explicit/known Thai position titles, leave unknown titles editable and required when used, and never substitute raw codes/RBAC roles or alter accepted signing evidence.
- Add focused tests for ledger races/repeats/denial, legacy audit preservation/backfill, projections, old hashes, formatting preservation, and rendering.
- Senior inspects the API boundary, generates contracts, saves the first meaningful checkpoint, and maintains one draft PR within authorized delivery scope.

## Phase 3: frontend reader and forms

- Frontend owns correspondence API wrapper/helpers/components, My Work presentation, and relevant browser/helper fixtures.
- Translate My Work's source badge; keep API module identifiers and unrelated module behavior stable.
- Add list open/read affordance and current-document automatic loading with stale-response/abort cleanup; report read only after successful page rendering.
- Move the mobile document ahead of metadata/actions/history; retain usable retry, history selection, explicit acknowledgment, and unsent local state.
- Present the bootstrap draft/version explanation accurately; do not delete revisions or pretend send uploaded another PDF.
- Apply unconfigured routing defaults and initial organization selector without resetting accepted settings or current user choices.
- Add per-register number-format preview, three stamp presets, saved-template editing, token expansion, and positioned text/border controls.
- Add inline accessible errors and reveal/focus/scroll behavior across touched forms; preserve files, mutation intents, and signature consent.
- Add browser coverage that observes actual render-success callback behavior, including failures/cancellation and repeated opens.

## Phase 4: verification and independent acceptance

- Follow [canonical correspondence recipes](../../TESTING.md#correspondence-and-reusable-processes); Senior schedules one shared Cargo lane and disposable database fixtures.
- Run the focused Workflow correspondence database target and renderer unit/fixture target; inspect synthetic Thai/rotated bordered PDFs in the real renderer.
- Verify both default and `api-docs` affected Rust targets, endpoint/schema registration, generated contract checks, and directly affected consumers.
- Check migration retry/backfill preservation, first-read concurrency and explicit-read compatibility; ensure no broadening from viewer rights to assignment authority.
- Run owning Svelte tooling, changed-file lint/format, relevant static/helper tests, and correspondence directed/workflow/usability browser cases at mobile/desktop sizes.
- Follow the canonical split between preview-based browser cases and development-import identity/realtime cases; report actual executions and unrun requirements separately.
- Update canonical Testing/Operations only for new commands or rollout/recovery facts; avoid duplicating durable standards in temporary artifacts.
- Senior freezes the complete candidate and assigns independent Reviewer and QA; writers pause until findings are returned and repairs are assigned.
- Reviewer verifies ownership, authorization, data preservation, replay serialization, lifecycle/cancellation, geometry, and full requested outcome coverage.
- QA records exact commands, tested tree/environment, results, and browser/PDF evidence; fixes belong to the relevant writer, followed by affected rechecks.

## Phase 5: integration and completion

- Senior refreshes main under the existing pause/clean-checkpoint rules, resolves integration through the owning writer, and revalidates affected evidence.
- Checkpoint pauses require writer acknowledgments; only Senior stages, commits, pushes, opens/updates the PR, and handles readiness/integration.
- Preserve all historical evidence and recover forward after feature use; deploy matching backend/contracts/frontend through existing maintenance gates.
- Track authorized CI/integration/deployment acceptance to its terminal result and describe remaining concrete limitations honestly.
- Record the completed implementation in the PR, then remove these temporary design/plan inputs under the repository documentation policy.
