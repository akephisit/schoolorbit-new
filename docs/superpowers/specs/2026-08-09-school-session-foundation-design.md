# School App Server-Side Session Foundation Design

**Date:** 2026-08-09

**Status:** Approved design

**Scope:** `backend-school` and `frontend-school` only

## Context

The school application currently authenticates browser users with a signed JWT stored in the backend host's `auth_token` cookie. The JWT has a fixed seven-day lifetime, while the login handler gives the cookie a one-day or thirty-day lifetime. Logout deletes only the browser cookie, password changes do not revoke existing tokens, and authenticated request paths do not all enforce the current database user status consistently. The same route-local authentication middleware is attached repeatedly throughout the router.

The frontend and backend also have different origins. A tenant frontend such as `https://demo.schoolorbit.app` calls `https://school-api.schoolorbit.app` directly with credentials. The backend cookie therefore belongs only to `school-api.schoolorbit.app`; the tenant frontend's SvelteKit server cannot read it during SSR. Expanding the cookie to `.schoolorbit.app` would expose it to every tenant subdomain and is not acceptable.

This design replaces browser JWT authentication with tenant-local opaque server-side sessions. It establishes one authentication boundary for ordinary staff, student, and parent application users before later work on account activation, password recovery, the public admission portal, notification authorization, or a same-origin frontend BFF.

## Goals

- Make every browser session individually revocable without storing a bearer token in plaintext server-side.
- Support concurrent sessions on multiple devices, revoking the current, a selected, or all sessions.
- Revoke other sessions when a password changes and reject every protected request from an inactive user.
- Give normal sessions a two-hour idle timeout and twelve-hour absolute lifetime.
- Give remembered sessions a seven-day idle timeout and thirty-day absolute lifetime.
- Rotate session tokens safely while tolerating concurrent browser requests.
- Centralize tenant resolution, session validation, active-user enforcement, and typed request identity.
- Protect cookie-authenticated mutations with exact Origin/tenant checks and a session-bound CSRF token.
- Add durable, enumeration-resistant login throttling without storing raw usernames, source addresses, credentials, or tokens.
- Minimize `/api/auth/me` to identity and authorization data needed by the application shell.
- Preserve the generated OpenAPI and TypeScript contract workflow.
- Force one clean re-login at rollout instead of supporting old and new authentication simultaneously.

## Non-Goals

- No changes to `backend-admin` or `frontend-admin`.
- No public admission portal session redesign.
- No activation, invitation, password-reset, MFA, or deterministic-password replacement flow.
- No change to the authorization or recipient policy of generic notification creation.
- No redesign of the full `/api/auth/me/profile` PII contract in this project; that remains separate follow-up work.
- No same-origin proxy/BFF or server-side frontend auth bootstrap.
- No shared multi-replica event transport; the database remains authoritative and realtime connections revalidate on a bounded heartbeat until SCALE-001 is implemented.
- No bearer-token compatibility layer for browser user authentication.

## Chosen Approach

Use an opaque random session token in a backend-host-only cookie and store only its SHA-256 digest in the tenant database. Every protected HTTP request validates the session through the tenant database and joins the owning user so inactive accounts fail closed. This adds one indexed lookup per authenticated request in exchange for immediate revocation, simple ownership rules, minimal browser credentials, and a single understandable security boundary.

A hybrid short-lived JWT plus refresh-token design was rejected because it adds refresh races and either delays revocation until the access token expires or reintroduces a database/cache lookup. A user-level token revision was rejected because it cannot revoke one device independently.

## Session Data Model

Add the next sequential tenant migration; never modify an applied migration. The migration introduces `auth_sessions` with:

- `id UUID PRIMARY KEY`
- `user_id UUID NOT NULL REFERENCES users(id)`
- `current_token_hash BYTEA NOT NULL`
- `previous_token_hash BYTEA NULL`
- `previous_token_valid_until TIMESTAMPTZ NULL`
- `remember_me BOOLEAN NOT NULL`
- `device_label TEXT NOT NULL`
- `created_at TIMESTAMPTZ NOT NULL`
- `last_seen_at TIMESTAMPTZ NOT NULL`
- `idle_expires_at TIMESTAMPTZ NOT NULL`
- `absolute_expires_at TIMESTAMPTZ NOT NULL`
- `rotated_at TIMESTAMPTZ NOT NULL`
- `revoked_at TIMESTAMPTZ NULL`
- `revocation_reason TEXT NULL`

Required indexes and constraints:

- unique index on `current_token_hash`;
- partial unique index on non-null `previous_token_hash`;
- index on active sessions by `user_id` and `absolute_expires_at`;
- index supporting bounded cleanup of expired or long-revoked rows;
- checks that idle and absolute expiry follow creation, previous-token fields appear together, and revocation reasons appear only with `revoked_at`.

The coarse `device_label` is derived server-side from browser and operating-system families. The raw User-Agent and source address are discarded rather than persisted in the session row. The label is display metadata only and is never an authorization signal.

## Login Throttle Data Model

Add `auth_login_throttles` in the same migration with one row per throttle bucket:

- `bucket_kind` constrained to `identifier` or `source`;
- `bucket_hash BYTEA NOT NULL`;
- `failure_count INTEGER NOT NULL`;
- `window_started_at TIMESTAMPTZ NOT NULL`;
- `blocked_until TIMESTAMPTZ NULL`;
- `updated_at TIMESTAMPTZ NOT NULL`;
- primary key on `(bucket_kind, bucket_hash)`.

Bucket hashes use domain-separated HMAC-SHA256 with a new required `SESSION_HMAC_KEY`:

- identifier bucket input: tenant identifier plus normalized username;
- source bucket input: tenant identifier plus the validated client network address.

The backend must use only the direct peer or a source address produced by explicitly trusted reverse-proxy handling. It must never trust an arbitrary forwarded header. If a trusted source address is unavailable, identifier throttling still applies and no raw fallback value is stored.

Failures use a fifteen-minute window. The first four failures do not block. From the fifth failure onward, `blocked_until` increases exponentially from one second to a maximum of thirty seconds. A source bucket uses a higher twenty-failure threshold before the same capped delay begins. A successful login clears the matching identifier bucket; source buckets expire through bounded cleanup. There is no permanent account lockout.

Unknown usernames run a fixed dummy bcrypt verification before returning the same authentication error as a wrong password, inactive account, or throttled identifier. HTTP `429` may include `Retry-After`, but the response body remains generic and never confirms account existence.

## Token and Cookie Contract

- Generate session tokens with an operating-system CSPRNG using at least 256 bits of entropy and base64url encoding.
- Store only SHA-256 token digests and compare digests in constant time where comparison is performed in application code.
- Name the cookie `__Host-schoolorbit_session`.
- Set `Secure`, `HttpOnly`, `SameSite=Lax`, and `Path=/`; never set `Domain`.
- A normal session cookie has no persistent `Max-Age`. The server still enforces a two-hour idle and twelve-hour absolute lifetime.
- A remembered session cookie has a maximum age of thirty days. The server enforces a seven-day idle and thirty-day absolute lifetime.
- Never return the session token outside its `Set-Cookie` header or include it in JSON, other headers, logs, errors, audit payloads, or realtime events.

Rotate the opaque token when the current token has not rotated for fifteen minutes. In one transaction, move the current hash to `previous_token_hash`, set its grace expiry to sixty seconds, and store the new current hash. Return the new cookie only after the transaction commits. Requests presenting the previous token during the grace window authenticate against the same session but cannot rotate it again. After the grace window, clear both previous-token fields during normal session activity or cleanup.

## CSRF and Origin Contract

Use `SESSION_HMAC_KEY` with a separate domain label to derive a CSRF token from the raw session token. The database does not need the raw CSRF value or a separate reversible secret.

- Login and `/api/auth/me` expose the current CSRF token through `X-CSRF-Token`.
- CORS exposes only that response header to the exact configured tenant frontend origin.
- `frontend-school` retains the value only in module memory and never writes it to local or session storage.
- Every cookie-authenticated `POST`, `PUT`, `PATCH`, and `DELETE` sends `X-CSRF-Token`. The only exception is idempotent logout when no valid session exists, where the backend may expire stale cookies after exact-Origin validation.
- The backend recomputes the expected value from the presented session token and uses a constant-time comparison.
- Safe methods do not require the CSRF header but still require normal session and tenant validation when protected.
- Login requires exact tenant Origin/Referer validation even though it has no session-bound CSRF token yet.

When token rotation succeeds, the response includes both the new cookie and the new CSRF response header. A concurrent request using the previous token and previous CSRF value remains valid during the same grace window.

## Central Authentication Boundary

Replace the stateless JWT parser with a state-aware session validator shared by HTTP, SSE, and WebSocket entry points. For an HTTP request it:

1. resolves the tenant through the existing canonical resolver;
2. reads and hashes the backend-host session cookie;
3. loads a non-revoked, non-expired session and its owning user;
4. rejects the request unless the user status is exactly `active`;
5. enforces idle and absolute expiry;
6. rotates the token when due;
7. updates `last_seen_at` and idle expiry at most once every five minutes;
8. inserts a typed `AuthenticatedSession` containing tenant context, session ID, user ID, username, and user type into request extensions.

Build protected endpoints under one router-level state-aware middleware rather than attaching the same middleware to each route. Public routes, the public admission portal, internal deployment routes, health/readiness routes, and deploy-key routes remain separate. Feature handlers continue to obtain actor or current-user context from shared request-context helpers; those helpers consume `AuthenticatedSession` instead of reparsing credentials.

Permission loading remains a separate authorization step. A valid session without the required permission produces `403`, not `401`.

## API Contracts

All JSON responses keep the standard `ApiResponse<T>` envelope and are registered in the backend OpenAPI document.

| Endpoint | Authentication | Behavior |
|---|---|---|
| `POST /api/auth/login` | Exact tenant origin | Verify credentials and throttle state, create session, set cookie, return minimal current user |
| `POST /api/auth/logout` | Optional current session plus exact origin | Revoke a valid current session before expiring both new and legacy cookies; expire stale cookies idempotently when no valid session exists |
| `GET /api/auth/me` | Session | Return minimal identity/authorization data and refresh the in-memory CSRF token |
| `GET /api/auth/sessions` | Session | List only the current user's active sessions and identify the current session |
| `DELETE /api/auth/sessions/{id}` | Session plus CSRF | Revoke only a session owned by the current user; deleting the current session also expires the cookie |
| `POST /api/auth/logout-all` | Session plus CSRF | Revoke all of the current user's sessions including the caller and expire the cookie |
| `POST /api/auth/me/change-password` | Session plus CSRF | Verify current password, update its hash, revoke all other sessions, and rotate the current session atomically |

The default current-user DTO contains only:

- id and username;
- first and last name;
- user type and active status;
- primary role name;
- profile image file ID;
- effective permissions.

It does not contain national ID, email, phone, date of birth, address, or other profile/PII fields. Full profile and future step-up PII access remain separate contracts.

## Frontend Behavior

Keep the existing direct browser-to-backend topology and `credentials: 'include'`. Do not share a session cookie at `.schoolorbit.app` and do not introduce a BFF in this project.

The shared API client:

- captures `X-CSRF-Token` from login, `/api/auth/me`, and rotation responses;
- appends it automatically to cookie-authenticated mutation requests;
- keeps transport status separate from the generated JSON envelope so authentication code can distinguish `401`, `403`, `429`, and `503` without response casts;
- clears the CSRF value on confirmed `401` or logout;
- never persists session or CSRF credentials in browser storage.

The auth store continues to bootstrap in the browser by calling `/api/auth/me` because the SvelteKit server at the tenant frontend origin cannot receive the backend-host cookie. Behavior by status is explicit:

- `401`: clear user and permissions, remember only the non-sensitive redirect path, then go to login;
- `403`: keep the authenticated state and show the denied route or action;
- `429`: show a generic retry message honoring `Retry-After`;
- `503`: keep the previous authenticated state when one exists and show a retryable availability state rather than logging the user out.

Add a shared authenticated `/account/security` route available to staff, student, and parent users through guard-only route metadata. It lists active sessions, marks the current session, supports revoking another device, and provides logout-all. The shared account menu links every user type to this route, and the existing staff and student settings pages link to it as well. Session state and mutation logic live in one shared feature boundary.

## Password Change

Password change runs in one tenant database transaction:

1. lock and load the active user and current session;
2. verify the current password;
3. validate and hash the new password;
4. update the password hash;
5. revoke every other active session with reason `password_changed`;
6. rotate the current session token;
7. commit before setting the new cookie.

A later password-reset or account-recovery flow will revoke every session including the recovery caller and require a normal login. That flow is outside this design.

## Realtime Authentication

SSE and WebSocket handshakes call the same session service and bind the authenticated tenant, user ID, and session ID server-side. Clients cannot supply or replace identity after connection establishment.

- Revocation publishes a process-local session/user signal so matching connections on the same replica close immediately.
- Long-lived connections revalidate the session and active-user status from the tenant database every thirty-second heartbeat.
- Expired, revoked, inactive, or tenant-mismatched connections close with a policy/authentication reason that contains no sensitive data.
- With future multiple replicas, database revalidation bounds cross-replica revocation to one heartbeat. SCALE-001 will replace that bound with shared delivery without changing the authoritative session model.

## Error Semantics

| Condition | Status | Frontend meaning |
|---|---:|---|
| Missing, malformed, expired, revoked, or tenant-mismatched session | `401` | Clear auth state and require login |
| Inactive user | `401` | Clear auth state and require administrator action |
| Valid session without permission | `403` | Stay logged in; deny the operation |
| Invalid Origin or CSRF token | `403` | Stay logged in; reject the request and record a redacted security event |
| Active login throttle | `429` | Retry after the bounded delay without confirming account existence |
| Tenant/session database unavailable | `503` | Preserve existing client state and offer retry |

Logout is idempotent. A missing or invalid session cannot be revoked, but the endpoint still expires the session and legacy JWT cookies. A database failure while revoking a known valid session returns `503` without expiring the new session cookie or clearing frontend auth state, so the user can retry and the UI does not claim server-side revocation succeeded.

## Audit, Logging, and Retention

Emit structured, redacted events for login success/failure category, session creation, rotation failure, current/selected/all revocation, password-change revocation, CSRF failure, and realtime disconnect. Events may contain tenant ID, user ID when known, session UUID, reason code, and timestamps. They must not contain raw username on failed lookup, source address, User-Agent, cookie, token, CSRF value, password, national ID, or request body.

Retain expired or revoked session metadata for thirty days, then delete it in bounded batches. Opportunistic cleanup runs during login and session-list operations; it must not introduce a second always-on scheduler. Throttle buckets whose windows ended more than one day ago are deleted in the same bounded cleanup path.

## Rollout and Rollback

1. Run the complete school-stack verification baseline before implementation and record pre-existing failures separately.
2. Add only the new sequential migration and validate it against a fresh PostgreSQL tenant database.
3. Generate and validate the OpenAPI and TypeScript contracts.
4. Exercise the full flow against a disposable or staging tenant through direct backend and reverse-proxy paths.
5. Provision one stable, random `SESSION_HMAC_KEY` through the deployment secret store without printing it or placing it in tracked configuration.
6. Rotate `JWT_SECRET` at cutover so every legacy JWT is cryptographically invalid, even for users who have not revisited the application.
7. Use the existing backend-school maintenance and centralized all-tenant migration gate.
8. Deploy the session-enabled backend and frontend, expire `auth_token`, and require one clean login per user.
9. Verify login, `/api/auth/me`, a protected read, a CSRF-protected mutation, password change, selected-device revocation, logout-all, SSE, and WebSocket behavior.

The migration is additive, so an emergency rollback can leave the new tables in place. A rolled-back backend uses the rotated `JWT_SECRET`; users log in again and receive only newly signed JWTs. The session cookie is ignored by the old binary. No migration file or SQLx checksum is edited during rollback.

## Testing Strategy

### Backend unit tests

- random token encoding, hashing, CSRF derivation, and constant-time comparison boundaries;
- normal and remembered idle/absolute expiry decisions;
- rotation eligibility and previous-token grace handling;
- throttle windows, thresholds, capped delay, success reset, and cleanup selection;
- generic authentication errors and status mapping;
- cookie attributes and legacy-cookie expiry.

### Backend database and HTTP tests

- fresh migration application and schema constraints;
- tenant isolation and current-user session ownership;
- active, inactive, expired, revoked, current-hash, and previous-hash validation;
- concurrent rotation requests without double rotation;
- revoke current, selected, and all sessions;
- atomic password update, other-session revocation, and current-session rotation;
- identifier/source throttling without raw values;
- exact Origin and CSRF enforcement for every mutation class;
- `401`, `403`, `429`, and `503` behavior;
- minimal `/api/auth/me` response with no national ID or unnecessary PII;
- SSE and WebSocket handshake plus revocation/heartbeat closure.

### Frontend tests

- generated DTO ownership for every auth/session endpoint;
- CSRF capture, memory-only storage, mutation injection, rotation update, and clearing;
- differentiated `401`, `403`, `429`, and `503` state transitions;
- session list and ownership-safe revoke actions;
- login/logout and password-change behavior;
- Svelte analysis and component tests for loading, error, empty, current-device, and mutation states;
- Playwright with two isolated browser contexts to prove multi-device login, selected revocation, logout-all, forced legacy re-login, and tenant isolation.

### Required verification

Run focused tests plus the applicable `.rules` matrix:

- backend-school formatting, static architecture tests, compile check, focused auth/session tests, and the full backend test suite;
- frontend-school lint, Svelte check, static tests, API contract generation/check/tests, and production build;
- fresh tenant migration tests;
- authenticated smoke and browser workflows through the deployed proxy path;
- `git diff --check`, final diff review, and `git status --short`.

## Implementation Slices

1. Add the migration, session/throttle domain types, pure lifecycle decisions, and database services with tests.
2. Add the central state-aware middleware, request context, login/logout/session/password APIs, CSRF/Origin enforcement, and OpenAPI contract.
3. Regenerate TypeScript contracts and update the API client, auth store, `/account/security` route, and role-settings links.
4. Integrate SSE/WebSocket revocation, cleanup, redacted audit events, rollout documentation, smoke coverage, and end-to-end verification.

Each slice is reviewable and test-owned, but the authentication cutover is enabled only after all four slices and the staging verification are complete.

## Success Criteria

- No legacy JWT authenticates after cutover.
- The database contains only hashes of browser session tokens and throttle identifiers.
- Every protected HTTP request rejects revoked, expired, tenant-mismatched, or inactive-user sessions.
- Current, selected, all-device, and password-change revocation behave as specified.
- Normal and remembered sessions obey both idle and absolute limits.
- Cookie-authenticated mutations fail without the exact Origin and valid session-bound CSRF token.
- Login failures do not reveal account existence and are durably throttled without permanent lockout.
- `/api/auth/me` exposes only the approved identity/authorization fields.
- Frontend auth behavior distinguishes authentication, authorization, throttling, and availability failures.
- SSE and WebSocket connections use the same authoritative session identity and close on revocation within the documented bound.
- Existing migrations remain byte-for-byte unchanged.
- No `backend-admin` or `frontend-admin` file changes.
