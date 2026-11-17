# Budget workspace redesign implementation plan

Design: [approved workspace design](../specs/2026-11-17-budget-workspace-redesign-design.md).

1. Workspace writer creates budget-local navigation and filter components, then adds independent catalog promises to the three list loaders and typed wrapper props. Verify URL state, read guards and independent failures.
2. Workspace writer redesigns overview/reports and list results with truthful totals and clear tasks. Detail/form writer concurrently updates disjoint detail and form owners using the agreed navigation interface and existing lifecycle owners.
3. Workspace writer reorders annual settings presentation while preserving per-year draft identity, save boundaries, receipt semantics, closure and deletion confirmations.
4. Workspace writer updates existing budget browser expectations for approved navigation/presentation changes and adds focused navigation/filter regression coverage. Retain all financial and delayed-navigation cases.
5. Senior freezes source and coordinates checks: scoped formatting/ESLint, `npm run check`, `npm run build`, focused budget helper and route-loading static tests, and the three budget Playwright suites against a ready loopback frontend. Inspect actual mobile/desktop light/dark screenshots and geometry.
6. Independent Reviewer and QA assess the complete frozen candidate. Writers repair assigned defects, then repeat affected checks/review. Senior records outcome in the PR and removes these temporary workflow artifacts after that record exists.

No API/permission generation or backend/database verification is needed unless scope changes. Writers do not run shared generators, broad formatters/builds or mutate Git. Existing browser fixture API interception is synthetic evidence, not production financial acceptance.
