# Tenant-Authenticated Timetable Realtime Design

## Goal

Close the P0 multi-tenant authentication boundary by binding school JWTs, permission caches, user-targeted in-process events, and timetable WebSocket connections to the resolved tenant. A client must never choose its authenticated user or tenant through WebSocket query parameters.

## Scope

This design covers:

- tenant-bound JWT issuance and verification for backend-school
- one shared request-authentication path for HTTP and WebSocket requests
- tenant-aware permission caching and invalidation
- tenant-aware notification, permission-change, and work-change broadcast envelopes
- authenticated and authorized timetable WebSocket upgrades
- server-derived realtime identity and event ownership
- timetable WebSocket heartbeat, message-size enforcement, and reconnect backoff
- Nginx WebSocket upgrade configuration
- regression tests and deployment sequencing

This design does not cover:

- Redis, NATS, PostgreSQL LISTEN/NOTIFY, or multi-replica realtime fan-out
- refresh tokens or per-session database-backed revocation
- general HTTP rate limiting
- timetable query batching or frontend bundle optimization
- redesigning the timetable event protocol beyond identity and authorization enforcement
- database schema changes or migrations

## Chosen approach

Use a strict tenant-bound JWT cutover with the existing HttpOnly authentication cookie. The WebSocket handler authenticates the cookie during the HTTP upgrade request, resolves the tenant from the same Origin/header rules as normal API requests, and derives the realtime identity from the tenant database.

Old JWTs are rejected because they do not contain the new required claims. Users log in again once after deployment. No legacy-token compatibility window is retained because it would preserve the tenant-boundary weakness during that window.

The WebSocket query contract changes to require only `semester_id`. For rolling deployment safety, the backend ignores unknown legacy query fields rather than trusting or rejecting them. This allows the secured backend to be deployed before the frontend removes `user_id`, `name`, and `school_key` from its URL.

## Rejected approaches

### Temporary legacy JWT compatibility

Accepting old tokens and deriving their tenant solely from the current request would reduce forced logins, but it would keep the vulnerable security model alive and add a second authentication path that must later be removed. This is rejected.

### Short-lived WebSocket tickets

An authenticated HTTP endpoint could issue a one-time short-lived WebSocket ticket. This supports channel-specific revocation but requires a ticket store, lifecycle management, and an additional request before every connection. It is unnecessary while a same-site HttpOnly cookie already reaches `school-api.schoolorbit.app`. This is deferred unless future deployment topology prevents cookie-authenticated upgrades.

### Tenant-specific JWT signing keys

Separate signing keys per school would create stronger cryptographic isolation, but would add key provisioning, rotation, lookup, and recovery requirements. A validated tenant claim under the backend-school signing key is sufficient for the current database-per-tenant architecture.

## Security invariants

The implementation must preserve all of these invariants:

1. A backend-school JWT is valid only for the tenant named in its `tenant` claim.
2. The request tenant comes from the central subdomain resolver, never from JWT alone.
3. The JWT tenant and request tenant must match before user data or permissions are read.
4. A permission cache entry is identified by both tenant and user ID.
5. User-targeted in-process events are identified by both tenant and user ID.
6. A timetable WebSocket client cannot choose its server-side user ID, display name, or room tenant.
7. The semester room must exist in the resolved tenant database.
8. Connecting requires timetable read or manage permission.
9. Edit-intent and drag-related client events require timetable manage permission.
10. Every relayed client event receives its user ID from the authenticated actor, replacing any value in the payload.
11. Authentication tokens, national IDs, passwords, and encryption keys are never logged.

## JWT contract

`backend-school/src/modules/auth/models.rs::Claims` gains these fields:

```rust
pub tenant: String,
pub iss: String,
pub aud: String,
pub token_version: u8,
```

Constants live beside `JwtService`:

```text
issuer: schoolorbit-backend-school
audience: schoolorbit-school-app
token version: 1
```

`JwtService::generate_token` receives the normalized tenant subdomain resolved during login. `JwtService::verify_token` requires `exp`, `sub`, `iss`, and `aud`, validates issuer and audience through `jsonwebtoken::Validation`, and rejects any token whose `token_version` is not exactly `1`.

Tenant comparison remains request-specific. A shared authentication helper performs this sequence:

1. extract Bearer token, falling back to `auth_token` cookie
2. verify signature and registered claims
3. derive the normalized request tenant with `extract_subdomain_from_request`
4. compare `claims.tenant` with the request tenant using exact normalized strings
5. parse `claims.sub` as the authenticated user UUID

The helper returns authenticated claims/user identity without querying a database. Callers that require a tenant database resolve it only after the comparison succeeds.

### HTTP integration

`auth_middleware`, permission loading, and request-context helpers use the shared authentication helper rather than each parsing cookies and JWTs independently. `current_user_tenant_context_from_claims` repeats the tenant equality check as defense in depth before returning a tenant pool and user ID.

Missing/invalid/expired/legacy tokens return `401 Unauthorized`. A malformed or missing request tenant follows the existing subdomain error policy. A valid token for a different tenant returns `401 Unauthorized` without revealing whether the target tenant contains the same user UUID.

## Tenant-aware permission cache

`PermissionCache` replaces its `DashMap<Uuid, CacheEntry>` with a composite key:

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TenantUserKey {
    pub tenant: String,
    pub user_id: Uuid,
}
```

The public cache operations become tenant-explicit:

```text
get(tenant, user_id)
set(tenant, user_id, permissions)
invalidate_user(tenant, user_id)
invalidate_tenant(tenant)
```

Global `clear_all()` is removed from normal mutation flows. Role and organization-permission changes invalidate only the resolved tenant. User role, membership, staff, and delegation changes invalidate only the affected user in the resolved tenant.

`get_cached_user_permissions` and `load_actor_context` receive the normalized tenant string. `actor_tenant_context` remains the primary handler entry point because it already owns the resolved `TenantContext` and can pass the subdomain into permission loading.

TTL behavior remains 30 minutes in this P0 change. Distributed invalidation and TTL tuning belong to the later multi-replica reliability plan.

## Tenant-aware in-process events

All process-global channels whose targets currently use a bare user UUID gain tenant identity.

### Permission events

`PermissionChangeEvent` gains `tenant: String`. Both targeted and all-user events apply only when the SSE connection tenant equals the event tenant. `AppState::notify_permission_changed` and `notify_all_permissions_changed` require the tenant argument.

### Notifications

Replace the `(Uuid, Notification)` tuple with an explicit envelope:

```rust
pub struct TenantNotificationEvent {
    pub tenant: String,
    pub user_id: Uuid,
    pub notification: Notification,
}
```

Notification creation and calendar-reminder flows pass the already-resolved subdomain. Notification SSE emits an event only when both tenant and user ID match.

### Work events

`WorkChangeEvent` gains `tenant: String`. Work-item and workflow-window mutation handlers pass their resolved tenant. SSE clients ignore work events from other tenants.

These changes do not make the channels multi-replica. They close the cross-tenant collision boundary inside one process.

## Timetable WebSocket access model

### Query contract

`WsParams` contains only:

```rust
pub semester_id: Uuid,
```

Unknown fields are ignored temporarily for rolling compatibility. No backend code reads `user_id`, `name`, or `school_key` from the query.

### Upgrade data flow

The handler performs all checks before `WebSocketUpgrade::on_upgrade`:

```text
Origin/X-School-Subdomain
    -> central tenant normalization
    -> cookie/Bearer JWT verification
    -> exact JWT tenant match
    -> tenant database pool
    -> tenant-aware ActorContext
    -> read/manage permission policy
    -> active user realtime identity
    -> semester existence check
    -> WebSocket upgrade
```

The handler returns `Result<Response, AppError>` or an equivalent response type so authentication and authorization failures remain normal HTTP responses before the protocol upgrade.

### Realtime access service

Create a focused academic realtime service rather than placing SQL in the WebSocket handler. It accepts `&PgPool`, `&ActorContext`, and `semester_id`, and returns:

```rust
pub struct TimetableSocketAccess {
    pub user_id: Uuid,
    pub display_name: String,
    pub can_manage: bool,
}
```

The service:

- requires `ACADEMIC_COURSE_PLAN_READ_ALL` or `ACADEMIC_COURSE_PLAN_MANAGE_ALL`
- sets `can_manage` only for manage permission or wildcard permission
- queries only `username`, `title`, `first_name`, and `last_name` for an active user
- verifies that `academic_semesters.id = semester_id` exists in the tenant database
- formats a non-empty display name, using username only as a fallback

It must not select password hash, national ID, email, phone, address, or other profile fields.

### Room identity

The room key continues to be `<tenant-subdomain>:<semester-id>`, but the tenant component comes only from `TenantContext.subdomain`. Presence user ID and display name come only from `TimetableSocketAccess`.

### Incoming event authorization

Move client-event normalization into a pure, testable function with this conceptual interface:

```text
sanitize_client_event(event, authenticated_user_id, can_manage)
    -> allowed normalized event | ignored/forbidden event
```

Policies:

- presence-safe cursor movement may be relayed for any authorized reader
- drag start/move/end, user activity, drop intent, entry intent, and client-requested table refresh require `can_manage`
- server-originated mutation event variants received from a client remain rejected
- every allowed event is reconstructed with `authenticated_user_id`
- malformed JSON is ignored without echoing its content to logs
- text frames larger than 64 KiB close the connection with WebSocket code `1009`

HTTP handlers remain the only source of persisted timetable mutations. WebSocket edit intents continue to be ephemeral collaboration signals and never write to the database.

## Heartbeat and connection lifecycle

The backend sends a WebSocket Ping every 30 seconds. It records the last inbound frame/Pong time and closes a connection that has been silent for 90 seconds. Browser WebSocket clients automatically answer protocol-level Ping frames with Pong frames.

Room cleanup preserves current multi-tab semantics:

- joining a second tab increments the connection count without broadcasting a second `UserJoined`
- disconnecting a non-final tab does not broadcast `UserLeft`
- timeout, client close, send failure, and receiver failure all execute the same leave-room cleanup path

The frontend reconnect policy changes from a fixed 3-second delay to exponential backoff with jitter:

```text
base delays: 1s, 2s, 4s, 8s, 16s, capped at 30s
jitter: 80%-120% of the selected delay
```

The attempt counter resets after a successful open. Explicit page teardown disables reconnect. When the browser is offline, the store waits for the `online` event rather than creating repeated sockets.

## Frontend contract

`connectTimetableSocket` retains the local current-user ID only for ignoring reflected events in the UI, but sends only `semester_id` in the WebSocket URL. The display name and school key are removed from its public parameters and query construction.

The timetable page passes:

```text
semester_id
current_user_id
```

The local user ID is never treated as authenticated server input. Server events remain authoritative for remote presence identity.

No token is placed in the URL, JavaScript storage, or WebSocket subprotocol. Authentication relies on the existing secure HttpOnly `auth_token` cookie.

## Nginx contract

Add a dedicated `/ws/` location before the general `/` location in `nginx-configs/school-api.schoolorbit.app.conf` with:

- `proxy_http_version 1.1`
- `proxy_set_header Upgrade $http_upgrade`
- `proxy_set_header Connection "upgrade"`
- forwarded Host, real IP, forwarded-for, forwarded-proto, and Origin headers
- `proxy_read_timeout` longer than the 30-second heartbeat interval
- no logging of query strings containing legacy identity values during the rolling transition when operational configuration allows it

The backend remains responsible for Origin/tenant validation. Nginx forwarding is not an authentication boundary.

## Error handling

| Condition | Result before/after upgrade |
|---|---|
| No authentication token | HTTP 401 before upgrade |
| Invalid, expired, legacy, wrong issuer/audience/version token | HTTP 401 before upgrade |
| JWT tenant differs from request tenant | HTTP 401 before upgrade |
| Tenant not found or unavailable | Existing safe 404/500 tenant error before upgrade |
| User inactive or missing | HTTP 401 before upgrade |
| No timetable read/manage permission | HTTP 403 before upgrade |
| Semester missing in tenant | HTTP 404 before upgrade |
| Read-only client sends edit event | Event rejected and not broadcast; connection remains open |
| Oversized frame | WebSocket close code 1009 |
| Heartbeat timeout | Connection closes and normal room cleanup runs |
| Broadcast receiver lag | Existing full-refresh event behavior remains |

Error responses and logs must not expose token contents, database connection strings, national IDs, password hashes, or raw malformed client payloads.

## Testing strategy

### JWT unit tests

- valid token contains tenant, issuer, audience, and token version
- missing/wrong issuer is rejected
- missing/wrong audience is rejected
- wrong token version is rejected
- expired token is rejected
- request tenant mismatch is rejected
- Bearer extraction has precedence over cookie extraction

### Permission cache unit tests

- same user UUID can store different permissions in two tenants
- invalidating one tenant/user does not remove the other tenant entry
- invalidating a tenant removes all and only that tenant's entries
- TTL expiry continues to remove stale entries

### Event-envelope tests

- notification event matches only the same tenant and user
- targeted permission event matches only the same tenant and user
- tenant-wide permission event matches every user in that tenant only
- work event matches only the same tenant

### WebSocket policy tests

- no read/manage permission is forbidden
- read and manage permissions can connect
- wildcard permission can connect and manage
- read-only actors cannot relay drag/edit/refresh events
- manage actors can relay allowed edit intents
- client user IDs are replaced by the authenticated actor ID
- server-only mutation variants sent by a client are rejected
- room key uses the resolved tenant, not query data

### Frontend/static tests

- WebSocket URL contains only semester identity from the server contract
- no `school_key`, display name, or user ID is serialized into the query
- reconnect delay grows exponentially, is capped, and includes jitter
- explicit disconnect cancels reconnect
- the timetable page passes the revised local connection parameters

### Architecture guards

- WebSocket handler uses central tenant/auth context
- `WsParams` has no user identity or tenant fields
- raw JWT parsing is not duplicated in feature modules
- permission-cache invalidation APIs always receive tenant identity

## Verification commands

Backend verification:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test --bin backend-school -- --skip modules::auth::tests::auth_tests::test_login_success --skip modules::auth::tests::auth_tests::test_login_invalid_credentials
cargo test --test static_architecture
cargo clippy --all-targets --all-features -- -D warnings
```

Database-backed authentication/realtime tests additionally run with a safe `TEST_DATABASE_URL` according to `docs/TESTING.md` when that environment is available.

Frontend verification:

```bash
cd frontend-school
npm run check
npm run test:static
npx eslint src/lib/stores/timetable-socket.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte'
npx prettier --check src/lib/stores/timetable-socket.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte'
npm run build
```

Repository verification:

```bash
git diff --check
git status --short
```

Svelte files are analyzed with the repository's Svelte 5 tooling/autofixer before completion.

## Deployment sequence

1. Deploy backend-school with strict JWT verification and secured WebSocket handling.
2. Existing JWTs become invalid; users are redirected to login and receive tenant-bound tokens.
3. Old frontend builds continue to send legacy WebSocket query fields, which the backend ignores.
4. Deploy frontend-school with the reduced query contract and reconnect changes.
5. Deploy the Nginx WebSocket location before or with the backend rollout if the active production configuration lacks Upgrade forwarding.
6. Verify login, `/api/auth/me`, timetable HTTP loading, WebSocket upgrade, presence, edit collaboration, and reconnect against a sandbox tenant.
7. Monitor 401/403 rates and WebSocket reconnect volume during rollout; do not log token or legacy query contents.

Rollback requires rolling backend and frontend to the previous compatible pair. Tokens issued by the new backend contain extra claims and remain decodable by the previous serde model only if unknown fields are accepted, but rollback validation must not rely on that behavior; users may be required to log in again after either direction of rollback.

## Acceptance criteria

The change is complete only when:

1. unauthenticated timetable WebSocket upgrades return 401
2. authenticated users without timetable read/manage permission receive 403
3. a tenant-A token cannot authenticate an HTTP or WebSocket request resolved as tenant B
4. WebSocket room tenant, user ID, and display name are server-derived
5. the permission cache separates identical user UUIDs across tenants
6. user-targeted in-process events cannot match another tenant with the same UUID
7. read-only WebSocket clients cannot broadcast editing intents
8. active authorized users can connect, receive state, collaborate, disconnect, and reconnect
9. heartbeat timeout removes stale connections and room presence
10. no authentication token or plaintext PII is logged
11. focused tests and project verification commands report their actual pass/fail state
12. no database migration is added or modified
