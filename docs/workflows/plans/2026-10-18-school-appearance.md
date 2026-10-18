# School appearance implementation plan

1. Extend Administration settings with closed typed DTOs, strict validation, migration 127 and a bounded read/atomic revision update service. Preserve logo methods and existing public school information.
2. Register public and protected routes under current tenant/session middleware and existing settings permissions. Register OpenAPI paths/schemas and regenerate the School contracts with `npm run generate:api-contracts`.
3. Implement frontend render-blocking tenant CSS, self-hosted font readiness, typed settings controls and visibility revalidation through existing owners. Keep user light/dark preference local and avoid additional tenant caches.
4. Run focused Rust unit/database/HTTP cases through the owned disposable PostgreSQL fixture; check affected crate and application with default and api-docs feature modes. Run contract generation/check/tests and focused frontend/static/browser checks. Record actual results in the PR/chat.
5. Pause writers for Senior-owned checkpoints. Freeze the integrated candidate for independent Rules/Security review and QA; repair findings and rerun affected checks. Keep PR 132 draft without merge, deploy or live migrations.
