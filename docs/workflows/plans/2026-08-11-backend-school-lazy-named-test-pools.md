# Backend School Lazy Test Pools Implementation Plan

**Goal:** Prevent shared and named test pools from consuming PostgreSQL connections while waiting for the existing migration lock.

**Scope:** `backend-school/src/test_helpers.rs` plus this design and plan. No runtime code or migration changes.

## Task 1: Make test application pool creation lazy

- [x] Add a RED unit test proving a search-path pool can be constructed with an unreachable but valid PostgreSQL URL.
- [x] Add a private lazy pool constructor that preserves maximum connections and `after_connect` search-path setup.
- [x] Use it from `create_named_test_pool_with_max_connections`; retain eager schema reset.
- [x] Use it from `create_test_pool`; retain the one-time eager shared-schema reset and keep pools local to each Tokio test runtime.
- [x] Run focused helper tests and representative named database tests.
- [x] Run the complete `backend-school` binary test suite twice and require both runs to pass.
- [x] Run the backend-school formatting, architecture, and compile checks required by `.rules`.
