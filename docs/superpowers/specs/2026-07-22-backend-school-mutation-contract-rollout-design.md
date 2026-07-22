# Backend-School Mutation Contract Rollout Design

**Date:** 2026-07-22

**Status:** Approved for implementation planning

**Scope:** Complete the generated API-contract rollout for backend-school JSON mutations, correct verified contract/security mismatches in the same bounded batch, and finish with a strict route-coverage guard.

## Context

SchoolOrbit currently generates a deterministic school OpenAPI document from Rust DTOs and `utoipa` handler metadata, then generates frontend TypeScript wire types from that document. The current checkpoint contains 68 operations: 32 authentication/authorization operations and 36 read-oriented operations. Reversible role and organization-unit deactivation established the first reviewed mutation-contract pattern, including permission checks, domain conflicts, audit behavior, cache/realtime effects, generated DTOs, and frontend integration.

Most backend-school workflow mutations remain outside the generated contract. The runtime surface is large: mutations are registered both directly in `main.rs` and through nested academic, admission, calendar, facility, question-bank, supervision, work, and workflow routers. A broad source scan finds roughly 262 mutation-method registrations, with the academic router alone containing roughly 123. These are inventory estimates rather than final operation counts because source files may also contain helper code and some mutations are already documented.

Treating this as one change would produce an unreviewable diff and make behavior corrections difficult to isolate. The rollout therefore uses four macro phases split into independently verified sub-batches of approximately 15–30 operations. Each successful sub-batch is fast-forward merged and pushed before the next sub-batch starts.

## Goals

- Represent every backend-school application JSON mutation in the generated OpenAPI document.
- Correct verified backend/frontend, permission, status, or response mismatches in the same sub-batch that discovers them.
- Replace handwritten frontend wire DTOs with generated aliases for every migrated operation.
- Preserve explicit view/domain models and mapping when UI state differs from the wire contract.
- Keep request/response optionality, nullability, UUID, date/time, and envelope semantics aligned with actual serde behavior.
- Make every mutation's authentication, permission, domain error, and side-effect behavior reviewable.
- Classify protocol-specific and binary routes explicitly instead of pretending they are ordinary JSON APIs.
- Add a final CI/static guard that rejects a new unclassified mutation route.
- Keep every sub-batch independently testable, reviewable, revertible, mergeable, and deployable.

## Non-goals

- Redesign stable APIs solely to make OpenAPI metadata easier to write.
- Generate a runtime HTTP client in this rollout.
- Add runtime response validation in this rollout.
- Model WebSocket frames, SSE streams, liveness/readiness probes, or raw binary transfer bodies as JSON operations.
- Refactor unrelated modules or split large services unless a focused change is required to test or correct a discovered contract bug.
- Add new permissions merely to make naming more uniform.
- Change database schemas without a verified behavior requirement; when schema work is required, it uses a new sequential migration.
- Expand this rollout to backend-admin or frontend-admin. Their generated `admin-api.json` remains a separate future design.

## Decisions already approved

1. When contract review proves a backend/frontend, permission, status, or DTO mismatch, correct behavior and add regression coverage in the same sub-batch. Do not merely document a known-bad behavior.
2. Split the four macro phases into bounded sub-batches of approximately 15–30 operations.
3. Fast-forward merge and push `main` after every sub-batch passes verification and review.
4. Backend-school JSON mutations are in scope. SSE, WebSocket, health/readiness, and binary bodies are explicit classifications outside the JSON contract.

## Approaches considered

### 1. Inventory-first bounded sub-batches — chosen

Derive a complete mutation inventory from direct and nested runtime routers. For each operation, inspect the handler, service, permission policy, response envelope, frontend API wrapper, and consumers before adding metadata. Migrate operations in bounded domain groups and enable the strict guard only after the existing inventory is classified.

This approach provides the strongest completeness evidence, keeps reviews focused, and makes rollback practical.

### 2. Router-file-by-router-file rollout

Annotate handlers in source-file order without a global inventory. This starts implementation quickly but makes nested routers, shared handlers, already-documented operations, and exclusions harder to reconcile. It also provides weak evidence that the rollout is complete.

### 3. Automatic contract inference from router source

Parse Axum route expressions and infer an OpenAPI document before inspecting handlers. Route registration can reveal path, method, and handler, but it cannot reliably reveal serde optionality, response type, permission policy, service errors, or frontend expectations. This risks producing a contract that looks complete while describing incorrect behavior.

## Architecture

Rust serde DTOs and `utoipa` handler metadata remain the wire-contract source of truth:

```text
Runtime route
  -> inspect handler, service, policy, and frontend consumer
  -> characterize request, response, permission, status, and side effects
  -> reproduce and correct verified mismatch with tests
  -> add utoipa path/schema metadata
  -> register operation and schemas in SchoolApiDoc
  -> regenerate school-api.json and school-api.ts
  -> migrate frontend wrapper to generated wire DTOs
  -> run focused and global verification
  -> review, fast-forward merge, and push
```

The runtime architecture does not depend on the inventory or generated document. The contract exporter continues to work offline without database credentials or a running backend. Frontend API wrappers continue to use the existing `apiClient` and may map wire DTOs into UI-specific models.

## Operation inventory

Before implementing a sub-batch, every candidate operation is recorded with:

- HTTP method and resolved path;
- runtime handler and router source;
- classification: JSON, multipart-with-JSON-metadata, binary, SSE, WebSocket, health/readiness, or intentional internal operation;
- request DTO, path/query parameters, and content type;
- success response DTO and standard envelope;
- status codes that current or corrected behavior can actually emit;
- authentication and exact permission constant/policy;
- service or policy boundary that owns domain behavior;
- cache, realtime, audit, notification, or file-storage side effects;
- frontend API function and current consumers, when present;
- generated-contract status and assigned sub-batch.

The inventory is rollout evidence, not a competing wire-contract source. Rust DTOs plus registered handler metadata remain authoritative. A frontend helper is not evidence that a backend route exists. Conversely, a backend application JSON route remains in scope even if it currently has no frontend consumer.

## Route classifications

### Generated JSON

An application HTTP operation with a JSON request, JSON response, or both belongs in `school-api.json`. Empty mutations use `ApiResponse<EmptyData>`. Created-resource responses use a typed resource or ID envelope according to actual runtime behavior.

### Multipart metadata

Multipart operations document the known form fields and the JSON response when the metadata is part of the application contract. Raw uploaded bytes are not modeled as JSON. Stable metadata shapes use concrete schemas rather than `any`, `unknown`, or unstructured objects.

### Protocol and binary exclusions

The following categories are classified but not forced into the ordinary school JSON document:

- SSE streams;
- WebSocket upgrades and frames;
- `/health` and `/ready` liveness/readiness probes;
- raw file, image, spreadsheet, PDF, document, or other binary download bodies;
- an internal/non-consumed route only when a concrete justification is recorded.

An exclusion is not a wildcard. It names an exact operation/handler and classification, and the final guard rejects unknown classifications and stale exclusions.

## Rollout phases and sub-batches

Exact operation membership and counts come from the inventory. The following boundaries define ownership and ordering; a sub-batch should normally contain 15–30 operations and may be split further if behavior review exposes a larger correction.

### Phase 1 — People, staff, and organization

1. Staff create, update, and deactivation plus their typed result/error behavior.
2. Student administration and student-parent relationship mutations.
3. Achievements and remaining people-management mutations.
4. Audit the already-migrated role, user-role, permission, delegation, member, and organization-unit mutations for inventory completeness; do not rewrite them without a verified defect.

This phase establishes reusable create/update/deactivate schemas, PII boundaries, permission checks, and person-resource mappings before larger workflow modules.

### Phase 2 — Academic workflows

1. Academic structure, years, semesters, classrooms, and enrollment.
2. Subjects, subject groups, curriculum, study plans, and related assignments.
3. Course planning, teaching assignments, and scheduling configuration.
4. Timetable entries, conflict results, generation actions, and timetable templates.
5. Activities, activity slots, groups, instructors, memberships, and results.
6. Assessment plans, score mutations, exam scheduling, invigilator/room assignments, and question-bank mutations.

Large academic modules remain independently reviewable. Timetable WebSocket messages and binary exports stay separately classified.

### Phase 3 — Admission and support domains

1. Admission rounds, subjects, tracks, and application lifecycle mutations.
2. Admission scores, ranking, room assignment, selection, enrollment, and student-ID mutations.
3. Applicant portal mutations, document metadata, exam rooms, exam configuration, and seat assignment.
4. Calendar and facility mutations.
5. Teaching-supervision lifecycle mutations.
6. Work items, workflow windows, consent, notifications, school settings, menu/feature administration, and remaining support mutations.
7. File and document endpoints: generate JSON/multipart metadata contracts where applicable and record binary-body exclusions precisely.

Public admission operations receive the same request/response review but must not accidentally gain staff authentication. PII and credential-bearing applicant flows must not expose or log secrets through generated examples, tests, or audit data.

### Phase 4 — Strict mutation coverage

1. Build a reusable test-only router inventory scanner for direct and nested router sources.
2. Resolve the configured prefixes for `main.rs` and nested academic, admission, calendar, facility, question-bank, supervision, work, and workflow routers.
3. Detect `post`, `put`, `patch`, and `delete`, including chained methods such as `get(...).post(...)`.
4. Compare every derived mutation handler/operation with the OpenAPI registry or an exact approved exclusion.
5. Reject new unclassified mutations, unknown exclusion categories, duplicate operation IDs, duplicate method/path pairs, and stale exclusions.
6. Derive and document final operation counts from the generated contract rather than maintaining hand-counted constants.

The scanner is a coverage guard, not a schema generator. Fixture tests prove its parsing behavior before it is applied to production router sources.

## Contract and DTO rules

- Handler request/response DTOs use serde and `ToSchema` with actual camelCase serialization.
- Known-shape JSON and JSONB boundaries use concrete structs.
- Optional and nullable are modeled separately according to serde behavior.
- UUIDs, dates, date-times, enums, tagged unions, pagination, and structured conflict details retain their exact wire types.
- JSON success responses use `ApiResponse<T>`; empty success uses `ApiResponse<EmptyData>`.
- JSON errors use `ApiErrorResponse` or an explicitly typed structured-error envelope.
- OpenAPI metadata lists only statuses the handler/service/policy can emit.
- A UI domain/view model may differ from the generated wire type only through an explicit mapper.
- A migrated frontend wrapper must not retain a handwritten duplicate of the generated wire DTO.
- Existing `apiClient` envelope validation remains the runtime frontend boundary.

## Behavior and security review

Every mutation review covers:

- authentication middleware and tenant resolution;
- exact backend permission constants and resource-aware policy;
- request validation and domain invariants;
- missing-resource and conflict semantics;
- transactional boundaries and rollback behavior;
- permission-cache, realtime, notification, audit, and external-service side effects;
- PII visibility, encryption, blind indexes, logging, and audit payloads;
- frontend route/action gating without treating frontend checks as authorization;
- action-specific loading state and local-state patching when the response supports it.

Handlers remain thin: request context, permission/policy, service call, response formatting, and permitted transport side effects. Database and business logic remain in services/policies. When a multi-row mutation writes homogeneous data, it validates and deduplicates before a bulk insert/upsert/delete where practical.

Verified mismatches follow this sequence:

1. Write a focused test that reproduces the current incompatibility or security/correctness failure.
2. Establish the correct behavior from domain rules, permission registry, current workflow, and existing data semantics.
3. Correct the handler/service/frontend behavior in the same sub-batch.
4. Add regression coverage for the corrected behavior.
5. Add OpenAPI metadata and migrate frontend wire types only after behavior is characterized.

No unrelated redesign is bundled into a contract batch.

## Error handling

- Authentication failures remain 401 and authorization failures remain 403.
- Missing resources use 404.
- Domain state conflicts use 409.
- Request parsing/validation uses the project's established 400 or 422 behavior and is documented precisely.
- Unexpected persistence or external-service failures use the standard 500 error envelope without leaking implementation details.
- Database error strings, credentials, tokens, raw request bodies, plaintext national IDs, and applicant portal secrets are never returned, logged, committed in fixtures, or stored in audit values.
- Partial external workflows preserve existing rollback/compensation semantics; contract work must not imply atomic behavior that the service does not provide.

## Database and migration safety

Contract-only work does not change schema. If a verified behavior fix needs a schema, index, data, or comment change, it adds the next sequential migration. Applied migrations remain immutable. Tests that require PostgreSQL use `TEST_DATABASE_URL` and the documented isolated-schema helpers.

`national_id` remains app-side AES-256-GCM ciphertext using `ENCRYPTION_KEY`, with keyed HMAC-SHA256 blind indexes using `BLIND_INDEX_KEY`. The rollout must not reintroduce PostgreSQL `pgcrypto` helpers or plaintext identifiers.

## Frontend integration

- API wrappers import generated request/response schemas through stable aliases.
- Components and pages consume view models or mapped generated DTOs without blind envelope casts.
- Permission checks use the generated frontend permission registry.
- Mutation actions expose action-specific loading/error state.
- When a typed mutation returns the affected resource, the UI replaces, inserts, or removes only that local resource instead of broadly reloading the workspace.
- Svelte 5 files preserve existing rune/event conventions and are checked with the Svelte autofixer after edits.
- Assignment and lookup behavior is not broadened merely because management mutations are documented.

## Testing strategy

Implementation follows test-driven development. Every sub-batch includes the smallest relevant set from:

### Inventory and contract tests

- expected path, method, and unique operation ID;
- request/path/query schema and optionality;
- typed success envelope and exact documented errors;
- deterministic OpenAPI/TypeScript generation;
- stale generated-output detection;
- no handwritten duplicate wire DTO in migrated frontend modules.

### Behavior and authorization tests

- success and validation behavior;
- authentication, exact permission, and resource-policy denial;
- not-found and domain-conflict behavior;
- transaction/rollback and idempotency where applicable;
- cache/realtime/audit/notification effects where applicable;
- PII redaction/encryption and safe audit/log payloads;
- database-backed behavior through an isolated test schema when SQL semantics matter.

### Frontend tests

- API wrapper path/method/body and typed envelope;
- permission-gated actions;
- local-state mutation behavior;
- focused static contract tests;
- `svelte-check` and Svelte autofixer for touched components;
- authenticated Playwright coverage when approved sandbox credentials are available.

If browser credentials are unavailable, static/type/contract coverage runs and the missing authenticated browser run is reported explicitly rather than represented as passing.

### Sub-batch verification gate

At minimum:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test api_contract::tests --bin backend-school
cargo test --test static_architecture

cd ../frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Run affected service/database tests and changed-file formatting checks in addition to this gate. Repository-wide lint failures are isolated from changed-file failures; unrelated files are not bulk-formatted.

## Integration workflow

Each sub-batch starts from the latest clean `main` in an isolated worktree. After focused implementation:

1. run affected and global contract checks;
2. inspect migration and generated diffs;
3. scan changed fixtures/audits/logging for plaintext PII or credentials;
4. review behavior, permission, status, response, and exclusions;
5. ensure the worktree is clean and commits are focused;
6. fast-forward merge to `main`;
7. push `origin/main`;
8. create the next sub-batch worktree from the new `main`.

No sub-batch is integrated merely because it compiles.

## Documentation updates

Every phase updates the current operation checkpoint and completed domain list in:

- `.rules`;
- `docs/TESTING.md`;
- `docs/backend-school/API_DEVELOPMENT.md`;
- `IMPROVEMENT_PLAN.md`.

Counts are derived from `school-api.json`. The documents keep SSE, WebSocket, health/readiness, binary, and backend-admin scope explicit.

## Success criteria

The rollout is complete when:

- every registered backend-school JSON mutation is represented in `school-api.json`;
- every non-JSON mutation is covered by an exact justified classification;
- frontend-school uses generated wire DTOs for every migrated mutation and contains no handwritten duplicate contract shapes;
- verified permission, status, request, response, and PII mismatches discovered during review are corrected with regression tests;
- generated OpenAPI and TypeScript artifacts are deterministic and current;
- direct and nested router mutations are covered by the strict guard;
- adding an unclassified mutation route causes CI/static verification to fail;
- stale or unused exclusions fail verification;
- all sub-batches have passed their relevant backend, database, frontend, contract, and formatting gates;
- `main` and `origin/main` contain each reviewed sub-batch in order.

## Expected outcome

Future API development gains one enforceable workflow: define a typed Rust contract, implement and authorize the behavior, register the operation, regenerate frontend types, and pass coverage checks. Frontend/backend DTO drift becomes a failing check instead of a production discovery, permission behavior is reviewed alongside every mutation, and large academic/admission modules can be refactored without maintaining parallel handwritten wire schemas.
