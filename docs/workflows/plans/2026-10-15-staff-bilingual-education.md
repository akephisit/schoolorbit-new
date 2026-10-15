# Staff profile implementation plan

Approved design: ../specs/2026-10-15-staff-bilingual-education-design.md

1. Inspect existing user-name reads/writes, personnel migration timeline, existing create/edit/profile lifecycles and tests; preserve narrow crate ownership and exact capabilities.
2. Add additive name columns and a shared resolved-name read projection. Add education rows/revision, deterministic backfill and fresh reconciliation/cleanup gates. Wire completion evidence into existing migration acceptance.
3. Implement typed Staff name validation and education collection operations within existing transactions. Update DTOs, profile completeness, list search/order, overview highest-level computation and direct name readers. Keep raw language fields in the editor response and preferred name in display projections.
4. Generate Rust-owned OpenAPI/TypeScript. Extract shared bilingual name and multiple-education fields; update draft normalization/version migration and create/edit payloads.
5. Redesign the three staff routes with shared primitives, compact responsive sections and complete permission-scoped data. Preserve streaming, independent history/achievement retries, lazy choices, mutation ownership and URL return context.
6. Add focused behavioral and migration cases; verify affected packages/identity consumers, API contracts, frontend check/lint and synthetic Chromium workflows. Record exact evidence on the final tree; review all changed owners.
7. Update durable rules and canonical test/operation recipes if required, remove these temporary documents, commit/push a ready PR, attach it and follow merge/deployment/acceptance without rerunning unchanged suites.
