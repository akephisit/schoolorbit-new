# Backend-school final-crate LTO-off implementation plan

> Execute inline, one task at a time, as requested by the user. Do not delegate. The user subsequently authorized committing and pushing the verified changes to main.

**Goal:** Reduce source-only release build time without importing the rejected image-engine extraction.

**Approved design:** Keep the existing application, dependency graph, release optimization level, cargo-chef layer, runtime image, and deployment gates. Pass `-C lto=off` only to the final backend-school crate via `cargo rustc`; do not change global Rust flags or Cargo profiles. Use the isolated `perf/backend-school-lto-off` worktree based on `main`.

**Tech stack:** Rust, Cargo, Docker BuildKit, Node static tests, disposable PostgreSQL.

## Tasks

- [x] Adapt the existing deployment guard to the final-crate command; observe failure before changing the Dockerfile.
- [x] Change only the final backend-school build invocation and explain the scope and rollback in `docs/OPERATIONS.md`.
- [x] Run the focused deployment and documentation guards sequentially.
- [x] Verify the release test harness with matching Rust and final-crate-only LTO flags through `scripts/test_backend_school.sh --release --locked -- --test-threads=1`; do not use production databases. A local ignored rustc workspace wrapper may supply the final-crate flag for the test harness.
- [x] Run `cargo fmt --all -- --check`, `cargo test --test static_architecture`, and `cargo check` from backend-school.
- [x] Run frontend-school lint, `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`, and `npm run test:static` sequentially.
- [x] Build the real Docker runtime image locally, inspect its exported contract, and preserve production readiness/smoke gates. Local tests do not constitute deployed-proxy acceptance.
- [x] Review the full diff, `git diff --check`, and `git status --short`; report any unavailable checks without claiming release readiness.

The user approved the separate [test-infrastructure repair plan](./2026-09-09-release-test-infrastructure.md)
to remove the runner's temporary-storage limit and update the stale runtime-policy
fixture. Keep that repair test-only; do not edit applied migrations or suppress cases.

## Rollback and acceptance

Restore the previous final `cargo build --release --bin backend-school --timings` invocation to undo the optimization. No schema, API, permission, or runtime configuration changes are required. Local timing evidence is not a promise of GitHub build/push or deployment duration; uploads, realtime, concurrency, and deployed-proxy smoke remain release acceptance concerns.
