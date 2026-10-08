# Backend HTTP compilation boundaries

Approved by the user in chat on 2026-10-08. Baseline: refreshed main 75b46a68.

Extract school-certificates-http and school-academic-http as concrete Axum handler
owners, school-navigation for menu/route/feature persistence, and
school-notifications for notification persistence, publishing, events and Web Push.
Application startup, router composition, middleware, complete OpenAPI composition,
scheduling and cross-domain adapters remain in backend-school. HTTP handlers use
concrete narrow state selected by application-owned FromRef implementations; no
internal crate depends on backend-school or accepts its complete AppState.

Central session/tenant request context has one authentication-owned HTTP adapter.
Academic realtime and existing result-lock/lifecycle ports are injected from the
application, preserving authorization, transaction and post-mutation event order.
Certificate file-deletion orchestration uses an application-owned narrow port.
Move domain path declarations with handlers; root composes the unchanged complete
document. Preserve exact API JSON, permissions, runtime settings and migrations.

Measure baseline and candidate with the pinned production Docker toolchain,
unchanged compiler profile, two source-changing samples per application/domain
case on the same runner. Prime changed workspace graphs separately. Record Cargo
units/time, binary size and persistent reuse; do not equate Cargo time with complete
deployment time. Reject or revise regressions before integration.

Verification includes workspace checking, all affected package tests, architecture,
API and permission contracts, relevant disposable database fixtures, frontend
static/lint/type checks, auth/realtime guards, CI and deployed authenticated smoke
plus release acceptance. No schema or data cutover; rollback is the preceding
application image with the same schema.
