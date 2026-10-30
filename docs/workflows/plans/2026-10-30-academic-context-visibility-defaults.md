# Academic context visibility and defaults implementation

Design: [approved specification](../specs/2026-10-30-academic-context-visibility-defaults-design.md).

The Senior owns Git/integration and shared-checkout coordination. Backend owns Rust, migrations, backend fixtures, API generation, these workflow artifacts and operational documentation. Frontend owns its source/tests and waits for generated contracts. Independent Reviewer and QA receive the frozen integrated candidate. No agent performs production mutations.

1. Finish read-only migration/consumer assessment across the complete 001–145 timeline. Record authoritative date owners, explicit missing-link reconciliation and current function/view dependencies in task state.
2. Implement Core's date-free identities, visibility/default models and atomic revision-protected settings service/HTTP endpoint. Keep management reads, operational lifecycle and existing permissions independent.
3. Reconcile legacy Attendance calendars, Calendar scoped relationships and Behavior reset intent through new forward migrations. Preserve genuine dated facts and existing links; refuse ambiguous data before dropping boundary dates.
4. Update Admission, Promotion, Delivery, Timetable, Lifecycle, Assessment and Results consumers to explicit context relationships and actual domain dates. Remove boundary defaults and deleted-field APIs rather than add compatibility aliases.
5. Update School setup/topbar and affected date-owning forms under Frontend ownership. Regenerate `contracts/openapi/school-api.json` and generated TypeScript once Rust is stable; coordinate the generator with Frontend first.
6. Run focused validation and preservation/refusal/concurrency tests through the disposable database runner. Compile affected crates/consumers and use workspace coverage where the cross-domain impact requires it. Run Rust format, architecture guards, production panic Clippy gates, API generation/check/tests, affected frontend checks/browser workflows and documentation policy as selected by the Senior/QA.
7. Pause writers for a meaningful checkpoint, then continue under the new HEAD. Freeze the final candidate for independent review and QA; route repairs back to the responsible writer. Senior owns draft PR, freshness, integration and authorized delivery.

Test expectation changes are intentional: old year/term date payloads now fail unknown-field validation; date containment and lifecycle-derived context defaults are replaced by explicit context/date validation, visibility filtering and independently persisted defaults. Historical migration fixtures keep their original pre-cutover shapes; current-schema fixtures use date-free academic identities plus real domain dates/calendars.
