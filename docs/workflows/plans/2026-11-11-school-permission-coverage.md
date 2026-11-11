# School permission coverage implementation plan

Approved input: [permission coverage design](../specs/2026-11-11-school-permission-coverage-design.md). The user's instruction to fix every P1/P2 finding authorizes this bounded remediation.

1. Revalidate all fourteen findings against current main and trace domain policies, DTOs, API wrappers, loaders and controls. Agree exact grant checks and generated capability fields before dependent frontend changes.
2. Backend writer repairs grant escalation, feature updates, workflow grant eligibility and supervision result authorization in their owners. Add narrow action-context lookups and resource capabilities for delegated unit, offering and observation actions; preserve tenant and scope boundaries.
3. Frontend writer aligns staff grant/create flows and feature/workflow controls, consumes generated resource capabilities, fixes leave print ownership and timetable controls, and separates homeroom/student-year reads and independent year/term actions from protected editor choices. Align admission action loaders with their permitted context provider.
4. Add focused backend authorization fixtures and browser regressions for limited grants, own versus other resources, mixed scopes, deferred protected reads and revoked access. Correct stale test selectors only while preserving behavioral coverage.
5. Backend owner regenerates API artifacts after source coordination. Run generated contract checks, focused runtime/static/browser suites, Svelte/TypeScript checks, scoped lint and the production build under the pinned runtime. Serialize shared generators and heavy checks.
6. Senior freezes the integrated candidate for independent Reviewer and QA, routes repairs to the owning writer, and completes delivery under the repository workflow. Keep these files as implementation inputs rather than completion reports.
