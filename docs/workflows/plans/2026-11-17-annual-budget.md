# Annual budget implementation

Approved design: [Annual budget control](../specs/2026-11-17-annual-budget-design.md).

1. Backend owns migration 162, year models/settings, annual cap enforcement, year-first locks, activity dates, HTTP contracts and generated API artifacts. Data owns migration 163, Budget deletion, Workflow purge and private privilege exclusions. Frontend owns year settings/funding/allocation/lifecycle UI and scoped catalog/project consumers. Senior alone owns Git.
2. Preserve historical JSON and stable IDs through year overlays. Backfill annual authorization and actual receipts as unknown. Keep all legacy years open until an explicit guarded close; never infer a closure from the former selection flag.
3. Replace global settings APIs with year creation/index/detail/save/funding/close/delete, exact revision/idempotency checks and capabilities. Add project hard delete only for never-approved projects.
4. Apply fiscal-year locks before all operational mutations and validate final approved commitments against school/source/unit envelopes. Approved cross-unit transfers/loans/repayments move balanced unit quotas atomically and advance the annual revision/audit without changing source authority or reserve; validate fully allocated units and stale settings races. Add required new activity dates and retain unknown historical response dates.
5. Update focused Budget fixtures for the approved canonical contract and add migration, concurrent approval, cap reduction, copy isolation, date locking, close, permission and deletion regressions.
6. Serialize Cargo/generators under Backend ownership. Run the owning disposable PostgreSQL Budget suite, affected Workflow/migration tests, scoped compile/format/architecture checks and generated API/permission checks. Frontend verifies typed consumers and affected browser flows. Freeze the candidate for independent Reviewer and QA acceptance; repairs return to the owning writer.

No historical financial amounts or dates are inferred. No applied migration or unrelated provider is changed. Workflow artifacts are removed after the implementation PR records the completed outcome under the repository policy.
