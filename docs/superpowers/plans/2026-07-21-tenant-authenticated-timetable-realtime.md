# Tenant-Authenticated Timetable Realtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bind every school authentication, permission-cache, in-process event, and timetable WebSocket decision to the server-resolved tenant so a client cannot impersonate another tenant or user.

**Architecture:** Introduce one tenant-aware JWT/request-authentication path and make tenant identity explicit at every process-global cache/channel boundary. Authenticate and authorize timetable WebSocket upgrades before switching protocols, derive realtime identity from the tenant database, sanitize every client event, and retain the HttpOnly-cookie browser flow with bounded reconnect behavior.

**Tech Stack:** Rust 2021, Axum, jsonwebtoken 9, sqlx/PostgreSQL, Tokio broadcast/WebSocket, DashMap, SvelteKit 5, TypeScript, Svelte stores, Node test runner, Nginx

## Global Constraints

- JWT issuer is exactly `schoolorbit-backend-school`.
- JWT audience is exactly `schoolorbit-school-app`.
- JWT token version is exactly `1`; tokens without the new required claims are rejected.
- The request tenant is resolved centrally from request headers and must exactly equal the JWT `tenant` claim before tenant data is read.
- The WebSocket URL carries only `semester_id`; user ID, display name, school key, and token are never accepted as authentication input.
- Timetable read or manage permission is required to connect; edit and drag events require manage permission.
- WebSocket text frames are limited to 64 KiB, Ping interval is 30 seconds, and inbound-silence timeout is 90 seconds.
- Frontend reconnect delays are 1, 2, 4, 8, 16 seconds and then capped at 30 seconds, with 80%-120% jitter.
- `national_id` remains AES-256-GCM encrypted with keyed HMAC-SHA256 blind indexes and is never logged in plaintext.
- No existing migration is modified and no database migration is added for this change.
- No Redis/NATS/PostgreSQL fan-out, refresh-token system, or general rate limiter is introduced.
- Authentication tokens, passwords, national IDs, encryption keys, database URLs, and malformed client payloads are never logged.

---

## File Structure

### New files

- `backend-school/src/modules/academic/services/timetable_realtime_service.rs` — owns timetable socket permission evaluation, minimal active-user lookup, display-name formatting, and semester existence validation.
- `frontend-school/src/lib/utils/timetable-reconnect.ts` — owns the pure reconnect delay calculation so timing policy is behavior-tested independently from browser state.
- `frontend-school/tests/static/timetable-realtime-security.test.mjs` — guards the browser URL contract, backoff policy, offline behavior, explicit teardown, and timetable-page call site.

### Modified backend files

- `backend-school/src/modules/auth/models.rs` — extends `Claims` with tenant and registered/version claims.
- `backend-school/src/utils/jwt.rs` — issues and strictly verifies version-1 school JWTs and exposes shared header authentication.
- `backend-school/src/modules/auth/handlers.rs` — passes the resolved tenant to JWT issuance and permission lookup.
- `backend-school/src/middleware/auth.rs` — delegates token extraction/verification to the shared helper.
- `backend-school/src/middleware/permission.rs` — accepts tenant explicitly while loading cached permissions.
- `backend-school/src/utils/request_context.rs` — resolves tenant first, authenticates against that tenant, and performs a claims defense-in-depth check.
- `backend-school/src/db/permission_cache.rs` — keys cache entries and invalidation by `(tenant, user_id)`.
- `backend-school/src/modules/staff/handlers/{organization_delegations,organization_members,organization_permissions,roles,staff,user_roles}.rs` — uses tenant-scoped cache invalidation and permission events.
- `backend-school/src/modules/notification/events.rs` — defines tenant-aware notification, permission, and work envelopes plus matching predicates.
- `backend-school/src/main.rs` — changes channel types and notification methods to require tenant identity.
- `backend-school/src/modules/notification/{handlers,services}.rs` and `backend-school/src/services/notification.rs` — propagate tenant through notification creation and filter SSE delivery.
- `backend-school/src/modules/calendar/{handlers,services}.rs` — propagates tenant into event notifications and every reminder path.
- `backend-school/src/modules/work/handlers.rs` and `backend-school/src/modules/workflow/handlers.rs` — emits tenant-scoped work events.
- `backend-school/src/modules/academic/services.rs` — registers the realtime access service.
- `backend-school/src/modules/academic/websockets.rs` — authenticates upgrades, derives room/presence identity, sanitizes events, limits frames, and manages heartbeat cleanup.
- `backend-school/tests/static_architecture.rs` — guards central auth, tenant-explicit invalidation/events, and server-owned WebSocket identity.

### Modified frontend and deployment files

- `frontend-school/src/lib/stores/timetable-socket.ts` — sends only the semester query value and implements bounded jittered reconnect/offline waiting.
- `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte` — passes `semester_id` and local-only `current_user_id`.
- `nginx-configs/school-api.schoolorbit.app.conf` — forwards WebSocket upgrade headers in a dedicated `/ws/` location.
- `docs/TESTING.md` — documents the secured timetable WebSocket sandbox checks and strict-token rollout behavior.

---

### Task 1: Strict tenant-bound JWT and shared request authentication

**Files:**
- Modify: `backend-school/src/modules/auth/models.rs:202-209`
- Modify: `backend-school/src/utils/jwt.rs:1-65`
- Modify: `backend-school/src/modules/auth/handlers.rs:22-78`
- Modify: `backend-school/src/middleware/auth.rs:1-100`
- Modify: `backend-school/src/utils/request_context.rs:1-90`
- Test: `backend-school/src/utils/jwt.rs`
- Test: `backend-school/src/utils/request_context.rs`

**Interfaces:**
- Produces: `AuthenticatedRequest { claims: Claims, user_id: Uuid, tenant: String }`.
- Produces: `JwtService::generate_token(user_id: &str, username: &str, user_type: &str, tenant: &str) -> Result<String, String>`.
- Produces: `JwtService::verify_token(token: &str) -> Result<Claims, String>`.
- Produces: `authenticate_request(headers: &HeaderMap) -> Result<AuthenticatedRequest, AppError>`.
- Produces: `authenticate_for_tenant(headers: &HeaderMap, expected_tenant: &str) -> Result<AuthenticatedRequest, AppError>`.
- Consumes: `extract_subdomain_from_request(headers)` from `backend-school/src/utils/subdomain.rs`.

- [ ] **Step 1: Add failing JWT contract and header-precedence tests**

Add the new claim fields and tests first. Use one process-wide test secret and generate claims through the public API:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, HeaderMap, HeaderValue};

    fn configure_secret() {
        std::env::set_var("JWT_SECRET", "test-only-secret-at-least-32-bytes");
    }

    #[test]
    fn generated_token_has_strict_tenant_contract() {
        configure_secret();
        let token = JwtService::generate_token(
            "8b391685-4a1c-4f25-a544-b1c5bd0d457e",
            "teacher.one",
            "staff",
            "tenant-a",
        ).unwrap();
        let claims = JwtService::verify_token(&token).unwrap();
        assert_eq!(claims.tenant, "tenant-a");
        assert_eq!(claims.iss, JWT_ISSUER);
        assert_eq!(claims.aud, JWT_AUDIENCE);
        assert_eq!(claims.token_version, TOKEN_VERSION);
    }

    #[test]
    fn bearer_token_takes_precedence_over_cookie() {
        configure_secret();
        let bearer = JwtService::generate_token(
            "8b391685-4a1c-4f25-a544-b1c5bd0d457e", "bearer", "staff", "tenant-a"
        ).unwrap();
        let cookie = JwtService::generate_token(
            "eb22ab8e-4382-4ddb-bcbb-8833b788e362", "cookie", "staff", "tenant-a"
        ).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {bearer}")).unwrap());
        headers.insert(header::COOKIE, HeaderValue::from_str(&format!("auth_token={cookie}")).unwrap());
        assert_eq!(extract_token_from_headers(&headers).unwrap(), bearer);
    }

    fn encode_claims(claims: Claims) -> String {
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"test-only-secret-at-least-32-bytes"),
        ).unwrap()
    }

    fn strict_claims() -> Claims {
        let now = Utc::now().timestamp();
        Claims {
            sub: "8b391685-4a1c-4f25-a544-b1c5bd0d457e".into(),
            username: "teacher.one".into(),
            user_type: "staff".into(),
            tenant: "tenant-a".into(),
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
            token_version: TOKEN_VERSION,
            exp: now + 300,
            iat: now,
        }
    }

    #[test]
    fn wrong_registered_claims_version_and_expiry_are_rejected() {
        configure_secret();
        let mut wrong_issuer = strict_claims();
        wrong_issuer.iss = "other-service".into();
        assert!(JwtService::verify_token(&encode_claims(wrong_issuer)).is_err());

        let mut wrong_audience = strict_claims();
        wrong_audience.aud = "other-app".into();
        assert!(JwtService::verify_token(&encode_claims(wrong_audience)).is_err());

        let mut wrong_version = strict_claims();
        wrong_version.token_version = 2;
        assert!(JwtService::verify_token(&encode_claims(wrong_version)).is_err());

        let mut expired = strict_claims();
        expired.exp = Utc::now().timestamp() - 1;
        assert!(JwtService::verify_token(&encode_claims(expired)).is_err());

        let mut missing_issuer = serde_json::to_value(strict_claims()).unwrap();
        missing_issuer.as_object_mut().unwrap().remove("iss");
        let token = encode(
            &Header::default(),
            &missing_issuer,
            &EncodingKey::from_secret(b"test-only-secret-at-least-32-bytes"),
        ).unwrap();
        assert!(JwtService::verify_token(&token).is_err());
    }

    #[test]
    fn request_tenant_must_match_token_tenant() {
        configure_secret();
        let token = encode_claims(strict_claims());
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {token}")).unwrap());
        headers.insert("x-school-subdomain", HeaderValue::from_static("tenant-b"));
        assert!(matches!(authenticate_request(&headers), Err(AppError::AuthError(_))));
    }
}
```

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run:

```bash
cd backend-school
cargo test --bin backend-school utils::jwt::tests -- --nocapture
```

Expected: compilation fails because the new `generate_token` argument and claim fields do not exist, and `extract_token_from_headers` is undefined.

- [ ] **Step 3: Implement strict claims and shared authentication**

Extend `Claims` exactly as follows:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub user_type: String,
    pub tenant: String,
    pub iss: String,
    pub aud: String,
    pub token_version: u8,
    pub exp: i64,
    pub iat: i64,
}
```

In `utils/jwt.rs`, add the strict constants/helper and use a `Validation` that requires registered claims:

```rust
pub const JWT_ISSUER: &str = "schoolorbit-backend-school";
pub const JWT_AUDIENCE: &str = "schoolorbit-school-app";
pub const TOKEN_VERSION: u8 = 1;

#[derive(Clone, Debug)]
pub struct AuthenticatedRequest {
    pub claims: Claims,
    pub user_id: Uuid,
    pub tenant: String,
}

pub fn extract_token_from_headers(headers: &HeaderMap) -> Result<String, AppError> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned);
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| JwtService::extract_token_from_cookie(Some(value)));
    bearer.or(cookie).ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))
}

pub fn authenticate_request(headers: &HeaderMap) -> Result<AuthenticatedRequest, AppError> {
    let token = extract_token_from_headers(headers)?;
    let claims = JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    let tenant = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    if claims.tenant != tenant {
        return Err(AppError::AuthError("Token ไม่ถูกต้อง".to_string()));
    }
    Ok(AuthenticatedRequest { claims, user_id, tenant })
}

pub fn authenticate_for_tenant(
    headers: &HeaderMap,
    expected_tenant: &str,
) -> Result<AuthenticatedRequest, AppError> {
    let authenticated = authenticate_request(headers)?;
    if authenticated.tenant != expected_tenant {
        return Err(AppError::AuthError("Token ไม่ถูกต้อง".to_string()));
    }
    Ok(authenticated)
}
```

Read configuration without panicking in both public JWT methods and keep the secret value out of the error:

```rust
let secret = env::var("JWT_SECRET")
    .map_err(|_| "JWT_SECRET environment variable must be set".to_string())?;
```

Use this exact generation/verification shape:

```rust
let claims = Claims {
    sub: user_id.to_string(),
    username: username.to_string(),
    user_type: user_type.to_string(),
    tenant: tenant.to_string(),
    iss: JWT_ISSUER.to_string(),
    aud: JWT_AUDIENCE.to_string(),
    token_version: TOKEN_VERSION,
    exp,
    iat: now,
};

let mut validation = Validation::default();
validation.leeway = 0;
validation.set_issuer(&[JWT_ISSUER]);
validation.set_audience(&[JWT_AUDIENCE]);
validation.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
let claims = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
    .map_err(|_| "Invalid token".to_string())?
    .claims;
if claims.token_version != TOKEN_VERSION {
    return Err("Invalid token version".to_string());
}
Ok(claims)
```

- [ ] **Step 4: Replace duplicate request parsing and bind login issuance**

In `auth_middleware`, call `authenticate_request(req.headers())`, insert `authenticated.claims`, and return the existing 401 JSON shape for `AppError::AuthError`. Preserve the existing async `extract_user_id` signature for current callers, but delegate it to the same helper:

```rust
pub async fn extract_user_id(
    headers: &HeaderMap,
    _pool: &sqlx::PgPool,
) -> Result<Uuid, String> {
    authenticate_request(headers)
        .map(|authenticated| authenticated.user_id)
        .map_err(|_| "Invalid user authentication".to_string())
}
```

In login, preserve the normalized subdomain before moving the pool and pass it to token issuance. Leave the cache call on its existing signature until Task 2:

```rust
let tenant = tenant_context(&state, &headers).await?;
let subdomain = tenant.subdomain;
let pool = tenant.pool;
let token = JwtService::generate_token(
    &user.id.to_string(),
    &user.username,
    &user.user_type,
    &subdomain,
)?;
let permissions = get_cached_user_permissions(user.id, &pool, &state.permission_cache).await?;
```

In `request_context.rs`, use the shared authentication in the headers path and repeat the claim comparison in the middleware-claims path:

```rust
pub async fn current_user_tenant_context_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<CurrentUserTenantContext, AppError> {
    let tenant = tenant_context(state, headers).await?;
    let authenticated = authenticate_for_tenant(headers, &tenant.subdomain)?;
    Ok(CurrentUserTenantContext { tenant, user_id: authenticated.user_id })
}

pub async fn current_user_tenant_context_from_claims(
    state: &AppState,
    headers: &HeaderMap,
    claims: &Claims,
) -> Result<CurrentUserTenantContext, AppError> {
    let tenant = tenant_context(state, headers).await?;
    if claims.tenant != tenant.subdomain {
        return Err(AppError::AuthError("Invalid user authentication".to_string()));
    }
    let user_id = user_id_from_claims(claims)?;
    Ok(CurrentUserTenantContext { tenant, user_id })
}
```

- [ ] **Step 5: Run JWT/request-context tests and backend check**

Run:

```bash
cd backend-school
cargo test --bin backend-school utils::jwt::tests -- --nocapture
cargo test --bin backend-school utils::request_context::tests -- --nocapture
cargo check --bin backend-school
```

Expected: focused tests and the complete backend check pass; no claim-construction or duplicate token-parser error remains.

- [ ] **Step 6: Commit the strict JWT boundary**

```bash
git add backend-school/src/modules/auth/models.rs backend-school/src/utils/jwt.rs backend-school/src/modules/auth/handlers.rs backend-school/src/middleware/auth.rs backend-school/src/utils/request_context.rs
git commit -m "fix(auth): bind school tokens to tenant"
```

---

### Task 2: Tenant-aware permission cache and invalidation

**Files:**
- Modify/Test: `backend-school/src/db/permission_cache.rs`
- Modify: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/src/utils/request_context.rs`
- Modify: `backend-school/src/modules/auth/handlers.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_delegations.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_members.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_permissions.rs`
- Modify: `backend-school/src/modules/staff/handlers/roles.rs`
- Modify: `backend-school/src/modules/staff/handlers/staff.rs`
- Modify: `backend-school/src/modules/staff/handlers/user_roles.rs`

**Interfaces:**
- Consumes: `authenticate_for_tenant(headers, tenant) -> Result<AuthenticatedRequest, AppError>` from Task 1.
- Produces: `TenantUserKey { tenant: String, user_id: Uuid }`.
- Produces: `PermissionCache::{get,set,invalidate_user,invalidate_tenant}` with tenant-explicit arguments.
- Produces: `get_cached_user_permissions(tenant: &str, user_id: Uuid, pool: &PgPool, cache: &PermissionCache) -> Result<Vec<String>, sqlx::Error>`.
- Produces: `load_actor_context(headers: &HeaderMap, tenant: &str, pool: &PgPool, cache: &PermissionCache) -> Result<ActorContext, AppError>`.

- [ ] **Step 1: Add failing cache-isolation tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_user_ids_are_isolated_by_tenant() {
        let cache = PermissionCache::new();
        let user_id = Uuid::new_v4();
        cache.set("tenant-a", user_id, vec!["a.read".into()]);
        cache.set("tenant-b", user_id, vec!["b.read".into()]);
        assert_eq!(cache.get("tenant-a", user_id), Some(vec!["a.read".into()]));
        assert_eq!(cache.get("tenant-b", user_id), Some(vec!["b.read".into()]));
    }

    #[test]
    fn invalidation_never_crosses_tenant_boundary() {
        let cache = PermissionCache::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        cache.set("tenant-a", first, vec!["a".into()]);
        cache.set("tenant-a", second, vec!["a".into()]);
        cache.set("tenant-b", first, vec!["b".into()]);
        cache.invalidate_user("tenant-a", first);
        assert!(cache.get("tenant-a", first).is_none());
        assert!(cache.get("tenant-b", first).is_some());
        cache.invalidate_tenant("tenant-a");
        assert!(cache.get("tenant-a", second).is_none());
        assert!(cache.get("tenant-b", first).is_some());
    }

    #[test]
    fn expired_entries_are_removed() {
        let cache = PermissionCache::new();
        let user_id = Uuid::new_v4();
        let key = TenantUserKey { tenant: "tenant-a".into(), user_id };
        cache.inner.insert(key, CacheEntry {
            permissions: vec!["a.read".into()],
            cached_at: Instant::now() - TTL - Duration::from_secs(1),
        });
        assert!(cache.get("tenant-a", user_id).is_none());
        assert!(cache.inner.is_empty());
    }
}
```

- [ ] **Step 2: Run cache tests and confirm they fail to compile**

Run: `cd backend-school && cargo test --bin backend-school db::permission_cache::tests -- --nocapture`

Expected: method-arity errors because the cache is still keyed only by UUID.

- [ ] **Step 3: Replace the key and public cache API**

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TenantUserKey {
    pub tenant: String,
    pub user_id: Uuid,
}

pub struct PermissionCache {
    inner: DashMap<TenantUserKey, CacheEntry>,
}

impl PermissionCache {
    fn key(tenant: &str, user_id: Uuid) -> TenantUserKey {
        TenantUserKey { tenant: tenant.to_string(), user_id }
    }

    pub fn get(&self, tenant: &str, user_id: Uuid) -> Option<Vec<String>> {
        let key = Self::key(tenant, user_id);
        let entry = self.inner.get(&key)?;
        if entry.cached_at.elapsed() > TTL {
            drop(entry);
            self.inner.remove(&key);
            return None;
        }
        Some(entry.permissions.clone())
    }

    pub fn set(&self, tenant: &str, user_id: Uuid, permissions: Vec<String>) {
        self.inner.insert(Self::key(tenant, user_id), CacheEntry { permissions, cached_at: Instant::now() });
    }

    pub fn invalidate_user(&self, tenant: &str, user_id: Uuid) {
        self.inner.remove(&Self::key(tenant, user_id));
    }

    pub fn invalidate_tenant(&self, tenant: &str) {
        self.inner.retain(|key, _| key.tenant != tenant);
    }
}
```

- [ ] **Step 4: Thread tenant through permission loading**

Use these exact signatures and calls in `middleware/permission.rs`:

```rust
pub async fn get_cached_user_permissions(
    tenant: &str,
    user_id: Uuid,
    pool: &PgPool,
    cache: &PermissionCache,
) -> Result<Vec<String>, sqlx::Error> {
    if let Some(permissions) = cache.get(tenant, user_id) {
        return Ok(permissions);
    }
    let permissions = fetch_user_permissions(user_id, pool).await?;
    cache.set(tenant, user_id, permissions.clone());
    Ok(permissions)
}

pub async fn load_actor_context(
    headers: &HeaderMap,
    tenant: &str,
    pool: &PgPool,
    cache: &PermissionCache,
) -> Result<ActorContext, AppError> {
    let user_id = authenticate_for_tenant(headers, tenant)?.user_id;
    let permissions = get_cached_user_permissions(tenant, user_id, pool, cache).await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์ได้".to_string()))?;
    Ok(ActorContext { user_id, permissions })
}

pub async fn load_actor_context_or_error(
    headers: &HeaderMap,
    tenant: &str,
    pool: &PgPool,
    cache: &PermissionCache,
) -> Result<ActorContext, AppError> {
    load_actor_context(headers, tenant, pool, cache).await
}
```

Update `actor_tenant_context` to pass `&tenant.subdomain` before `&tenant.pool`.

Update both permission reads in `modules/auth/handlers.rs` to retain and pass the tenant string:

```rust
let subdomain = context.tenant.subdomain.clone();
let pool = context.tenant.pool;
let permissions = get_cached_user_permissions(
    &subdomain,
    user.id,
    &pool,
    &state.permission_cache,
).await?;
```

- [ ] **Step 5: Replace every mutation invalidation with tenant scope**

At user-specific mutation sites, capture `let tenant = context.tenant.subdomain.clone();` and use:

```rust
state.permission_cache.invalidate_user(&tenant, affected_user_id);
state.notify_permission_changed(affected_user_id);
```

At role and organization-grant mutation sites, use:

```rust
state.permission_cache.invalidate_tenant(&tenant);
state.notify_all_permissions_changed();
```

Apply the user-specific form in `organization_delegations.rs`, `organization_members.rs`, `staff.rs`, and `user_roles.rs`. Apply the tenant-wide form in `roles.rs` and `organization_permissions.rs`. Notification methods stay on their current signatures until Task 3 changes the event envelopes. Do not retain `invalidate(...)` or `clear_all()` calls.

- [ ] **Step 6: Verify cache behavior and all call sites**

Run:

```bash
cd backend-school
cargo test --bin backend-school db::permission_cache::tests middleware::permission::tests -- --nocapture
cargo check --bin backend-school
rg -n 'permission_cache\.(invalidate\(|clear_all\()' src
```

Expected: tests and check pass; the final search returns no matches.

- [ ] **Step 7: Commit the tenant-aware cache**

```bash
git add backend-school/src/db/permission_cache.rs backend-school/src/middleware/permission.rs backend-school/src/utils/request_context.rs backend-school/src/modules/auth/handlers.rs backend-school/src/modules/staff/handlers
git commit -m "fix(auth): isolate permission cache by tenant"
```

---

### Task 3: Tenant-aware notification, permission, and work event envelopes

**Files:**
- Modify/Test: `backend-school/src/modules/notification/events.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/modules/notification/handlers.rs`
- Modify: `backend-school/src/modules/notification/services.rs`
- Modify: `backend-school/src/services/notification.rs`
- Modify: `backend-school/src/modules/calendar/handlers.rs`
- Modify: `backend-school/src/modules/calendar/services.rs`
- Modify: `backend-school/src/modules/work/handlers.rs`
- Modify: `backend-school/src/modules/workflow/handlers.rs`
- Modify: staff handler files listed in Task 2

**Interfaces:**
- Produces: `TenantNotificationEvent { tenant: String, user_id: Uuid, notification: Notification }`.
- Produces: `PermissionChangeEvent::for_user(tenant: &str, user_id: Uuid)` and `for_all_users(tenant: &str)`.
- Produces: `PermissionChangeEvent::applies_to(tenant: &str, user_id: Uuid) -> bool`.
- Produces: tenant-bearing `WorkChangeEvent::{work_items_changed,workflow_window_changed}` and `applies_to(tenant: &str)`.
- Produces: `NotificationService::send(pool, notification_tx, tenant, user_id, ...)`.

- [ ] **Step 1: Add failing envelope-matching tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_notification() -> Notification {
        Notification {
            id: Uuid::nil(),
            title: "Test".into(),
            message: "Message".into(),
            type_: "info".into(),
            link: None,
            read_at: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn permission_events_match_tenant_and_optional_user() {
        let user = Uuid::new_v4();
        assert!(PermissionChangeEvent::for_user("tenant-a", user).applies_to("tenant-a", user));
        assert!(!PermissionChangeEvent::for_user("tenant-a", user).applies_to("tenant-b", user));
        assert!(PermissionChangeEvent::for_all_users("tenant-a").applies_to("tenant-a", Uuid::new_v4()));
        assert!(!PermissionChangeEvent::for_all_users("tenant-a").applies_to("tenant-b", user));
    }

    #[test]
    fn work_events_match_only_their_tenant() {
        let event = WorkChangeEvent::work_items_changed("tenant-a");
        assert!(event.applies_to("tenant-a"));
        assert!(!event.applies_to("tenant-b"));
    }

    #[test]
    fn notification_events_match_tenant_and_user() {
        let user = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let notification = test_notification();
        let event = TenantNotificationEvent::new("tenant-a", user, notification);
        assert!(event.applies_to("tenant-a", user));
        assert!(!event.applies_to("tenant-b", user));
        assert!(!event.applies_to("tenant-a", other_user));
    }
}
```

- [ ] **Step 2: Run the tests and confirm the tenant API is absent**

Run: `cd backend-school && cargo test --bin backend-school modules::notification::events::tests -- --nocapture`

Expected: constructor/method-arity failures.

- [ ] **Step 3: Implement explicit tenant envelopes**

```rust
use super::models::Notification;

#[derive(Debug, Clone)]
pub struct TenantNotificationEvent {
    pub tenant: String,
    pub user_id: Uuid,
    pub notification: Notification,
}

impl TenantNotificationEvent {
    pub fn new(tenant: &str, user_id: Uuid, notification: Notification) -> Self {
        Self { tenant: tenant.to_string(), user_id, notification }
    }
    pub fn applies_to(&self, tenant: &str, user_id: Uuid) -> bool {
        self.tenant == tenant && self.user_id == user_id
    }
}

#[derive(Debug, Clone)]
pub struct PermissionChangeEvent {
    pub tenant: String,
    pub target_user_id: Option<Uuid>,
}

impl PermissionChangeEvent {
    pub fn for_user(tenant: &str, user_id: Uuid) -> Self {
        Self { tenant: tenant.to_string(), target_user_id: Some(user_id) }
    }
    pub fn for_all_users(tenant: &str) -> Self {
        Self { tenant: tenant.to_string(), target_user_id: None }
    }
    pub fn applies_to(&self, tenant: &str, user_id: Uuid) -> bool {
        self.tenant == tenant
            && self.target_user_id.map(|target| target == user_id).unwrap_or(true)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkChangeEvent {
    pub tenant: String,
    pub kind: WorkChangeKind,
}

impl WorkChangeEvent {
    pub fn work_items_changed(tenant: &str) -> Self {
        Self { tenant: tenant.to_string(), kind: WorkChangeKind::WorkItemsChanged }
    }
    pub fn workflow_window_changed(tenant: &str) -> Self {
        Self { tenant: tenant.to_string(), kind: WorkChangeKind::WorkflowWindowChanged }
    }
    pub fn applies_to(&self, tenant: &str) -> bool { self.tenant == tenant }
}
```

- [ ] **Step 4: Change AppState channels and emitters**

```rust
pub notification_channel: broadcast::Sender<TenantNotificationEvent>,

pub fn notify_permission_changed(&self, tenant: &str, target_user_id: Uuid) {
    let _ = self.permission_event_channel.send(PermissionChangeEvent::for_user(tenant, target_user_id));
}
pub fn notify_all_permissions_changed(&self, tenant: &str) {
    let _ = self.permission_event_channel.send(PermissionChangeEvent::for_all_users(tenant));
}
pub fn notify_work_items_changed(&self, tenant: &str) {
    let _ = self.work_event_channel.send(WorkChangeEvent::work_items_changed(tenant));
}
pub fn notify_workflow_window_changed(&self, tenant: &str) {
    let _ = self.work_event_channel.send(WorkChangeEvent::workflow_window_changed(tenant));
}
```

- [ ] **Step 5: Propagate tenant through notification and calendar services**

Change every notification-channel type to `broadcast::Sender<TenantNotificationEvent>`, add `tenant: &str` before `user_id`, and broadcast:

```rust
let _ = notification_tx.send(TenantNotificationEvent::new(tenant, user_id, notification));
```

For calendar HTTP handlers pass `&context.tenant.subdomain`. For the all-tenant reminder loop pass the school identity into each tenant run:

```rust
process_due_reminders(
    &pool,
    &notification_channel,
    &school.subdomain,
    tenant_current_date,
).await
```

Continue passing that same `tenant` argument through `process_due_reminder_candidate`, `process_advisory_locked_reminder`, `send_event_notification`, and `NotificationService::send`.

- [ ] **Step 6: Filter SSE and work emitters by tenant**

Capture `let tenant = context.tenant.subdomain;` before building the SSE stream and use:

```rust
Ok(event) if event.applies_to(&tenant, user_id) => { /* serialize event.notification */ }
Ok(event) if event.applies_to(&tenant, user_id) => {
    yield Ok(Event::default().event("permission_changed").data("{}"));
}
Ok(event) if event.applies_to(&tenant) => {
    yield Ok(Event::default().event(event.event_name()).data("{}"));
}
Ok(_) => {}
```

For `broadcast::error::RecvError::Lagged(_)` on all three channel branches, emit nothing because the skipped event may belong to another tenant. The SSE reconnect/fetch path remains the recovery mechanism and no tenant receives a refresh signal caused only by another tenant's event volume.

In work/workflow handlers call `notify_work_items_changed(&context.tenant.subdomain)` and `notify_workflow_window_changed(&context.tenant.subdomain)`.

At every staff invalidation site changed in Task 2, pass the same captured tenant to the matching event method:

```rust
state.notify_permission_changed(&tenant, affected_user_id);
state.notify_all_permissions_changed(&tenant);
```

Use only the first line after `invalidate_user`; use only the second line after `invalidate_tenant`.

- [ ] **Step 7: Verify event isolation and compile all producers**

Run:

```bash
cd backend-school
cargo test --bin backend-school modules::notification::events::tests -- --nocapture
cargo check --bin backend-school
rg -n 'Sender<\(Uuid, Notification\)>|notify_(permission_changed|all_permissions_changed|work_items_changed|workflow_window_changed)\(\)' src
```

Expected: tests/check pass and the search returns no matches.

- [ ] **Step 8: Commit tenant-scoped in-process events**

```bash
git add backend-school/src/main.rs backend-school/src/modules/notification backend-school/src/services/notification.rs backend-school/src/modules/calendar backend-school/src/modules/work/handlers.rs backend-school/src/modules/workflow/handlers.rs backend-school/src/modules/staff/handlers
git commit -m "fix(realtime): scope process events by tenant"
```

---

### Task 4: Timetable realtime access service

**Files:**
- Create/Test: `backend-school/src/modules/academic/services/timetable_realtime_service.rs`
- Modify: `backend-school/src/modules/academic/services.rs`

**Interfaces:**
- Consumes: `ActorContext::{has_permission,has_any_permission}`.
- Produces: `TimetableSocketAccess { user_id: Uuid, display_name: String, can_manage: bool }`.
- Produces: `authorize_socket(pool: &PgPool, actor: &ActorContext, semester_id: Uuid) -> Result<TimetableSocketAccess, AppError>`.
- Produces: private `socket_permission(actor: &ActorContext) -> Result<bool, AppError>` where the boolean is manage capability.

- [ ] **Step 1: Create the service with failing pure permission/name tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext { user_id: Uuid::new_v4(), permissions: permissions.iter().map(|p| p.to_string()).collect() }
    }

    #[test]
    fn reader_connects_without_manage_capability() {
        assert!(!socket_permission(&actor(&[codes::ACADEMIC_COURSE_PLAN_READ_ALL])).unwrap());
    }

    #[test]
    fn manager_and_wildcard_can_manage() {
        assert!(socket_permission(&actor(&[codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL])).unwrap());
        assert!(socket_permission(&actor(&[codes::WILDCARD])).unwrap());
    }

    #[test]
    fn unrelated_permission_is_forbidden() {
        assert!(matches!(socket_permission(&actor(&["calendar.read.all"])), Err(AppError::Forbidden(_))));
    }

    #[test]
    fn display_name_prefers_person_name_and_falls_back_to_username() {
        assert_eq!(display_name(Some("นาย"), "สมชาย", "ใจดี", "staff1"), "นายสมชาย ใจดี");
        assert_eq!(display_name(None, "", "", "staff1"), "staff1");
    }
}
```

- [ ] **Step 2: Run focused tests and confirm the missing service functions**

Run: `cd backend-school && cargo test --bin backend-school timetable_realtime_service -- --nocapture`

Expected: compilation fails until the module is registered and the helpers exist.

- [ ] **Step 3: Implement permission policy and minimal queries**

```rust
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TimetableSocketAccess {
    pub user_id: Uuid,
    pub display_name: String,
    pub can_manage: bool,
}

#[derive(sqlx::FromRow)]
struct RealtimeUser {
    username: String,
    title: Option<String>,
    first_name: String,
    last_name: String,
}

fn socket_permission(actor: &ActorContext) -> Result<bool, AppError> {
    let can_manage = actor.has_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL);
    if can_manage || actor.has_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL) {
        Ok(can_manage)
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์ดูตารางเรียน".to_string()))
    }
}

fn display_name(title: Option<&str>, first_name: &str, last_name: &str, username: &str) -> String {
    let given_name = format!("{}{}", title.unwrap_or_default().trim(), first_name.trim());
    let full_name = [given_name.as_str(), last_name.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if full_name.is_empty() {
        username.trim().to_string()
    } else {
        full_name
    }
}

pub async fn authorize_socket(
    pool: &PgPool,
    actor: &ActorContext,
    semester_id: Uuid,
) -> Result<TimetableSocketAccess, AppError> {
    let can_manage = socket_permission(actor)?;
    let user = sqlx::query_as::<_, RealtimeUser>(
        "SELECT username, title, first_name, last_name FROM users WHERE id = $1 AND status = 'active'"
    ).bind(actor.user_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::AuthError("ไม่พบผู้ใช้งานที่เปิดใช้งาน".to_string()))?;
    let semester_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM academic_semesters WHERE id = $1)"
    ).bind(semester_id).fetch_one(pool).await?;
    if !semester_exists {
        return Err(AppError::NotFound("ไม่พบภาคเรียน".to_string()));
    }
    Ok(TimetableSocketAccess {
        user_id: actor.user_id,
        display_name: display_name(user.title.as_deref(), &user.first_name, &user.last_name, &user.username),
        can_manage,
    })
}
```

- [ ] **Step 4: Register and verify the service**

Add `pub mod timetable_realtime_service;` to `academic/services.rs`, then run:

```bash
cd backend-school
cargo fmt --all
cargo test --bin backend-school timetable_realtime_service -- --nocapture
cargo check --bin backend-school
```

Expected: four focused tests pass and the binary checks without selecting protected user fields.

- [ ] **Step 5: Commit the access service**

```bash
git add backend-school/src/modules/academic/services.rs backend-school/src/modules/academic/services/timetable_realtime_service.rs
git commit -m "feat(realtime): add timetable socket access policy"
```

---

### Task 5: Authenticated WebSocket upgrade, event sanitizer, and heartbeat

**Files:**
- Modify/Test: `backend-school/src/modules/academic/websockets.rs`
- Modify: `backend-school/src/main.rs` only if router return-type inference requires it

**Interfaces:**
- Consumes: `actor_tenant_context(state, headers) -> ActorTenantContext` from Tasks 1-2.
- Consumes: `authorize_socket(pool, actor, semester_id) -> TimetableSocketAccess` from Task 4.
- Produces: `WsParams { semester_id: Uuid }`.
- Produces: `sanitize_client_event(event: TimetableEvent, authenticated_user_id: Uuid, can_manage: bool) -> Option<TimetableEvent>`.
- Produces: `handle_socket(socket, state, semester_id, tenant, access)` with server-owned identity.

- [ ] **Step 1: Add failing query and event-policy tests**

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn legacy_query_identity_is_ignored() {
        let params: WsParams = serde_json::from_value(serde_json::json!({
            "semester_id": "8b391685-4a1c-4f25-a544-b1c5bd0d457e",
            "user_id": "eb22ab8e-4382-4ddb-bcbb-8833b788e362",
            "name": "attacker",
            "school_key": "other"
        })).unwrap();
        assert_eq!(params.semester_id.to_string(), "8b391685-4a1c-4f25-a544-b1c5bd0d457e");
    }

    #[test]
    fn reader_can_move_cursor_but_cannot_relay_edit_intent() {
        let actor = Uuid::new_v4();
        let forged = Uuid::new_v4();
        let cursor = TimetableEvent::CursorMove { user_id: forged, x: 1.0, y: 2.0, context: None };
        assert!(matches!(sanitize_client_event(cursor, actor, false), Some(TimetableEvent::CursorMove { user_id, .. }) if user_id == actor));
        let refresh = TimetableEvent::TableRefresh { user_id: forged };
        assert!(sanitize_client_event(refresh, actor, false).is_none());
    }

    #[test]
    fn manager_identity_replaces_forged_payload_identity() {
        let actor = Uuid::new_v4();
        let drag = TimetableEvent::DragEnd { user_id: Uuid::new_v4() };
        assert!(matches!(sanitize_client_event(drag, actor, true), Some(TimetableEvent::DragEnd { user_id }) if user_id == actor));
    }

    #[test]
    fn server_only_events_are_never_accepted_from_clients() {
        let event = TimetableEvent::UserLeft { user_id: Uuid::new_v4() };
        assert!(sanitize_client_event(event, Uuid::new_v4(), true).is_none());
    }

    #[test]
    fn room_key_uses_server_tenant() {
        let semester = Uuid::new_v4();
        assert_eq!(WebSocketManager::get_room_key("tenant-a".to_string(), semester), format!("tenant-a:{semester}"));
    }

    #[test]
    fn frame_limit_and_heartbeat_deadline_are_exact() {
        assert!(!text_frame_too_large(64 * 1024));
        assert!(text_frame_too_large(64 * 1024 + 1));
        let last = Instant::now();
        assert!(!heartbeat_timed_out(last, last + Duration::from_secs(89)));
        assert!(heartbeat_timed_out(last, last + Duration::from_secs(90)));
    }

    #[test]
    fn multi_tab_presence_joins_and_leaves_once() {
        let manager = WebSocketManager::new();
        let semester = Uuid::new_v4();
        let tenant = "tenant-a".to_string();
        let user_id = Uuid::new_v4();
        manager.get_or_create_room(tenant.clone(), semester);
        let presence = UserPresence {
            user_id,
            name: "Teacher".into(),
            color: "#112233".into(),
            context: None,
        };
        assert!(manager.join_room(tenant.clone(), semester, presence.clone()));
        assert!(!manager.join_room(tenant.clone(), semester, presence));
        assert!(!manager.leave_room(tenant.clone(), semester, user_id));
        assert!(manager.leave_room(tenant, semester, user_id));
    }
}
```

- [ ] **Step 2: Run policy tests and confirm the old query contract fails**

Run: `cd backend-school && cargo test --bin backend-school academic::websockets::security_tests -- --nocapture`

Expected: missing sanitizer/type mismatches until `WsParams` is reduced and policy is added.

- [ ] **Step 3: Reduce query data and authenticate before upgrade**

```rust
#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub semester_id: Uuid,
}

pub async fn timetable_websocket_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WsParams>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let access = authorize_socket(&context.tenant.pool, &context.actor, params.semester_id).await?;
    let tenant = context.tenant.subdomain;
    Ok(ws.on_upgrade(move |socket| {
        handle_socket(socket, state, params.semester_id, tenant, access)
    }))
}
```

Do not read `Host`, `school_key`, `name`, or `user_id` inside the handler. Serde's default unknown-field behavior intentionally accepts old frontend query fields during rolling deployment.

- [ ] **Step 4: Extract the pure client-event sanitizer**

Move the current event reconstruction match into:

```rust
fn sanitize_client_event(
    event: TimetableEvent,
    authenticated_user_id: Uuid,
    can_manage: bool,
) -> Option<TimetableEvent> {
    match event {
        TimetableEvent::CursorMove { x, y, context, .. } => Some(TimetableEvent::CursorMove {
            user_id: authenticated_user_id, x, y, context,
        }),
        TimetableEvent::DragStart { course_id, entry_id, info, .. } if can_manage => Some(TimetableEvent::DragStart {
            user_id: authenticated_user_id, course_id, entry_id, info,
        }),
        TimetableEvent::DragEnd { .. } if can_manage => Some(TimetableEvent::DragEnd { user_id: authenticated_user_id }),
        TimetableEvent::DragMove { x, y, target_day, target_period_id, .. } if can_manage => Some(TimetableEvent::DragMove {
            user_id: authenticated_user_id, x, y, target_day, target_period_id,
        }),
        TimetableEvent::UserActivity { activity_type, target, .. } if can_manage => Some(TimetableEvent::UserActivity {
            user_id: authenticated_user_id, activity_type, target,
        }),
        TimetableEvent::UserActivityEnd { .. } if can_manage => Some(TimetableEvent::UserActivityEnd { user_id: authenticated_user_id }),
        TimetableEvent::TableRefresh { .. } if can_manage => Some(TimetableEvent::TableRefresh { user_id: authenticated_user_id }),
        TimetableEvent::DropIntent { kind, entry_id, day_of_week, period_id, room_id, swap_partner_id, swap_partner_day, swap_partner_period_id, new_classroom_course_id, new_activity_slot_id, new_classroom_id, .. } if can_manage => Some(TimetableEvent::DropIntent {
            user_id: authenticated_user_id, kind, entry_id, day_of_week, period_id, room_id,
            swap_partner_id, swap_partner_day, swap_partner_period_id,
            new_classroom_course_id, new_activity_slot_id, new_classroom_id,
        }),
        TimetableEvent::EntryIntent { temp_id, classroom_id, classroom_course_id, activity_slot_id, day_of_week, period_id, room_id, title, entry_type, .. } if can_manage => Some(TimetableEvent::EntryIntent {
            user_id: authenticated_user_id, temp_id, classroom_id, classroom_course_id,
            activity_slot_id, day_of_week, period_id, room_id, title, entry_type,
        }),
        _ => None,
    }
}
```

After sanitizing, keep the existing in-memory context/drag/activity updates but take user ID from `access.user_id`; never from the parsed event.

- [ ] **Step 5: Consolidate socket lifecycle into one heartbeat-aware loop**

Use server-derived values for presence and room keys:

```rust
let user_id = access.user_id;
let mut user_presence = UserPresence {
    user_id,
    name: access.display_name,
    color: generate_color_from_uuid(&user_id),
    context: None,
};
let tx = state.websocket_manager.get_or_create_room(tenant.clone(), semester_id);
```

Replace the detached send task plus receiver loop with one `tokio::select!` loop so all exits share cleanup:

```rust
const MAX_TEXT_FRAME_BYTES: usize = 64 * 1024;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const SILENCE_TIMEOUT: Duration = Duration::from_secs(90);

fn text_frame_too_large(bytes: usize) -> bool {
    bytes > MAX_TEXT_FRAME_BYTES
}

fn heartbeat_timed_out(last_inbound: Instant, now: Instant) -> bool {
    now.duration_since(last_inbound) >= SILENCE_TIMEOUT
}

let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
let mut last_inbound = Instant::now();

loop {
    tokio::select! {
        incoming = socket.next() => match incoming {
            Some(Ok(Message::Text(text))) => {
                last_inbound = Instant::now();
                if text_frame_too_large(text.len()) {
                    let _ = socket.send(Message::Close(Some(CloseFrame { code: 1009, reason: "Message too large".into() }))).await;
                    break;
                }
                if let Ok(event) = serde_json::from_str::<TimetableEvent>(&text) {
                    if let Some(event) = sanitize_client_event(event, user_id, access.can_manage) {
                        relay_client_event(&state.websocket_manager, &tx, &tenant, semester_id, &mut user_presence, event);
                    }
                }
            }
            Some(Ok(Message::Pong(_))) => last_inbound = Instant::now(),
            Some(Ok(Message::Ping(payload))) => {
                last_inbound = Instant::now();
                if socket.send(Message::Pong(payload)).await.is_err() { break; }
            }
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
            Some(Ok(_)) => last_inbound = Instant::now(),
        },
        broadcast = rx.recv() => {
            if send_broadcast_event(&mut socket, broadcast).await.is_err() { break; }
        },
        _ = heartbeat.tick() => {
            if heartbeat_timed_out(last_inbound, Instant::now()) { break; }
            if socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
        }
    }
}
```

`send_broadcast_event` must preserve the existing lag behavior by sending `TableRefresh { user_id: Uuid::nil() }`. `relay_client_event` must preserve context/drag/activity state updates and mutation buffering. It must be synchronous and use the already-sanitized event.

- [ ] **Step 6: Preserve one cleanup path and multi-tab semantics**

After the loop, call exactly one leave path:

```rust
let is_last_tab = state.websocket_manager.leave_room(tenant.clone(), semester_id, user_id);
if is_last_tab {
    let _ = tx.send(SeqEvent {
        seq: None,
        event: TimetableEvent::UserLeft { user_id },
    });
}
```

No spawned socket sender task remains to abort. A timeout, close, parse-safe ignore, receiver closure, and send failure all reach this cleanup.

- [ ] **Step 7: Run WebSocket policy tests and backend verification**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --bin backend-school academic::websockets::security_tests -- --nocapture
cargo check --bin backend-school
cargo clippy --bin backend-school -- -D warnings
```

Expected: five policy tests pass; check/clippy pass; no client query value controls tenant/user/name.

- [ ] **Step 8: Commit secured timetable WebSocket handling**

```bash
git add backend-school/src/modules/academic/websockets.rs backend-school/src/main.rs
git commit -m "fix(realtime): authenticate timetable websocket clients"
```

---

### Task 6: Frontend URL contract and resilient reconnect

**Files:**
- Create/Test: `frontend-school/src/lib/utils/timetable-reconnect.ts`
- Create/Test: `frontend-school/tests/static/timetable-realtime-security.test.mjs`
- Modify: `frontend-school/src/lib/stores/timetable-socket.ts:463-595`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte:3103-3115`

**Interfaces:**
- Produces: `connectTimetableSocket({ semester_id: string, current_user_id: string }): void`.
- Produces: `reconnectDelayMs(attempt: number, random?: () => number): number`.
- Consumes: existing `BACKEND_WS_URL` and server-authoritative `TimetableEvent` payloads.

- [ ] **Step 1: Add failing static contract tests**

```javascript
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { reconnectDelayMs } from '../../src/lib/utils/timetable-reconnect.ts';

const store = readFileSync(new URL('../../src/lib/stores/timetable-socket.ts', import.meta.url), 'utf8');
const page = readFileSync(new URL('../../src/routes/(app)/staff/academic/timetable/+page.svelte', import.meta.url), 'utf8');
const connectionContract = store.slice(
  store.indexOf('type TimetableSocketParams'),
  store.indexOf('export function disconnectTimetableSocket')
);

test('timetable socket URL contains only semester identity', () => {
  assert.match(connectionContract, /new URLSearchParams\(\{\s*semester_id:\s*String\(params\.semester_id\)\s*\}\)/s);
  assert.doesNotMatch(connectionContract, /school_key\s*:/);
  assert.doesNotMatch(connectionContract, /name:\s*string/);
  assert.doesNotMatch(connectionContract, /user_id:\s*String\(params\.user_id\)/);
});

test('reconnect is exponential, capped, jittered, and offline aware', () => {
  assert.deepEqual([0, 1, 2, 3, 4].map((attempt) => reconnectDelayMs(attempt, () => 0.5)), [1000, 2000, 4000, 8000, 16000]);
  assert.equal(reconnectDelayMs(8, () => 0.5), 30000);
  assert.equal(reconnectDelayMs(0, () => 0), 800);
  assert.equal(reconnectDelayMs(0, () => 1), 1200);
  assert.match(store, /window\.addEventListener\('online'/);
  assert.match(store, /window\.removeEventListener\('online'/);
  assert.match(store, /reconnectAttempt\s*=\s*0/);
});

test('page passes server query and local-only current user identity', () => {
  assert.match(page, /connectTimetableSocket\(\{\s*semester_id:[\s\S]*current_user_id:/);
  assert.doesNotMatch(page, /connectTimetableSocket\(\{[\s\S]{0,180}name:/);
});
```

- [ ] **Step 2: Run the new static test and confirm it fails**

Run: `cd frontend-school && node --test tests/static/timetable-realtime-security.test.mjs`

Expected: all three tests fail against the legacy identity URL and fixed 3-second reconnect.

- [ ] **Step 3: Change the public connection contract and URL construction**

```typescript
type TimetableSocketParams = {
	semester_id: string;
	current_user_id: string;
};

let lastParams: TimetableSocketParams | null = null;

export function connectTimetableSocket(params: TimetableSocketParams) {
	currentUserId = params.current_user_id;
	const qs = new URLSearchParams({
		semester_id: String(params.semester_id)
	}).toString();
	const url = `${BACKEND_WS_URL}/ws/timetable?${qs}`;
	socket = new WebSocket(url);
}
```

Remove URL logging because rolling clients may still have legacy query fields in production logs. Keep local `currentUserId` only for ignoring reflected events and constructing payloads that the server overwrites.

- [ ] **Step 4: Implement jittered exponential reconnect and offline waiting**

Create the pure helper in `src/lib/utils/timetable-reconnect.ts`:

```typescript
export function reconnectDelayMs(attempt: number, random: () => number = Math.random): number {
	const base = Math.min(30_000, 1_000 * 2 ** attempt);
	return Math.round(base * (0.8 + random() * 0.4));
}
```

Import it into the store, then implement browser lifecycle state there:

```typescript
let reconnectAttempt = 0;
let waitingForOnline = false;

function clearOnlineListener() {
	if (waitingForOnline && typeof window !== 'undefined') {
		window.removeEventListener('online', handleOnline);
		waitingForOnline = false;
	}
}

function handleOnline() {
	clearOnlineListener();
	if (shouldReconnect && lastParams) connectTimetableSocket(lastParams);
}

function scheduleReconnect() {
	if (!shouldReconnect || !lastParams || typeof window === 'undefined') return;
	if (!navigator.onLine) {
		if (!waitingForOnline) {
			waitingForOnline = true;
			window.addEventListener('online', handleOnline, { once: true });
		}
		return;
	}
	const delay = reconnectDelayMs(reconnectAttempt);
	reconnectAttempt += 1;
	reconnectTimer = setTimeout(() => {
		if (shouldReconnect && lastParams) connectTimetableSocket(lastParams);
	}, delay);
}
```

Set `reconnectAttempt = 0` in `socket.onopen`. Call `scheduleReconnect()` in `socket.onclose`. In `disconnectTimetableSocket`, set `shouldReconnect = false`, clear both timers, call `clearOnlineListener()`, clear stores, set `isConnected` false, and close/null the socket.

- [ ] **Step 5: Update the Svelte page call site**

```svelte
connectTimetableSocket({
	semester_id: selectedSemesterId,
	current_user_id: user.id
});
```

Keep the existing `$effect` condition (`canReadTimetable`, selected semester, and authenticated user) and retain the existing `onDestroy(() => disconnectTimetableSocket())` teardown.

- [ ] **Step 6: Run Svelte tooling and frontend tests**

Run the Svelte skill analyzer/autofixer against the modified page and then:

```bash
cd frontend-school
node --test tests/static/timetable-realtime-security.test.mjs
npm run check
npm run test:static
npx eslint src/lib/stores/timetable-socket.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte'
npx prettier --check src/lib/utils/timetable-reconnect.ts src/lib/stores/timetable-socket.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte' tests/static/timetable-realtime-security.test.mjs
```

Expected: new three-test file passes, all static tests pass, and Svelte/ESLint/Prettier report no errors.

- [ ] **Step 7: Commit the browser contract**

```bash
git add frontend-school/src/lib/utils/timetable-reconnect.ts frontend-school/src/lib/stores/timetable-socket.ts 'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' frontend-school/tests/static/timetable-realtime-security.test.mjs
git commit -m "fix(frontend): secure timetable socket reconnect"
```

---

### Task 7: Nginx forwarding, architecture guards, and rollout documentation

**Files:**
- Modify/Test: `backend-school/tests/static_architecture.rs:2070-2200`
- Modify: `nginx-configs/school-api.schoolorbit.app.conf`
- Modify: `docs/TESTING.md`

**Interfaces:**
- Consumes: final backend/frontend contracts from Tasks 1-6.
- Produces: static guard tests that fail if tenant/user query trust or tenantless cache/event APIs return.
- Produces: `/ws/` reverse proxy with Upgrade forwarding and heartbeat-safe timeout.

- [ ] **Step 1: Add failing backend architecture guards**

```rust
#[test]
fn timetable_websocket_identity_is_server_owned() {
    let source = read_source(manifest_dir().join("src/modules/academic/websockets.rs"));
    let params_start = source.find("pub struct WsParams").unwrap();
    let params_end = source[params_start..].find("// ==========================================\n// State Manager").unwrap();
    let params = &source[params_start..params_start + params_end];
    assert!(params.contains("pub semester_id: Uuid"));
    assert!(!params.contains("user_id"));
    assert!(!params.contains("name:"));
    assert!(!params.contains("school_key"));
    assert!(source.contains("actor_tenant_context(&state, &headers)"));
    assert!(source.contains("authorize_socket("));
    assert!(source.contains("sanitize_client_event("));
}

#[test]
fn permission_cache_and_process_events_are_tenant_explicit() {
    let cache = read_source(manifest_dir().join("src/db/permission_cache.rs"));
    let events = read_source(manifest_dir().join("src/modules/notification/events.rs"));
    let main = read_source(manifest_dir().join("src/main.rs"));
    assert!(cache.contains("TenantUserKey"));
    assert!(Regex::new(r"invalidate_user\s*\(\s*&self,\s*tenant:\s*&str").unwrap().is_match(&cache));
    assert!(Regex::new(r"invalidate_tenant\s*\(\s*&self,\s*tenant:\s*&str").unwrap().is_match(&cache));
    assert!(!cache.contains("clear_all"));
    assert!(events.contains("pub tenant: String"));
    assert!(Regex::new(r"notify_permission_changed\s*\(\s*&self,\s*tenant:\s*&str").unwrap().is_match(&main));
    assert!(Regex::new(r"notify_work_items_changed\s*\(\s*&self,\s*tenant:\s*&str").unwrap().is_match(&main));
}

#[test]
fn feature_modules_do_not_parse_jwt_directly() {
    for path in list_files(manifest_dir().join("src/modules"), |path| path.extension().and_then(|ext| ext.to_str()) == Some("rs")) {
        let source = read_source(&path);
        assert!(!source.contains("JwtService::verify_token"), "duplicate JWT verification in {}", relative(&path));
    }
}
```

Adapt the helper names to the file's existing `read_source`/directory traversal helpers instead of adding duplicate filesystem helpers. Keep the assertions exactly equivalent.

- [ ] **Step 2: Run the guard tests before changing old expectations**

Run:

```bash
cd backend-school
cargo test --test static_architecture timetable_websocket_identity_is_server_owned -- --nocapture
cargo test --test static_architecture permission_cache_and_process_events_are_tenant_explicit -- --nocapture
cargo test --test static_architecture feature_modules_do_not_parse_jwt_directly -- --nocapture
```

Expected: the new tests expose any missed legacy contract; old event/cache expectation tests may also fail until updated.

- [ ] **Step 3: Update existing static expectations to the new APIs**

Update the existing invalidation scanner to require the matching tenant-aware event within the following three source lines:

```rust
if line.contains("permission_cache.invalidate_tenant(")
    && !next_lines.contains("notify_all_permissions_changed(")
{
    violations.push(format!("{}:{}: tenant invalidation must emit tenant permission_changed", relative(&file), index + 1));
}
if line.contains("permission_cache.invalidate_user(")
    && !next_lines.contains("notify_permission_changed(")
{
    violations.push(format!("{}:{}: user invalidation must emit tenant permission_changed", relative(&file), index + 1));
}
```

Update the permission SSE contract assertions to expect `for_user(tenant: &str, user_id: Uuid)`, `for_all_users(tenant: &str)`, `applies_to(&self, tenant: &str, user_id: Uuid)`, and `event.applies_to(&tenant, user_id)`. Update the work SSE contract to require tenant parameters and matching:

```rust
assert!(Regex::new(r"notify_work_items_changed\s*\(\s*&self,\s*tenant:\s*&str").unwrap().is_match(&app_state));
assert!(Regex::new(r"notify_workflow_window_changed\s*\(\s*&self,\s*tenant:\s*&str").unwrap().is_match(&app_state));
assert!(work_handler.contains("notify_work_items_changed(&context.tenant.subdomain)"));
assert!(workflow_handler.contains("notify_workflow_window_changed(&context.tenant.subdomain)"));
assert!(notification_handler.contains("event.applies_to(&tenant)"));
```

- [ ] **Step 4: Add the dedicated Nginx WebSocket location before `location /`**

```nginx
location /ws/ {
    proxy_pass http://schoolorbit-backend-school:8081;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header Origin $http_origin;
    proxy_read_timeout 120s;
    proxy_send_timeout 120s;
}
```

Do not add access-log directives that print query strings. Keep backend Origin/subdomain validation as the authorization boundary.

- [ ] **Step 5: Document sandbox and strict-token rollout checks**

Add this executable checklist to `docs/TESTING.md`:

```markdown
### Tenant-authenticated timetable realtime

1. Deploy backend first; existing school JWTs are intentionally rejected and users log in once.
2. Verify `GET /api/auth/me` succeeds on the login tenant and returns 401 when the same cookie is sent with another tenant header/origin.
3. Set `E2E_SEMESTER_ID` to a sandbox semester UUID. Verify `/ws/timetable?semester_id=$E2E_SEMESTER_ID` returns 401 without the HttpOnly cookie, 403 for an authenticated user without read/manage permission, and 404 when the value is changed to a valid UUID that is absent from that tenant.
4. Open two authorized tabs and verify one joined presence, no leave event until the final tab closes, cursor collaboration for readers, and edit collaboration only for managers.
5. Disable the network, restore it, and verify one reconnect attempt resumes with no reconnect loop after page teardown.
6. Confirm logs contain no JWT, password, national ID, database URL, or malformed WebSocket payload.
```

- [ ] **Step 6: Verify configuration and guards**

Run:

```bash
cd backend-school
cargo test --test static_architecture -- --nocapture
cd ..
docker run --rm -v "$PWD/nginx-configs/school-api.schoolorbit.app.conf:/etc/nginx/conf.d/default.conf:ro" nginx:alpine nginx -t
git diff --check
```

Expected: architecture tests pass; Nginx reports configuration syntax successful if Docker is available. If Docker is unavailable, record that exact environment limitation and validate with the deployment host before rollout.

- [ ] **Step 7: Commit guards and operations contract**

```bash
git add backend-school/tests/static_architecture.rs nginx-configs/school-api.schoolorbit.app.conf docs/TESTING.md
git commit -m "test(security): guard tenant realtime boundary"
```

---

### Task 8: Full verification, security audit, and result recording

**Files:**
- Modify: `docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md`
- Verify: all files from Tasks 1-7

**Interfaces:**
- Consumes: the complete secured tenant/realtime implementation.
- Produces: actual command results and an updated P0 status in the project improvement analysis.

- [ ] **Step 1: Run backend formatting, tests, check, and lint from a clean command context**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test --bin backend-school -- --skip modules::auth::tests::auth_tests::test_login_success --skip modules::auth::tests::auth_tests::test_login_invalid_credentials
cargo test --test static_architecture
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: every command exits 0. Database-dependent tests that require `TEST_DATABASE_URL` are run only against the safe environment defined in `docs/TESTING.md`; their actual availability/result is recorded.

- [ ] **Step 2: Run frontend check, static tests, targeted lint/format, and production build**

```bash
cd frontend-school
npm run check
npm run test:static
npx eslint src/lib/stores/timetable-socket.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte'
npx prettier --check src/lib/utils/timetable-reconnect.ts src/lib/stores/timetable-socket.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte' tests/static/timetable-realtime-security.test.mjs
npm run build
```

Expected: every command exits 0.

- [ ] **Step 3: Run repository-wide boundary searches**

```bash
sed -n '/pub struct WsParams/,/State Manager/p' backend-school/src/modules/academic/websockets.rs | rg -n 'user_id|name|school_key'
sed -n '/type TimetableSocketParams/,/export function disconnectTimetableSocket/p' frontend-school/src/lib/stores/timetable-socket.ts | rg -n 'school_key|name:\s*string|user_id:\s*String\(params\.user_id\)'
rg -n 'Sender<\(Uuid, Notification\)>|permission_cache\.(invalidate\(|clear_all\()' backend-school/src
rg -n 'JwtService::verify_token' backend-school/src/modules
rg -n 'auth_token|JWT_SECRET|ENCRYPTION_KEY|BLIND_INDEX_KEY|national_id' backend-school/src/modules/academic/websockets.rs frontend-school/src/lib/stores/timetable-socket.ts
```

Expected: both identity-contract searches return no matches; the cache/event search returns no legacy API; the JWT search returns no feature-module parser; the final search returns no secret/PII handling in realtime files.

- [ ] **Step 4: Review diffs for migration, PII, and unrelated changes**

```bash
git diff --check
git diff --name-only -- backend-school/migrations
git status --short
git diff --stat
```

Expected: diff check passes, migration file list is empty, and status contains only files deliberately changed by this plan plus the pre-existing analysis report until it is added in the next step.

- [ ] **Step 5: Record delivered P0 outcomes and actual verification state**

In `docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md`, mark the tenant JWT/cache/realtime P0 item completed and add a dated verification note containing the exact successful commands. If any environment-dependent command could not run, name the command and reason without describing it as passed.

Use this outcome wording:

```markdown
### P0 tenant authentication and realtime boundary — completed 2026-07-21

- JWTs are tenant-, issuer-, audience-, and version-bound; the request tenant must match before tenant data is read.
- Permission cache entries and process-global notification/permission/work events are tenant-scoped.
- Timetable WebSocket identity and rooms are server-derived; connection/edit permissions, frame limits, heartbeat, and bounded reconnect are enforced.
- No database migration was added or modified.
- Verification commands run on 2026-07-21 are listed here with one of two exact prefixes: `PASS —` followed by the successful command, or `NOT RUN —` followed by the command and the concrete environment limitation.
```

Replace the final verification guidance sentence with the actual `PASS —` and `NOT RUN —` entries before committing the file.

- [ ] **Step 6: Commit the analysis status and perform final verification**

```bash
git add docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md
git commit -m "docs: record tenant realtime security completion"
git diff HEAD~1 --check
git status --short
```

Expected: final diff check exits 0 and the worktree has no unexpected changes. Report commit hashes, test outcomes, and any safe-environment check that remains for deployment; do not claim unrun checks passed.
