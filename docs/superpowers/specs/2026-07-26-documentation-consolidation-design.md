# Documentation Consolidation Design

**Date:** 2026-07-26

## Goal

Reduce repository documentation to a small, enforceable set of current guides while making `.rules` the single authoritative development standard for humans and coding agents.

The completed repository will contain exactly 11 tracked Markdown files. Historical plans, completed reports, feature-specific design documents, duplicated setup guides, and stale architecture instructions will be removed rather than archived.

## Current Problem

The repository currently tracks 126 Markdown files. The largest source of clutter is implementation history under `docs/superpowers/`, with additional duplication across root documents, component guides, architecture notes, setup instructions, and feature-specific reports.

Several prominent documents contradict the current implementation:

- `backend-school/docs/MODULE_CREATION_GUIDE.md` describes migration-only permissions, a `roles.permissions` array, SQL-authored menu entries, and unsupported permission vocabulary.
- `docs/PERMISSIONS.md` uses legacy permission codes and permission helpers.
- `docs/backend-school/API_DEVELOPMENT.md` mixes current generated-contract instructions with obsolete root-level handler/repository structure.
- encryption setup documents describe the removed PostgreSQL `pgcrypto` or role-setting path instead of application-side AES-GCM and HMAC blind indexes.
- static tests require the same transient API checkpoint text to be duplicated across several documents.

This creates ambiguity about which instructions are authoritative and makes routine maintenance likely to produce further drift.

## Chosen Approach

Use a minimal canonical documentation set:

- `.rules` is the only authoritative development standard.
- `docs/README.md` is a short documentation index.
- `docs/TESTING.md` contains detailed local, contract, migration, smoke, and browser verification recipes.
- `docs/OPERATIONS.md` contains current deployment, server, secret, encryption-operation, and troubleshooting guidance.
- the root and four service READMEs contain only orientation, setup, run commands, environment requirements, and links to the canonical guides.
- `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` remain minimal auto-loaded entry points that direct tools to `.rules`.

No general `DEVELOPMENT.md` will be created because it would duplicate `.rules`.

## Final Documentation Allowlist

The final repository will contain these tracked Markdown files and no others:

1. `AGENTS.md`
2. `CLAUDE.md`
3. `GEMINI.md`
4. `README.md`
5. `docs/README.md`
6. `docs/TESTING.md`
7. `docs/OPERATIONS.md`
8. `backend-admin/README.md`
9. `backend-school/README.md`
10. `frontend-admin/README.md`
11. `frontend-school/README.md`

`.rules` is retained in addition to this Markdown allowlist.

Separate checkouts below `.worktrees/` are outside this change and must not be edited.

## Content Ownership

### `.rules`

`.rules` owns mandatory development behavior:

1. analysis before changes;
2. adding or changing a feature;
3. backend module, handler, service, policy, DTO, and error-handling rules;
4. frontend API, route metadata, permission UI, shared state, and layout rules;
5. permission contract and database grant workflow;
6. generated API contract workflow;
7. migration immutability and clean tenant safety;
8. realtime identity, heartbeat, reconnect, and invalidation rules;
9. PDPA, encryption, blind-index, PII permission, secret, and logging rules;
10. documentation policy;
11. a change-type verification matrix.

The file must describe durable rules rather than feature rollout history. It must not contain operation counts, completed implementation checkpoints, or long lists of individual endpoints.

### `docs/TESTING.md`

`docs/TESTING.md` owns command details and environment-dependent testing:

- universal pre-commit checks;
- backend-school and backend-admin checks;
- frontend-school and frontend-admin checks;
- permission contract generation and validation;
- API contract generation and validation;
- migration and database test isolation;
- encryption and PII tests;
- smoke tests;
- Playwright E2E configuration;
- realtime rollout checks;
- Ubuntu 26.04 Playwright compatibility;
- how to report a required check that could not run.

It must not repeat feature status or API operation counts.

### `docs/OPERATIONS.md`

`docs/OPERATIONS.md` owns current runtime and deployment operations:

- service topology and ports;
- Podman/Compose deployment entry points;
- required runtime secrets and caller-specific internal secrets;
- health and readiness behavior;
- deployment workflows and tenant menu registration inputs;
- reverse proxy, SSE, WebSocket, and upload requirements;
- Cloudflare and GitHub credential requirements;
- application-side encryption and blind-index operational rules;
- key stability and rotation requirements;
- file-storage environment and maintenance notes;
- safe tenant migration/cutover commands;
- focused troubleshooting that remains valid against current configuration.

Operational text must point to tracked configuration, scripts, and workflows as the executable source of truth. Removed `pgcrypto`, `ALTER ROLE`, database-setting encryption, and plaintext-PII instructions must not be retained.

### Root and service READMEs

`README.md` owns the current repository map, system purpose, quick start, and canonical documentation links.

Each service README owns only:

- the service purpose;
- its current framework and main dependencies;
- local prerequisites;
- setup, run, check, and test commands;
- ports and health endpoints where applicable;
- service-specific environment variables;
- links back to `.rules`, `docs/TESTING.md`, and `docs/OPERATIONS.md`.

Generated template text, obsolete frameworks, old ports, feature status, duplicated permission tutorials, and deployment walkthroughs must be removed.

### Tool entry points

`AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` must remain equivalent, concise pointers to `.rules` and the three active documents. They may preserve a short list of high-risk invariants but must not become independent rule sources.

## Development Workflow Required by `.rules`

### Adding a feature

The feature workflow must require:

1. read relevant active documentation and inspect the current implementation;
2. identify backend, frontend, database, API contract, realtime, security, and deployment impact;
3. write or update focused tests before implementation when behavior changes;
4. add a new sequential migration for schema or seed changes;
5. place backend code under `backend-school/src/modules/<feature>/` using Rust 2018 module roots;
6. keep handlers limited to request context, authorization/policy, service calls, response formatting, and realtime notification;
7. keep database access and business logic in services/models, with pure helpers and focused service tests;
8. use resource policies for relationship-aware scopes;
9. register implemented routes and documented OpenAPI operations;
10. consume generated wire DTOs through frontend API modules;
11. add `_meta.menu` only for real menu destinations and `_meta.access` for guard-only routes;
12. gate UI actions with generated permission constants while keeping backend authorization authoritative;
13. update existing canonical documentation only when a durable rule or operation changed;
14. run the verification matrix for every affected layer.

Feature toggles are optional rollout controls, not a mandatory step for every feature. When used, they must be created through a new migration and enforced on the backend.

### Adding or changing a permission

The permission workflow must require:

1. edit `contracts/permissions.json`, the only handwritten permission registry;
2. use the canonical `module.action.scope` tuple and the vocabulary accepted by `contracts/permissions.schema.json`;
3. add a new sequential migration when database permission rows, default role grants, organization grants, or data renames are required;
4. run `npm run generate:permissions`, `npm run check:permissions`, and `npm run test:permissions` from `frontend-school`;
5. commit the contract, lock, Rust generated registry, and TypeScript generated registry together;
6. use `codes::*` in backend production code/tests and `PERMISSIONS` or `PERMISSION_MODULES` in frontend production code/tests;
7. use a backend policy for resource-aware scopes;
8. keep PII access separate from ordinary profile access;
9. invalidate the tenant/user permission cache and emit `permission_changed` for runtime mutations that change effective access;
10. never edit generated registries directly.

The rules must explain that post-migration permission synchronization removes database permission codes absent from the contract, so migration-only permission definitions are invalid.

### Adding or changing a documented API

The API workflow must require:

1. typed Rust serde request/response DTOs;
2. `ToSchema` and exact `utoipa::path` metadata;
3. handler and schema registration in `backend-school/src/api_contract.rs`;
4. the shared `{ success, data, error?, message? }` response envelope;
5. `ApiResponse::empty()` or `empty_with_message()` for empty mutations;
6. `npm run generate:api-contracts`, `npm run check:api-contracts`, and `npm run test:api-contracts`;
7. generated frontend wire DTO consumption, with an explicit mapper only for a genuinely different view model;
8. focused backend/frontend contract tests;
9. no direct edits to generated OpenAPI or TypeScript artifacts.

### Adding a migration

The migration workflow must require:

- inspect the full active migration timeline first;
- create the next sequential file under `backend-school/migrations/`;
- never edit an applied migration, including `001_baseline.sql`;
- keep historical migrations under `migrations_legacy/` out of the runtime migrator;
- use `TEST_DATABASE_URL` and an isolated schema for DB-backed tests;
- use direct Neon endpoints rather than transaction-pooler endpoints for schema-based migration tests;
- use reviewed clean-tenant preparation and cutover scripts for operational rebaseline work;
- report rather than hide checksum mismatches.

## Verification Matrix

`.rules` will contain a concise matrix, while `docs/TESTING.md` contains exact recipes.

| Change type | Required verification |
| --- | --- |
| Every change | focused tests, `git diff --check`, `git status --short` |
| Rust backend | `cargo fmt --all -- --check`, focused tests, `cargo check` |
| Backend architecture/permission | `cargo test --test static_architecture` |
| Frontend | `npm run lint`, `npm run check`, focused static tests |
| Permission contract | generate, check, and test permissions; cross-stack static guards |
| API contract | generate, check, and test API contracts; backend API contract tests; frontend contract tests |
| Migration | sequential migration guard and DB-backed test when `TEST_DATABASE_URL` is available |
| PII/encryption | field-encryption and admission PII tests |
| Realtime | focused socket/SSE tests and relevant E2E when its environment is available |
| Browser flow | Playwright or smoke test when the required environment and credentials are available |

If an environment-dependent required check cannot run, the completion report must name the command, the missing dependency or credential, and the remaining risk. It must not imply full verification.

## Documentation Enforcement

A repository-level documentation policy test will run through the frontend static-test harness and CI.

It will:

- enumerate tracked Markdown files from the repository checkout while excluding `.worktrees/`;
- compare the result to the exact 11-file allowlist;
- validate relative local links in retained Markdown documents;
- assert that `.rules` contains the durable workflow sections and required generator/check commands;
- fail with the unexpected path or missing target when the policy is violated.

A dedicated or updated GitHub Actions workflow will trigger for `.rules`, Markdown files, the documentation policy test, and its workflow definition.

Adding a twelfth Markdown file will require an intentional allowlist change and a justification that the content cannot fit one of the existing owners.

## Existing Guard Changes

Static guards must follow code/configuration sources rather than deleted narrative documents:

- API contract ownership checks will read `.rules` and `docs/TESTING.md`, not `docs/backend-school/API_DEVELOPMENT.md`.
- tests that require the `178 unique operations` checkpoint in multiple documents will be removed; generated contract tests own operation accuracy.
- documentation assertions for role/organization deactivation will be removed or replaced with code/contract assertions.
- `frontend-school/tests/static/foundation-plan.test.mjs` will be deleted because it tests a completed plan rather than runtime code.
- backend deployment/readiness guards will inspect Compose files, deployment workflows, and smoke scripts directly, not `docs/PODMAN_SETUP.md`.
- GitHub workflow path filters will remove deleted document paths and include the new canonical paths where relevant.

Tests that preserve a genuine operational checklist, such as the WebSocket legacy query-identity rollout check in `docs/TESTING.md`, may remain when the checklist cannot be derived from static configuration alone.

## Deletion Policy

Every tracked Markdown file outside the final allowlist will be deleted after current, valid operational content is extracted.

This includes:

- all `docs/superpowers/plans/` and `docs/superpowers/specs/`;
- `frontend-school/docs/superpowers/`;
- `docs/plans/`;
- feature design and system analysis documents;
- improvement plans and completed phase reports;
- old permission, architecture, API development, frontend development, and module-creation guides;
- duplicated CORS, Nginx, deployment, Cloudflare, encryption, file-storage, and upload guides;
- historical performance evidence and changelogs.

Deleted content remains recoverable from Git history. No archive directory will be created.

The design specification and implementation plan for this consolidation are temporary implementation artifacts. They will be removed before final verification so the allowlist remains exact.

## Safety

- Deletion targets must be resolved from `git ls-files '*.md'` and compared to the allowlist before removal.
- `.worktrees/` must be excluded explicitly.
- current content must be written and link-checked before obsolete source documents are deleted.
- unrelated source changes must not be reformatted or modified.
- old encryption instructions must not be copied into the canonical files.
- package scripts, Cargo commands, workflow names, paths, ports, and environment names must be verified against the current repository before being documented.

## Acceptance Criteria

The implementation is complete when:

1. exactly 11 tracked Markdown files remain;
2. `.rules` is the single complete development standard and contains no feature-history checkpoints;
3. active documents have clear non-overlapping ownership;
4. root and service READMEs describe the current stack and link to canonical guides;
5. no retained document instructs developers to use legacy permissions, `roles.permissions`, old handler/repository paths, `pgcrypto`, `ALTER ROLE` encryption, or plaintext PII;
6. all retained relative local links resolve;
7. documentation policy tests reject unapproved Markdown files and broken links;
8. existing static tests and workflows no longer depend on deleted documents;
9. permission and API generated artifacts remain current;
10. backend static architecture tests pass;
11. frontend static tests, lint, and type checks pass;
12. `git diff --check` passes;
13. temporary design and implementation plan documents are deleted before the final verification;
14. the completion report lists every verification command and any environment-dependent check that could not run.

## Non-Goals

- changing application behavior, database schema, permissions, or API contracts;
- rewriting source-code comments that are unrelated to documentation ownership;
- editing files inside `.worktrees/`;
- publishing documents outside the repository;
- preserving completed plans in a new archive.
