# Remembered School devices implementation

Approved design: `docs/workflows/specs/2026-11-23-remembered-school-devices-design.md`.

1. Auth writer owns migration 168, Auth persistence/crypto/service/HTTP, Auth contract registration, focused Auth tests, this design/plan and narrow rules/operations updates. Preserve finite sessions, add explicit remembered grants and acknowledged idempotent restore, then integrate device listing/revocation, password and deactivation lifecycle.
2. Notifications writer owns Push persistence/admission/cleanup/publisher and focused tests. Consume migration 168's exclusive-branch eligibility view; promote only proven active grant registrations and retain existing subscription version/token/installation fences.
3. Frontend writer owns Auth bootstrap recovery, account-security labels/null expiries, durable Push matching and focused browser/static/runtime tests. Keep recovery single-flight and generation fenced; never replay arbitrary business mutations or persist Auth/CSRF secrets.
4. Coordinator owns shared Git, generators and serialized Cargo. Freeze and checkpoint bounded work, generate OpenAPI/TypeScript contracts, then assign independent Review and QA. Writers repair findings within their owners; Coordinator completes authorized PR/integration/deployment steps.

Verification: scoped rustfmt and diff checks during writing; focused `school-auth` and `school-auth-http` tests, composed Auth HTTP integration tests, notification lifecycle tests, API generation/checks, architecture checks, applicable frontend Auth/Push static/runtime and dedicated-account Playwright coverage. Broaden only for shared-boundary uncertainty or failures under `.rules` section 11. Deployed readiness/proxy smoke/browser acceptance follows `docs/TESTING.md` and `docs/OPERATIONS.md`. A saved checkpoint does not imply completed checks.
