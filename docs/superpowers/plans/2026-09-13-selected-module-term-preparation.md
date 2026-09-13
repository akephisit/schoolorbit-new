# Selected-Module Future-Term Preparation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preview and apply selected future-term configuration as independent drafts without changing the active context or copying operational/student data.

**Architecture:** Lifecycle owns the preparation command, immutable request receipt and source/target checksum. Each source module exposes a typed read/write provider operating inside the caller's transaction. Delivery is always rebuilt from the target curriculum; later modules consume explicit source-to-target mappings and report conflicts instead of guessing IDs.

**Tech Stack:** Rust/Axum/SQLx, PostgreSQL forward migration, generated OpenAPI/TypeScript, Svelte 5 with local shadcn controls.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md` (Future-term preparation and activation).

## Global Constraints

- Never modify migrations 001–078; add migration 079 only.
- Source term must be `closed`, target term must be a distinct `planning` term, and neither Topbar selection nor dates apply preparation.
- Selected modules are `delivery`, `assessments`, `timetable`, `exams`, and `supervision`. Activity registration behavior is carried by target curriculum activity offerings, not a duplicate standalone configuration module.
- Never copy scores, rosters, attendance, result locks/corrections, exam outcomes/invigilation assignments, teaching logs, observations/evaluations/reviews, publication state or entry-window state.
- Target offerings come from applicable target curriculum requirements. Teaching-period overrides reset to target catalog standards.
- Teacher, room, homeroom, group, bell-slot and date references require visible mappings; missing or colliding mappings are blocking findings, never fallback to source IDs.
- Apply writes only drafts in one transaction and is replay-safe by actor/request/input checksum.

---

### Task 1: Preparation receipt and typed workspace

**Files:**
- Create: `backend-school/migrations/079_academic_term_preparation_runs.sql`
- Create: Lifecycle `models/term_preparation.rs`, `services/term_preparation.rs`, focused tests
- Modify: sibling registrations

**Interfaces:**
```rust
enum TermPreparationModule { Delivery, Assessments, Timetable, Exams, Supervision }
struct PreviewTermPreparationInput { source_term_id, target_term_id, modules, mappings }
struct ApplyTermPreparationInput { request_id, source_checksum, source_term_id, target_term_id, modules, mappings }
struct TermPreparationWorkspace { context, modules, mappings, findings, can_apply, source_checksum }
```

- [x] RED: validate non-empty unique module selection, source/target state and ownership, bounded mapping lists, permission, deterministic checksum and zero writes during preview.
- [x] Add immutable apply receipt with request/actor/checksum, selected modules, mapping snapshot, counts and target IDs; protect with the official immutable trigger.
- [x] Implement one repeatable-read preview and one serializable apply under the tenant transition lock. Exact replay succeeds; stale preview or partial module failure rolls back all module drafts and the receipt.

### Task 2: Delivery target-curriculum provider

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`, `groups.rs`, `services.rs`
- Test: focused delivery preparation tests

**Interfaces:**
- `preview_term_preparation(tx, source_term, target_term, mappings) -> DeliveryPreparationEvidence`
- `apply_term_preparation(tx, actor, evidence, mappings) -> DeliveryPreparationOutcome`

- [x] RED: term-1-only source subjects are absent when target curriculum does not require them; applicable course/activity offerings and default groups map; actual periods reset to target standard; source publication/rosters are not copied.
- [x] Refactor existing curriculum preview/apply internals for caller-owned transactions without weakening standalone endpoints.
- [x] Copy only compatible teacher and preferred-room draft configuration after explicit mapping. Keep new offerings/groups/rosters draft and return stable source-to-target IDs plus conflicts.
- [x] Run focused Delivery tests GREEN and compare source rows before/after.

### Task 3: Assessment draft provider

**Files:**
- Create: focused child provider/test under `backend-school/src/modules/academic/services/assessment_service/`
- Modify: assessment service registration

- [x] RED: matching mapped course offerings copy independent four-phase plan rows and phase maxima; no score items, scores, confirmations, locks, results, coordinator fallback or open phase controls are copied.
- [x] Require a mapped target offering/teacher when copying a coordinator; report absent mapping as blocking instead of dropping the coordinator silently.
- [x] Apply draft plans idempotently only to pristine target offerings; existing conflicting plans block. Run focused tests GREEN.

### Task 4: Timetable draft provider with explicit collision mapping

**Files:**
- Create: child provider/test under `backend-school/src/modules/academic/services/timetable_version_service/`
- Modify: timetable service registration

- [x] RED: every source group/teacher/room/bell slot used by a copied block requires a target mapping; collisions in target room, teacher, homeroom or slot are blocking.
- [x] Create one draft target version and one-period blocks only after mappings pass. Preserve shared-activity grouping while mapping each linked group independently; do not publish or set effective dates.
- [x] Assert no source version/block changes and run focused tests GREEN.

### Task 5: Exam and supervision draft providers

**Files:**
- Add focused providers/tests under exam schedule and supervision services

- [x] RED: target date mappings must lie inside the target term; group/offering/assessment mappings must be complete. Exam rounds/days/sessions remain drafts and omit invigilators/outcomes.
- [x] Copy compatible supervision cycle configuration and target rules with mapped target dates/groups, but no observations, evaluators, responses, approvals or reviews.
- [x] Existing non-pristine target configuration blocks instead of being overwritten. Run both focused suites GREEN.

### Task 6: HTTP contracts and preparation workspace UI

**Files:**
- Modify: Lifecycle handler/router and OpenAPI registry
- Generate: API contracts
- Modify: `frontend-school/src/lib/api/academic-lifecycle.ts`
- Create: `frontend-school/src/lib/components/academic/lifecycle/TermPreparationDialog.svelte`
- Modify: term lifecycle page
- Test: `frontend-school/tests/e2e/academic-term-lifecycle.spec.ts`, API static tests

**Interfaces:**
```text
POST /api/academic/lifecycle/term-preparations/preview
POST /api/academic/lifecycle/term-preparations/apply
```

- [x] Contract RED covers exact DTOs/errors; generate contracts and wrappers.
- [x] Browser RED covers module selection, visible source/target scope, mapping blockers, explicit apply confirmation, stale refresh, reader denial and no Topbar/context mutation.
- [x] Build a compact step dialog: select modules, inspect mappings/findings, confirm draft counts. Preserve request ID on uncertain retry and label every output as draft.
- [x] Run Svelte autofixer, one-worker Playwright, frontend check/static/API gates and focused backend tests.

### Task 7: Release integration and exact-build boundary

- [x] Run the complete `.rules` matrix serially and verify no scores/results/rosters/source timetable or observation records changed in preparation tests.
- [x] Rebase/merge current `origin/main` only after preserving unrelated shared-session files, resolve generated-contract changes from source, and obtain a final code review.
- [ ] Commit and push the complete branch to `main` only after local gates. Then verify the exact deployed build on `sandbox.schoolorbit.app` with owned `E2E-LIFECYCLE-` fixtures; do not report mocked browser coverage as live evidence.
