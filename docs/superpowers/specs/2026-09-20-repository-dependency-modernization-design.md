# Repository Dependency Modernization Design

## Goal

Bring every direct dependency in `frontend-school`, `frontend-admin`, `backend-school`, and
`backend-admin` to the newest stable version that the repository can support without forcing an
invalid dependency graph, while preserving application behavior, stored data compatibility,
security boundaries, and production rollback clarity.

## Success Criteria

- Every direct JavaScript and Rust dependency is either upgraded to its newest compatible stable
  release or has a documented ecosystem constraint that prevents the upgrade.
- Installation succeeds without `--force`, `--legacy-peer-deps`, patched registry metadata, or
  disabled checks.
- Deprecated `lucide-svelte` imports are migrated to `@lucide/svelte`, and the deprecated package
  is removed.
- The SheetJS dependency uses an immutable versioned artifact URL rather than `xlsx-latest.tgz`.
- Both frontend applications pass lint, type checking, production builds, and their applicable
  automated tests.
- Both Rust applications pass formatting, tests, locked checks, and compilation on the repository
  toolchain.
- Authentication, encryption, database access, file generation, API contracts, and browser
  navigation preserve their existing externally observable behavior.
- Each release wave passes production deployment acceptance before the next wave is integrated.

## Scope

This modernization covers the four deployable applications and the internal crates in the
`backend-school` workspace:

- `frontend-school/package.json` and `package-lock.json`
- `frontend-admin/package.json` and `package-lock.json`
- `backend-school/Cargo.toml`, `Cargo.lock`, and internal-crate consumers
- `backend-admin/Cargo.toml`, `Cargo.lock`, and dependency consumers
- Source and configuration changes strictly required by dependency API or tooling migrations
- Focused regression tests needed to prove compatibility across a major upgrade

It does not change domain behavior, database schemas, permission definitions, API payload shapes,
or deployment topology. A dependency release that requires one of those changes must be isolated
in a separately reviewed design rather than smuggled into this modernization.

## Version Selection Policy

“Update everything” means the newest stable version that produces a supported graph and matches
the repository runtime, not the numerically newest registry entry at any cost.

- Never use npm peer-dependency bypass flags or Cargo patch overrides merely to make resolution
  succeed.
- Prefer stable releases; release candidates and prereleases are excluded.
- Keep `@types/node` on major 24 because both frontends require Node `>=24 <25`.
- Use TypeScript 6 in `frontend-admin`, not TypeScript 7, until both SvelteKit and
  `typescript-eslint` declare support for TypeScript 7. Keep `frontend-school` on TypeScript 5.9.3
  until the latest stable `openapi-typescript` supports TypeScript 6; do not bypass its peer range.
- Upgrade ESLint to 10 because current `typescript-eslint`, `eslint-plugin-svelte`, and
  `@eslint/compat` releases declare ESLint 10 support.
- Preserve exact-version pins where the project intentionally owns reproducibility. Replace the
  mutable SheetJS URL with the versioned `0.20.3` artifact URL.
- Update manifest minimums as well as lockfiles so the tested dependency baseline is visible and a
  clean install cannot silently resolve back to an older floor.
- Remove a direct dependency only after repository-wide import and configuration searches prove it
  unused or after all consumers are migrated in the same wave.

## Current Compatibility Findings

- `backend-school` declares Axum `0.8.7`, but its lockfile already resolves Axum `0.8.9`.
- `backend-admin` still resolves Axum `0.8.7`; the compatible refresh moves it to `0.8.9`.
- `lucide-svelte` is deprecated in favor of `@lucide/svelte`; approximately 229 source files use
  the deprecated import while `@lucide/svelte` currently has no source consumers.
- The latest TypeScript registry release is 7, but SvelteKit 2.70 and `typescript-eslint` 8.70 only
  declare support through TypeScript 6.
- `openapi-typescript` 7.13.0 is the latest stable generator and declares TypeScript `^5.x`, so the
  school frontend must retain TypeScript 5.9.3 while the admin frontend can move to 6.0.3.
- Rust major upgrades exist for SQLx, Reqwest, JsonWebToken, Rand, AES-GCM, HMAC, SHA-2, Base64,
  and Lopdf. These affect independent runtime boundaries and must not be hidden inside one release.

## Release Architecture

The work is one modernization program delivered through four independently testable and
deployable waves. A later wave starts only after the previous wave has passed local verification,
CI, deployment, and acceptance.

### Wave 1: Compatible Refresh

Refresh lockfiles and direct manifest floors within the currently accepted API-compatible ranges
for all four applications. This wave includes Axum `0.8.9`, current Tokio/Tower/Serde patch or
minor releases, current Svelte ecosystem releases that do not require source migrations, and all
other compatible patch/minor updates.

This wave establishes a clean baseline before any breaking source migration. It must not include
behavioral refactors or suppress newly surfaced warnings.

### Wave 2: Frontend Major and Deprecated-Package Migration

Upgrade both frontends to the supported major toolchain baseline, including TypeScript 6 in
`frontend-admin`, TypeScript 5.9.3 in `frontend-school` until its stable OpenAPI generator supports
6, ESLint 10, Prettier Svelte plugin 4, and other latest compatible JavaScript packages. Migrate
every `lucide-svelte` import to `@lucide/svelte`, remove the deprecated package, and pin the
SheetJS artifact.

Required source edits are limited to compiler, linter, formatter, component-library, and package
API compatibility. Formatting changes must remain mechanical and must not be mixed with UI
redesign.

### Wave 3: Rust Data, Transport, and Document Majors

Upgrade the Rust dependencies whose APIs own external I/O or document processing, including SQLx
0.9, Reqwest 0.13, Base64 0.23, Rand 0.10 where it is not security-format-sensitive, and Lopdf
0.45. Update compile-time SQL, request construction, TLS configuration, PDF behavior, and typed
error handling only as required by the new APIs.

Database schemas and API contracts remain unchanged. SQLx migrations are never edited. Document
outputs receive focused regression coverage so an apparently successful compile cannot hide a
broken export.

### Wave 4: Rust Authentication and Cryptography Majors

Upgrade JsonWebToken 11, backend-admin bcrypt to the shared current major, AES-GCM 0.11, HMAC
0.13, SHA-2 0.11, and related security-sensitive direct dependencies.

The existing token claims, password verification behavior, encrypted-field format, blind-index
output, session boundaries, and secret handling remain compatible. Known vectors and stored-format
round trips must prove compatibility before deployment. No key rotation, token-format redesign, or
password-policy change is part of this wave.

## Error Handling and Compatibility Rules

- A newly surfaced compiler, linter, or runtime error is fixed at its cause; checks are not disabled
  and errors are not swallowed.
- If a latest stable version requires an unrelated domain, schema, or wire-contract redesign, hold
  that dependency at the newest supported version and record the exact upstream constraint.
- If two direct dependencies require incompatible peer or crate versions, prefer the combination
  supported by both rather than installing duplicate top-level owners or forcing resolution.
- Existing Cargo transitive duplicates may remain only when owned by upstream crates. Direct
  dependencies must use the canonical workspace owner in `backend-school`.
- Lockfiles are committed and all release builds use their locked dependency graph.

## Verification Strategy

Each wave starts from a passing baseline and uses focused tests for every migration it performs.
The final candidate tree for a wave must run the applicable matrix below.

### Frontend School

- `npm ci`
- `npm run lint`
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`
- `npm run test:menu-sync`
- `npm run test:static`
- `npm run build`
- Playwright discovery and the non-destructive mocked browser suites affected by upgraded runtime,
  component, editor, export, or navigation dependencies

### Frontend Admin

- `npm ci`
- `npm run lint`
- `npm run check`
- `npm run test:unit`
- `npm run build`

### Backend School

- `cargo fmt --all -- --check`
- Focused package tests for every changed direct dependency boundary
- `cargo test --test static_architecture`
- `cargo check --workspace --all-targets`
- `RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school`
- API-contract generation and comparison when a library migration touches OpenAPI composition

### Backend Admin

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo check --locked`

### Repository and Deployment

- `git diff --check`
- `git status --short`
- Existing contract workflows remain green
- The coordinated production workflow selects the correct release scope and reaches acceptance
- Relevant deployed browser or authenticated smoke checks run after a wave that changes their
  runtime boundary

## Deployment and Rollback

Each wave is squash-merged and deployed separately. The accepted commit before a wave remains its
rollback boundary. A failed wave is fixed forward on its own branch; later waves do not proceed
until the failed wave is accepted or explicitly abandoned.

Frontend-only waves must use the frontend-only release path. A wave touching either Rust service
uses the corresponding backend or full release path and all existing readiness, migration audit,
smoke, and acceptance gates. No database migration is expected, so rollback does not cross a schema
compatibility boundary.

## Impact Assessment

- **Backend:** dependency APIs and compilation may change; domain behavior must not.
- **Frontend:** compiler, lint, formatting, UI dependency, and browser behavior may change; routes
  and user workflows must not.
- **Database:** no schema or migration changes; SQLx 0.9 must continue to use the existing migration
  history unchanged.
- **Permissions:** no permission contract changes.
- **API contracts:** no intentional DTO or endpoint changes; generated artifacts must remain clean.
- **Realtime:** Tokio, Axum, Reqwest, and browser runtime upgrades require existing SSE/WebSocket
  regression coverage.
- **Security and PDPA:** authentication and cryptography waves require format compatibility tests;
  no plaintext sensitive data or secrets may appear in fixtures or logs.
- **Operations:** four bounded deployments replace one high-risk combined deployment.

## Program Completion

The modernization is complete only when all four waves are accepted in production, no direct
dependency remains behind without a recorded upstream compatibility constraint, deprecated direct
packages in scope are removed, and both npm and Cargo lockfiles reproduce the tested graphs from a
clean installation.
