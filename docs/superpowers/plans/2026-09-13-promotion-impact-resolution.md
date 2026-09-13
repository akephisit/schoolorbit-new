# Promotion Impact Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let authorized academic staff explicitly retain or replace an executed promotion decision after an official result correction, while preserving the original run and correction history.

**Architecture:** Lifecycle owns immutable resolution receipts and rechecks current correction evidence under the tenant transition lock. Core owns the narrowly scoped reconciliation of receipt-owned planned student-year and placement rows. The command is intentionally limited to a planning target year; after activation, staff may acknowledge the impact but cannot silently rewrite active enrollment through this workflow.

**Tech Stack:** Rust/Axum/SQLx, PostgreSQL forward migration, generated OpenAPI/TypeScript, Svelte 5 and local shadcn components.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-year-promotion-design.md` (Persistent runs and safe retries).

## Global Constraints

- Never modify migrations 001–077; add migration 078 only.
- Never mutate an executed run item, execution receipt, correction row, or historical annual snapshot.
- Resolution is explicit, requires a non-empty reason and a separate school-scoped permission, and never runs merely by viewing data.
- `keep_existing` writes no Core enrollment data. `replace_decision` may reconcile only rows owned by the execution receipt or earlier resolution receipts and only while the target year is `planning`.
- Recompute exact unresolved impact evidence in the write transaction. Stale checksums, reused request IDs with different input, foreign actors, or already-resolved impacts return conflict without partial writes.
- No plaintext national IDs or unnecessary student identifiers in receipt/audit JSON.

---

### Task 1: Immutable resolution storage and existing correction permission

**Files:**
- Create: `backend-school/migrations/078_promotion_impact_resolutions.sql`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Produces one immutable row per `(item_id, correction_id)` with `request_id`, actor, input checksum, resolution kind, optional replacement decision, reason, source checksum and typed outcome JSON.

- [ ] RED: add static/schema assertions for the immutable trigger, exact checksums, foreign keys and one-resolution-per-impact uniqueness. Assert the command uses the already generated `academic_promotion.correct.school` capability introduced by migration 072.
- [ ] Run the focused static/schema test and observe the missing migration failure.
- [ ] Add forward migration 078 without changing the existing permission contract.
- [ ] Run the focused schema assertions GREEN.

### Task 2: Core reconciliation of receipt-owned planned targets

**Files:**
- Create: `backend-school/src/modules/academic/core/services/promotion_reconciliation.rs`
- Create: `backend-school/src/modules/academic/core/services/promotion_reconciliation_tests.rs`
- Modify: `backend-school/src/modules/academic/core/services.rs`, `promotion_targets.rs`

**Interfaces:**
- Consumes the immutable source context, original/current owned target IDs, target year and a validated `PromotionDecisionInput`.
- Produces `PromotionReconciliationOutcome { target_student_year_id, target_placement_id, source_row_version }` without committing its transaction.

- [ ] RED: real fixtures cover target-to-target grade/program/room change, target-to-hold, hold-to-target, graduate/transfer source-status reconciliation, capacity conflict, foreign target ownership and non-planning target rejection.
- [ ] Run `scripts/test_backend_school.sh -- promotion_reconciliation --test-threads=1` and observe missing implementation.
- [ ] Implement lock-ordered reference validation and reconciliation. Reuse the existing destination rules while allowing only the command's explicitly owned target row. End obsolete planned placements instead of deleting them; retain every receipt FK.
- [ ] Assert source results, source placements, annual snapshots and original promotion receipts remain byte-for-byte unchanged; run focused tests GREEN.

### Task 3: Transactional resolution command and pending projection

**Files:**
- Create: `backend-school/src/modules/academic/lifecycle/models/promotion_impact_resolutions.rs`
- Create: `backend-school/src/modules/academic/lifecycle/services/promotion_impact_resolutions.rs`
- Create: `backend-school/src/modules/academic/lifecycle/services/promotion_impact_resolution_tests.rs`
- Modify: sibling model/service registration, `promotion_impacts.rs`, `promotion_opening.rs`

**Interfaces:**
```rust
async fn resolve_promotion_impact(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
    impact_id: Uuid,
    input: ResolvePromotionImpactInput,
) -> Result<PromotionImpactResolution, AppError>;
```

- [ ] RED: readers are denied; keep-existing writes only a resolution; replace-decision writes Core plus resolution atomically; stale evidence, duplicate/foreign impact and reason validation fail closed.
- [ ] Add rollback injection for audit and receipt insertion. Exact replay by actor/input returns the original outcome; concurrent equal commands create one receipt, while reuse with different input conflicts.
- [ ] Join resolution rows into the impact workspace, expose pending/resolved status, and make opening readiness count only current unresolved `(item, correction)` evidence. A later correction creates a new pending impact.
- [ ] Run `scripts/test_backend_school.sh -- promotion_impact promotion_reconciliation opening_readiness --test-threads=1` GREEN.

### Task 4: Typed API and read-first resolution UI

**Files:**
- Modify: Lifecycle handler/router, `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`, `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academic-promotion.ts`
- Modify: `frontend-school/src/lib/components/academic/lifecycle/PromotionImpactsDialog.svelte`
- Test: `frontend-school/tests/e2e/academic-promotion-runs.spec.ts`, API contract static tests

**Interfaces:**
```text
POST /api/academic/lifecycle/promotion-runs/{run_id}/impacts/{impact_id}/resolve
```

- [ ] Contract RED requires typed camel-case input/output and 200/400/401/403/404/409/422/500 envelopes; generate contracts and add the concrete wrapper.
- [ ] Browser RED covers reader inspection without mutation controls, authorized keep-existing with reason, replacement decision using existing grade/program/homeroom selectors, stale 409 refresh and retained resolved history.
- [ ] Extend the existing lazy impacts dialog with pending/history sections and a nested mobile-safe resolution dialog. State clearly that replacement works only before the target year opens; do not suggest the original run is edited.
- [ ] Run Svelte autofixer on every touched component, focused one-worker Playwright, frontend check/static/API gates and focused backend tests.

### Task 5: Whole-slice verification

- [ ] Run the `.rules` change matrix serially, inspect migration/API/permission diffs, and verify no unrelated exam files were changed.
- [ ] Mark this plan complete only when pending impacts block activation, explicit resolutions persist across reload, and exact source history remains unchanged.
