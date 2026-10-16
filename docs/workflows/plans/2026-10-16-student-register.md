# Student register implementation plan

Design: [Permanent student register](../specs/2026-10-16-student-register-design.md).

1. Backend: add the immutable forward migration and Students-owned typed administrative status;
   preserve historical data and use pending for future inserts. Separate staff register DTOs from
   mandatory-year self/parent profiles; implement year-independent reads and consistent own/assigned
   access. Add bounded validation and tri-state permanent profile edits.
2. Backend: add focused database/schema/service coverage for migration defaults, registration
   visibility, status/account separation, provenance, selected-year fields, allowed/denied scopes,
   PII, nullable updates, rollback and annual-history preservation. Update legitimate API schema
   expectations to the approved contract.
3. Frontend: consume generated DTOs; rebuild the year-independent directory and compact new/edit
   forms, preserve valid detail source context and independently authorized academic history.
   Add focused helper/static and browser regressions for desktop/mobile, errors and disposal.
4. Coordinate the shared generator and single Rust owner. Run Students package checks in both
   documentation modes, focused root student/profile/mutation and lifecycle tests through
   `scripts/test_backend_school.sh`, API generation/check/tests and the affected frontend checks.
   Run documentation policy, architecture guards and `git diff --check` for the candidate.
5. Senior freezes the integrated candidate for independent Reviewer and QA. Assign fixes to the
   owning writer and repeat invalidated checks. Senior owns checkpoint commits and reports actual
   results and limitations, then handles authorized PR delivery, eligible integration and selected
   Pipeline acceptance. Writers never perform those operations independently.
