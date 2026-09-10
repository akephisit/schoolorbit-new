# Future Term Planning Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Execute tests serially.

**Goal:** Allow a school to prepare another planning term, including summer, during an active academic year without changing the active context or opening entry windows.

**Architecture:** Core remains the only writer of academic terms. The year-row lock serializes term configuration with future year lifecycle transitions. Annual inclusion implies that the term blocks annual closure, enforced at the service and database boundaries.

**Tech Stack:** Rust, SQLx, PostgreSQL, Svelte 5 and local shadcn; existing generated term API unchanged.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md` (future-term preparation and term inclusion).

## Constraints

- New terms remain `planning`; dates and Topbar selections never activate them.
- Do not copy scores or open assessment, gradebook, or learner entry windows.
- Unknown planned end date stays supported; known dates must fit the owning year.
- Existing active/closing/closed term configuration is not made ordinarily editable.
- Preserve previous migrations; use migration 067 for the inclusion invariant.

## Tasks

- [x] Add an integration test in `backend-school/src/modules/academic/core/services_tests.rs`: create summer in the active fixture year with `planned_end_date: None`; assert `planning`, unchanged active year/term IDs, and disabled controls. Update this new planning term successfully while preserving the active term.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh future_term -- --test-threads=1`; observe the current planning-year-only rejection.
- [x] In `core/services/years_terms.rs`, allow planning-term creation/update only when the year is `planning` or `active`, under the existing year lock. Continue rejecting ready/closing/closed/archived years and non-planning term updates. Add focused tests for allowed and denied year states.
- [x] Test that `included_in_year_result: true` with `blocks_year_closure: false` is rejected before writing; excluded terms may independently block closure. Add a shared boolean validator to create/update services.
- [x] Add `067_term_annual_inclusion_guard.sql`: normalize inconsistent existing flags, increment changed row versions, and add `CHECK (NOT included_in_year_result OR blocks_year_closure)`. Verify the constraint with an actual inconsistent insert/update in disposable PostgreSQL.
- [x] Align the term form with allowed year states and annual flag dependencies. Keep history visible for closed years; key the editor by year so switching years cannot retain another year's edit. Test the helper and the real form with mocked local-browser API calls.
- [x] Run Svelte analysis, frontend lint/type/static checks and one-worker Playwright. Browser fixtures must not mutate a live tenant.
- [x] Run the full Core tests, backend architecture, formatting and compile checks. Recheck generated API artifacts (the existing HTTP shape must not change). Review the diff and commit this coherent Core change on the feature branch.

## Verification

```bash
CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh modules::academic::core -- --test-threads=1
```

Run backend and API gates from `.rules` section 11 serially. This change prepares draft terms only; closing, reopening and activation require the remaining lifecycle readiness and transactional write guards.
