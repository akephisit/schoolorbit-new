# School session activity renewal

The approved scope is School browser authentication only. The auth domain owns deadlines,
credential lifecycle and cache fences; its HTTP adapter owns cookies, origin and CSRF. One
application controller reports human interaction on authenticated pages, including preloaded
timetables. Admin authentication and business page APIs remain separate.

`POST /api/auth/activity` accepts `{ expectedSessionId: UUID }` and returns the normal envelope
with `SessionState`: `sessionId`, `serverNow`, `idleExpiresAt`, `absoluteExpiresAt` and
`nextRenewalAt`. Login's `data.user.session` and `/auth/me` expose the same state using already
loaded session metadata. The next renewal is persisted `last_seen_at + 5 minutes`. Responses
are private and no-store. A session mismatch returns 409 `session_changed` without clearing
cookies or mutating session state; the client reconciles `/me` once.

Ordinary HTTP, SSE and WebSocket checks validate without extending human idle. Required origin,
CSRF and expected-session checks precede maintenance. Only activity can extend idle (at most
once per five minutes), preserving normal 2h/12h and remembered 7d/30d idle/absolute limits.
Absolute expiry never slides. Safe validation caches remain 60 seconds and realtime validation
five minutes with local expiry and synchronous invalidation fences.

Periodic rotation becomes acknowledged. A due activity stages one pending B while A stays
current. B is reproducible from the stable session HMAC key, a versioned domain, fixed-width
tenant/session UUIDs, raw A and a PostgreSQL-microsecond pending timestamp. Only B's hash and
timestamp persist. Repeated A activity verifies the derived digest and resends the same B.
Pending credentials have no shorter timeout than the session. A valid B request promotes it
under lock after security checks, moves A to the exact 60-second previous window, clears pending
and starts the rotation clock. Only activity sends periodic replacement cookies. Promotion
invalidates every cached credential for the session even without a replacement response.
Previous/pending acceptance is never cached. Password changes retain their atomic semantics and
clear pending; revocation clears it too. All credential allocation checks cover all three digest
columns under the established user/session/token lock order.

Migration 145 adds paired pending columns, length/time/pair checks and a partial unique index;
applied migrations remain immutable. The supported deployment remains one School backend
process. No raw credentials enter storage, caches, logs or diagnostics. Safe rejection reasons
distinguish expiry/revocation/inactive/unknown/expired-previous where known. Very delayed older
Set-Cookie responses across multiple rotations remain bounded; no arbitrary delivery guarantee
is claimed.

Verification covers invalid-CSRF and expected-session no-mutation, lost/repeated stage replies,
promotion/grace/concurrency, quiet reading, expiry/revocation/inactive/tenant denial, absolute
clamping, five-minute write coalescing, three-column collisions, timestamp roundtrip, warm-cache
invalidation and password changes. Auth HTTP, realtime, generated contracts and the central
browser controller are checked separately with owned disposable fixtures.
