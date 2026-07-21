# Shared API Contracts Design

**Date:** 2026-07-22

**Status:** Approved

**Scope:** Backend-first OpenAPI generation, beginning with `/api/auth/me` and expanding in verified phases to all application JSON HTTP APIs

## Context

SchoolOrbit already has a consistent JSON response envelope and concrete Rust DTOs at most backend boundaries. The frontend clients also use typed `ApiResponse<T>` calls. The payload types themselves are nevertheless maintained by hand in Rust and TypeScript, so the two sides can drift even when both compile independently.

The current `/api/auth/me` flow is a useful example. Rust serializes `UserResponse` with camel-case JSON fields, while `frontend-school/src/lib/api/auth.ts` defines a separate compatibility type that accepts both camel-case and legacy snake-case fields. Several nullable backend fields are represented as optional, non-null frontend fields. Static regular-expression tests detect selected historical mismatches, but they do not make the backend contract the source of the frontend type.

The permission registry solved a related problem with one source of truth, deterministic generation, committed artifacts, and a focused drift check. API contracts need the same developer experience without replacing the existing API client or changing runtime behavior.

## Goals

- Make backend Rust wire DTOs and handler metadata the source of truth for HTTP contracts.
- Export a deterministic OpenAPI document without starting a server or connecting to a database.
- Generate TypeScript wire types consumed by the existing frontend API clients.
- Detect stale or manually edited OpenAPI and TypeScript artifacts locally and in focused CI.
- Start with `/api/auth/me` as a contained pilot, verify the toolchain, and then migrate the remaining application JSON APIs in bounded groups.
- Preserve current routes, authentication, authorization, response envelopes, payloads, and user-visible behavior during contract adoption.
- Keep frontend domain/view models separate when they contain derived UI state.
- Document exact required, optional, nullable, UUID, date, and date-time semantics.
- Leave a repeatable workflow for adding future endpoints without handwriting matching TypeScript DTOs.

## Non-goals

- Generate a new runtime HTTP client during the initial rollout.
- Add Zod or another runtime response validator during the initial rollout.
- Expose Swagger UI or a public OpenAPI endpoint.
- Redesign the standard API response or error envelope.
- Change cookie authentication, permission checks, tenant resolution, or endpoint behavior.
- Change database schema or add a migration.
- Put business logic in a shared contract crate.
- Force file downloads, image/binary responses, health probes, Server-Sent Events, or WebSocket messages into OpenAPI response types. Realtime message contracts require a separate future design because OpenAPI models HTTP request/response operations, not bidirectional socket protocols.
- Remove a compatibility field until direct inspection verifies that the backend no longer emits or consumes it.

## Chosen approach

The implementation will use backend-first OpenAPI.

Rust request/response DTOs remain authoritative. OpenAPI schema derives and path metadata describe those same types and operations. A backend export command renders the OpenAPI document, and a pinned TypeScript generator renders frontend wire types from that document.

This approach was selected over:

- direct Rust-to-TypeScript generation, which is lighter but does not describe paths, methods, status codes, or envelopes and would provide a weaker base for future client generation;
- handwritten JSON Schema, which would require generating or duplicating Rust DTOs and move authority away from the backend boundary already used at runtime.

The flow is:

```text
Rust DTOs + HTTP operation metadata
                 |
                 v
       deterministic OpenAPI
                 |
                 v
       generated TypeScript DTOs
                 |
                 v
 existing apiClient + explicit domain mapper
                 |
                 v
          Svelte stores and UI
```

Generated files are committed artifacts. Application builds consume them but do not regenerate them implicitly.

## Contract ownership and file layout

The planned logical layout is:

```text
backend-school/src/
├── api_contract.rs                 # OpenAPI document and exporter
├── api_response.rs                 # shared success/error envelope DTOs
└── modules/<feature>/              # authoritative request/response DTOs and handlers

backend-admin/src/
├── api_contract.rs                 # added when the admin rollout begins
└── ...                             # authoritative admin DTOs and operations

contracts/openapi/
├── school-api.json                 # generated backend-school document
└── admin-api.json                  # generated in the backend-admin phase

scripts/
└── generate-api-contracts.mjs      # generate/check orchestration

frontend-school/src/lib/api/generated/
└── school-api.ts                   # generated wire schemas and operations

frontend-admin/src/lib/api/generated/
└── admin-api.ts                    # generated in the backend-admin phase
```

Exact module names may be adjusted in the implementation plan to match compiler constraints, but the ownership boundary does not change:

- handwritten Rust wire DTOs and operation metadata are authoritative;
- OpenAPI JSON and TypeScript files are generated and must not be edited manually;
- handwritten frontend API modules adapt generated wire types to the existing client and domain models;
- frontend components and stores do not import raw generated operation internals when a stable API-module alias is available.

The pilot will not introduce a shared Rust business crate. If backend-school and backend-admin later need identical stable HTTP DTOs, a small contract-only crate may be proposed separately. It must not contain handlers, services, policies, database models, or business rules.

## Pilot: `/api/auth/me`

The first documented operation is:

```text
GET /api/auth/me
```

The OpenAPI operation covers:

- the successful standard envelope containing `UserResponse`;
- the standard unauthorized error envelope;
- operation identifier, method, path, status codes, and component schemas;
- UUID and date-time formats;
- exact JSON field names, required fields, optional fields, and nullable fields.

The `UserResponse` wire shape is expected to express:

- required `id`, `username`, `firstName`, `lastName`, `userType`, `status`, and `createdAt` fields;
- nullable `nationalId`, `email`, `phone`, and `profileImageUrl` fields because Rust serializes their `Option` values even when absent;
- optional `primaryRoleName` and `permissions` fields because Rust uses `skip_serializing_if` for them;
- `id` as a UUID-formatted string and `createdAt` as a date-time-formatted string.

Schema annotations will be added where derive inference does not reproduce serde serialization exactly. Contract tests compare the generated representation with intended JSON behavior rather than assuming library defaults are correct.

The pilot does not change which fields `/api/auth/me` exposes. In particular, it neither adds sensitive fields nor logs values. Generated artifacts contain only names, descriptions, and type metadata, never real user data.

## Frontend integration

The generated schema is a wire boundary, not the application store model.

`frontend-school/src/lib/api/auth.ts` will expose a stable alias such as `CurrentUserDto` backed by the generated `UserResponse` schema. A handwritten `normalizeCurrentUser(dto)` function maps it explicitly into the existing frontend `User` model.

The mapper will:

- derive the UI `role` value from `primaryRoleName` or `userType` as it does today;
- preserve the current `user_type` domain field only if UI consumers still require it;
- normalize nullable wire fields to `undefined` where the frontend domain model intentionally uses optional fields;
- stop accepting a snake-case `user_type` wire fallback after direct backend inspection and contract tests establish that only `userType` is serialized;
- avoid a broad object spread when it would hide nullability or naming conversions.

The login payload currently embeds the same backend `UserResponse`. Its handwritten `LoginData` wrapper may reuse `CurrentUserDto` in the pilot so the user shape is not duplicated, but `/api/auth/login` is not considered fully documented until its operation, request, responses, and status codes are added in the auth rollout phase.

The existing `apiClient`, `requireApiData`, cookie behavior, error normalization, stores, and UI consumers remain in place. Generated operation clients and runtime response validation are deferred until contract coverage is complete and their value can be evaluated separately.

## Export and generation workflow

The developer-facing commands will be available from a frontend package or an equally discoverable repository command:

```bash
npm run generate:api-contracts
npm run check:api-contracts
```

Normal generation performs these steps:

1. Run the relevant Rust OpenAPI exporter.
2. Exit before environment initialization, database pool creation, background jobs, or HTTP binding.
3. Serialize the OpenAPI document with stable ordering and formatting.
4. Generate TypeScript using a pinned dev dependency.
5. Apply deterministic formatting.
6. Write only the explicitly configured contract and generated frontend paths.

Check mode performs the same rendering into a temporary location and compares every artifact byte-for-byte. It does not modify tracked files and exits non-zero when an artifact is absent, stale, or manually edited.

The exporter must not require network access, tenant configuration, database credentials, authentication secrets, or application runtime environment variables. The exporter path will be tested to prove this property.

Generated files begin with a do-not-edit header and identify their generating command. Dependency versions are pinned through `Cargo.lock` and the appropriate npm lockfile.

## Error handling and safety

Generation failures are fail-closed:

- OpenAPI export failure prevents TypeScript generation.
- An unresolved or unsupported schema type fails the command instead of falling back to `any`.
- Missing operation identifiers, duplicate operation identifiers, or duplicate schema names fail validation.
- Known request and response bodies may not generate `any`, `unknown`, or untyped object maps merely to make generation pass.
- File writes occur only after all outputs render successfully, preventing partial contract updates.
- Check mode never writes.

Adopting a contract must not silently change runtime serialization. If OpenAPI inference and serde behavior differ, explicit schema annotations or a corrected DTO are required. Any intentional payload change is a separate API change with its own compatibility review and tests.

No generated description or example may contain national IDs, passwords, cookies, tokens, database URLs, or real personal data. Examples, if later added, must be synthetic.

## Testing and drift protection

### Backend contract tests

The pilot adds focused tests that assert:

- `/api/auth/me` exists with the correct method and operation identifier;
- the documented success and unauthorized responses use the intended envelope schemas;
- `UserResponse` field names, required set, optional set, nullability, UUID format, and date-time format match serde behavior;
- OpenAPI export is deterministic;
- export completes without application runtime configuration or database access.

As coverage expands, each migrated operation gets equivalent request, response, status, and schema assertions where derive behavior alone is not sufficient.

### Frontend contract tests

The pilot updates cross-stack static checks to ensure:

- `auth.ts` imports the generated current-user wire type;
- the handwritten `BackendUser` copy is removed;
- the obsolete snake-case wire fallback is absent;
- the mapper preserves required domain behavior;
- no known generated schema degrades to `any` or `unknown`.

Existing behavior-oriented tests remain. Generated types complement them; they do not replace authorization, service, integration, smoke, or browser tests.

### Focused CI

A focused workflow, similar to the permission-contract workflow, runs when relevant Rust DTOs/handlers, response envelopes, generator code, contract artifacts, generated TypeScript, or dependency locks change. It runs contract generation in check mode plus focused Rust and frontend tests.

This focused gate is useful even while the project commonly integrates directly to `main`: it catches a stale generated artifact after a backend DTO edit. It does not imply a broad redesign of the repository's deployment workflows.

## Rollout to all application JSON APIs

The global rollout follows the successful pilot and is deliberately incremental. “All APIs” in this design means application JSON HTTP operations provided by backend-school and backend-admin, including internal JSON operations that have a TypeScript or service consumer. Protocol-specific and binary endpoints remain governed by their existing typed code and receive separate contract designs if generation is needed.

### Phase 1 — Pilot

- `/api/auth/me`
- common success and error envelopes required by the operation
- school OpenAPI exporter, TypeScript generator, check command, tests, and focused CI

The phase is complete only when runtime output remains compatible and the full pilot verification passes.

### Phase 2 — Authentication and authorization administration

- remaining backend-school auth operations;
- roles, permissions, user-role assignments, and organization authorization DTOs;
- shared identifiers and empty/id response schemas used by those operations.

This phase removes high-impact security and session DTO duplication before feature-heavy modules are migrated.

### Phase 3 — Backend-school read-oriented modules

- lookup and menu operations;
- current-user, parent, staff dashboard, calendar, notification, and other primarily read-oriented APIs;
- paginated and list envelopes.

Read-oriented groups establish reusable schema patterns with lower mutation risk.

### Phase 4 — Backend-school workflow and mutation modules

- academic, timetable, admission, supervision, exam scheduling, facilities, work, consent, files metadata, and remaining feature modules;
- typed validation and structured conflict/error payloads;
- multipart metadata contracts where JSON bodies or JSON responses are present.

Large modules are split into multiple migration batches. A batch must keep frontend and backend compiling and may not leave a migrated frontend call on a handwritten duplicate DTO.

### Phase 5 — Backend-admin and frontend-admin

- create `admin-api.json` from backend-admin Rust DTOs and operations;
- generate frontend-admin wire types;
- migrate auth, school management, deployment history, and migration orchestration contracts;
- distinguish frontend-admin SvelteKit proxy endpoints from backend-admin endpoints so each documented path names the actual network boundary.

### Phase 6 — Coverage audit and client-generation decision

- inventory every registered HTTP route and classify it as generated JSON, binary/file, health/readiness, SSE, WebSocket, or intentionally internal/non-consumed;
- fail coverage checks when a new JSON application route lacks contract metadata;
- remove superseded regex-only shape checks while retaining behavior and architecture guards;
- decide separately whether generated operation clients and runtime validation now provide enough benefit to adopt.

Each phase is independently reviewable and revertible. The pilot infrastructure is not considered proof that every DTO inference is safe; each module must be verified against its actual serde behavior and current frontend consumers.

## Migration rules for each endpoint batch

Every batch follows the same sequence:

1. Read the handler, service output, Rust DTO, frontend API function, and consuming types/components.
2. Record the actual request, response, status, optionality, nullability, and error variants.
3. Add OpenAPI schema and operation metadata without changing runtime behavior.
4. Regenerate OpenAPI and TypeScript artifacts.
5. Replace handwritten frontend wire DTOs with generated aliases.
6. Keep or add explicit mapping when the UI model differs from the wire model.
7. Run focused backend/frontend contract tests and the module's behavior tests.
8. Run the global contract drift check before commit.

If inspection finds an existing backend/frontend mismatch, the batch must first characterize current production behavior. Fixing the mismatch is a separate explicit change or a clearly identified part of the batch, not an accidental consequence of generated types.

## Success criteria

The pilot succeeds when:

- `/api/auth/me` returns the same payload and status behavior as before;
- frontend-school consumes a generated wire DTO instead of a copied `BackendUser` interface;
- required, optional, nullable, UUID, and date-time details match actual JSON;
- modifying the Rust contract without regenerating causes `check:api-contracts` and focused CI to fail;
- generation works offline without database or application runtime configuration;
- backend checks/tests/clippy, frontend checks/static tests/build, and generator checks pass;
- authenticated smoke verification runs when approved sandbox credentials are available.

The full rollout succeeds when:

- every application JSON HTTP operation is represented in the appropriate generated OpenAPI document or is explicitly classified with a justified exclusion;
- frontend-school and frontend-admin no longer maintain handwritten copies of migrated wire DTOs;
- new JSON routes fail coverage checks when contract metadata is missing;
- generated artifacts are deterministic, committed, and current;
- no contract schema uses `any`/`unknown` for a known payload merely to bypass modeling;
- API behavior, authorization, tenant isolation, encryption boundaries, and migration safety remain intact.

## Verification baseline

The implementation plan will assign exact commands per phase. At minimum, the pilot verification includes:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test --bin backend-school
cargo clippy --bin backend-school -- -D warnings

cd ../frontend-school
npm run check:api-contracts
npm run check
npm run test:static
npm run build

cd ..
git diff --check
git status --short
```

Database-backed, authenticated smoke, and browser tests use only approved environment-provided credentials. No credential is written to a contract, generated artifact, test fixture, log, or commit.

## Documentation and developer workflow

After the pilot proves the commands, `.rules`, `docs/TESTING.md`, and the relevant backend API development guide will state:

```text
1. Define or update the Rust request/response DTO.
2. Add or update OpenAPI operation metadata.
3. Run generate:api-contracts.
4. Use the generated TypeScript wire type through the frontend API module.
5. Add an explicit domain mapper when UI state differs from the wire shape.
6. Run check:api-contracts and focused behavior tests.
7. Commit Rust metadata, OpenAPI, generated TypeScript, tests, and documentation together.
```

Documentation must also explain the difference between a wire DTO and a frontend domain/view model, and must warn developers and AI agents never to edit generated files directly.
