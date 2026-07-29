# Notification SSE Proxy Design

## Problem

The browser connects to `/api/notifications/stream`, but the tracked nginx
configuration applies its unbuffered SSE settings only to
`/api/v1/.../stream`. The notification stream therefore falls through to the
ordinary proxy location, where response buffering is enabled by default.

Backend-school emits an SSE keepalive at most every 15 seconds. In production,
an authenticated notification request returns normally with the tenant CORS
origin, while the stream returns neither headers nor bytes within 22 seconds.
Cloudflare eventually returns `524`, and the browser reports a secondary CORS
error because that Cloudflare error response does not carry the application's
CORS headers.

## Approaches

1. Add an exact nginx location for `/api/notifications/stream`. This is the
   selected approach because it fixes the buffering boundary where the data is
   being held and limits the special proxy behavior to the known endpoint.
2. Add only an `X-Accel-Buffering: no` response header in backend-school. Nginx
   supports this, but relying only on an application header leaves the tracked
   proxy route and its timeouts incorrect.
3. Increase Cloudflare's timeout or bypass its proxy. This hides the symptom,
   does not deliver keepalives promptly, and adds an unnecessary operational
   exception.

## Design

Replace the stale `/api/v1/.../stream` nginx location with an exact
`/api/notifications/stream` location. Preserve the existing SSE-specific
settings:

- buffering and proxy cache disabled;
- HTTP/1.1 with an empty `Connection` header;
- chunked transfer enabled;
- long read and send timeouts;
- tenant-aware CORS headers and credential support;
- the existing upstream and forwarded request headers.

Backend-school's handler, authentication, 15-second keepalive, event payloads,
frontend reconnect behavior, database schema, permissions, and API contracts
remain unchanged. The backend deployment workflow already uploads this tracked
configuration, validates it with `nginx -t`, installs it, and reloads nginx.

## Failure Handling

If the replacement configuration fails `nginx -t` or reload, the existing
deployment workflow restores the prior active configuration and fails the run.
The frontend reconnect remains as protection for genuine transient connection
failures, but a quiet healthy stream must no longer cycle through `524`.

## Verification

1. Add a static proxy regression test that resolves the exact notification SSE
   location and requires its streaming, timeout, upstream, and CORS directives.
2. Confirm the test fails against the stale route, then passes after the
   configuration change.
3. Run the complete frontend static suite and repository diff checks.
4. Push and monitor the backend-school deployment, including nginx validation,
   reload, and readiness.
5. Run the authenticated production smoke test.
6. Through the Cloudflare hostname, verify that the authenticated stream returns
   `200`, the expected tenant CORS header, and multiple keepalive chunks within
   the backend's heartbeat interval without exposing credentials or cookies.
