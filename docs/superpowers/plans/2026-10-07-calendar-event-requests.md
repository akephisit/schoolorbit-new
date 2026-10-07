# Calendar event requests implementation

1. Preserve existing event facts and academic context provenance in a new migration; retire event academic linkage while retaining classroom target enrollment scope.
2. Add request models, own/school authorization policy, bounded list, request validation and atomic approve/reject services in school-calendar. Reuse transactional event persistence and notification intent.
3. Register typed HTTP routes and OpenAPI; generate permissions and API contracts.
4. Add a short request form and date defaults, a route for request history/review, and remove academic prerequisites from calendar views. Load manager target options by event date.
5. Verify policies, validation, approval/rollback/concurrency and migration preservation on disposable fixtures. Run focused calendar and route tests, frontend lint/check/static and Playwright mobile/desktop light/dark acceptance; run permissions/API generators and backend matrix.
6. Review final diff. Integrate only after all required checks pass; report unavailable dependencies and leave a reviewable branch if they do not.
