# Mutation Performance Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make supervision saves visibly responsive, avoid unnecessary full reloads after mutations, and codify bulk write/local state update rules for future modules.

**Architecture:** Optimize the supervision backend by replacing per-row evaluation response writes with typed bulk validation/upsert helpers. Update the supervision frontend so mutations maintain specific loading states and replace only affected in-memory resources from API responses instead of calling `refreshAll()` after every save. Add static guards and `.rules` guidance so future feature work follows the same pattern.

**Tech Stack:** Rust + Axum + sqlx/PostgreSQL, SvelteKit 5 + TypeScript, local static architecture tests, frontend Node static tests.

---

### Task 1: Add Regression Guards

**Files:**
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`

- [x] **Step 1: Add backend supervision bulk-write guard**

Add a static test that reads `src/modules/supervision/services.rs` and requires named bulk helpers for template items and evaluation responses. It should also reject the old `for response in input.responses { ... execute(pool) ... }` pattern.

- [x] **Step 2: Add frontend supervision local-update guard**

Extend the existing supervision frontend static test so it requires specific saving flags and helper functions such as `replaceTemplate`, `replaceObservation`, and `refreshTemplates`, while rejecting `await refreshAll()` inside `createTemplate()` and `saveEvaluation()`.

- [x] **Step 3: Verify RED**

Run `cd frontend-school && npm run test:static` and `cd backend-school && cargo test --test static_architecture supervision`. Expected: fail before implementation.

### Task 2: Bulk Backend Writes

**Files:**
- Modify: `backend-school/src/modules/supervision/services.rs`

- [x] **Step 1: Refactor template inserts**

Generate section UUIDs in Rust, bulk insert sections with `QueryBuilder::push_values`, then bulk insert all items with `QueryBuilder::push_values`. Keep the existing transaction and response shape.

- [x] **Step 2: Refactor evaluation save**

Validate response item membership/range in one query, dedupe duplicate template item responses with latest-answer-wins semantics, then bulk upsert responses with `QueryBuilder::push_values ... ON CONFLICT (evaluator_id, template_item_id) DO UPDATE`. Keep submitted evaluations immutable.

- [x] **Step 3: Add focused helper tests**

Add unit tests for helper functions that flatten section/item payloads and response payloads for bulk operations.

### Task 3: Frontend Local Updates and Loading UX

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte`

- [x] **Step 1: Split loading state**

Replace the global mutation-only `saving` behavior in template/evaluation flows with `savingTemplate` and `savingEvaluation`.

- [x] **Step 2: Replace affected state only**

After template create/update, replace only `templates`. After evaluation save/submit, replace only the returned observation in `observations`. Keep manual refresh available for a full reload.

- [x] **Step 3: Add visible loading buttons**

Use spinner/text on template save, draft save, and submit buttons so users can see ongoing work.

### Task 4: Project Rules

**Files:**
- Modify: `.rules`

- [x] **Step 1: Add mutation performance rules**

Document that multi-row mutations should use bulk insert/upsert when practical, action-specific loading states must be visible, and mutation success should update local affected state before falling back to full reload.

### Task 5: Verification

**Commands:**
- `cd backend-school && cargo fmt`
- `cd backend-school && cargo test --test static_architecture supervision`
- `cd backend-school && cargo test modules::supervision::services::tests --bin backend-school`
- `cd backend-school && cargo check`
- `cd frontend-school && npm run test:static`
- `cd frontend-school && npm run check`
- `cd frontend-school && npm run lint`
- `git diff --check`

Status: all listed verification commands completed successfully.
