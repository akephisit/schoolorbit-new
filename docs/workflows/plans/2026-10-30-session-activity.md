# Session activity implementation plan

1. Extend auth persistence with migration 145, deterministic pending derivation and side-effect-free
   credential validation. Preserve lock ordering, digest availability and password/revocation fences.
2. Add guarded activity and pending acknowledgment operations to the auth owner. Carry session
   deadlines through authentication/login, update cache invalidation/race handling and rejection logs.
3. Bind ordinary HTTP/realtime to validation-only checks. Register typed activity and session state
   DTOs, enforce expected-session fencing before mutation, and generate OpenAPI/TypeScript artifacts.
4. Implement the central frontend controller in the separately assigned frontend paths, using
   server deadlines, activity coalescing, auth-generation fences and `/me` reconciliation on 409.
5. Update durable policy/operations/test recipes. Run focused auth repository/service/cache/schema,
   auth HTTP/root session HTTP, affected realtime and architecture tests with owned PostgreSQL;
   generate/check/test API contracts and run frontend controller/browser checks. Senior freezes
   the combined candidate for independent security review and QA before delivery.

Backend writer owns auth, assigned transport/composition/tests, migration and API generation.
Frontend writer owns the application controller and consumers. Senior owns shared Git state,
checkpoints, final review/QA assignment and delivery; only one Cargo owner runs at a time.
