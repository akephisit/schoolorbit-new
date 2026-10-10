# Attendance reliability implementation

1. Repair default-role provisioning using migration 102, transactional role validation, typed mutation outcomes and post-commit cache invalidation. Cover missing/inactive/default/custom/ended roles and unrelated accounts.
2. Introduce a pure three-way merge helper for session result/note fields; add explicit conflict choices and reload selected detail. Preserve accepted discard behavior.
3. Reconcile committed scan IDs before new-capture checks, retain pending only for ambiguous/retryable errors, and verify no duplicate notifications or evidence deletion.
4. Extend typed report query/response with bounded pagination/search/category, regenerate API contracts, and update frontend paging plus complete explicit CSV exports. Bound roster DOM and render mobile cards.
5. Run focused and owned pipeline checks, review every changed producer/consumer, open and attach a PR with exact evidence. No production data mutations during tests.
