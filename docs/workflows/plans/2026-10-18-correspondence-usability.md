# Correspondence usability implementation plan

Approved design: [correspondence usability](../specs/2026-10-18-correspondence-usability-design.md). Execute under the current `.rules` and the user's approved scope.

1. Backend writer extends the existing list query/typed DTOs, shared authoritative list eligibility/counts/projection and registration search. Add draft-only metadata to the revision transaction and preserve old request hashes. Register schemas and generate API artifacts once the typed contract is in place.
2. Frontend writer implements the typed list views, three-step wizard and draft recovery, upload retry, recipient selection, timeline/main action, direct signature placement and template presets/copy in its assigned UI and tests. Use generated types and existing transport/File Platform providers.
3. Backend writer adds focused Workflow database cases for counts, filters, pagination, permissions, frozen snapshots and action parity, plus metadata validation/atomicity/access/replay. Update only the canonical testing and operational procedures affected by the change.
4. Verify focused Workflow/default/api-docs consumers, generated API generation/check/tests, frontend check/lint/static/browser workflows and documentation policy. Serialize Cargo/Docker/heavy builds, reuse the compatible shared target and disposable PostgreSQL fixture, and record exact results in chat/PR.
5. Senior pauses writers, inspects and saves checkpoints, freezes the integrated candidate, obtains separate read-only Reviewer and QA acceptance, assigns any repairs, then handles the draft PR and authorized integration/release gates. Remove these workflow inputs after the completed outcome is recorded in the implementation PR.

No migration, dependency/toolchain upgrade, parallel retry queue, notification worker or permission expansion is planned. Root owns all Git mutations and PR/integration actions; writers edit only assigned disjoint paths. Backend is the sole contract generator and heavy Rust/fixture owner during implementation.
