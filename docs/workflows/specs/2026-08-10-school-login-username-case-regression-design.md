# School Login Username Case Regression Design

**Date:** 2026-08-10

## Problem

The session login flow currently trims and lowercases the supplied username, then uses that normalized value both for the login throttle bucket and the database lookup. Existing school staff accounts are generated with case-sensitive usernames such as `T0001`, while the `users.username` column and its unique constraint preserve PostgreSQL's case-sensitive equality semantics. The lookup therefore searches for `t0001`, does not find `T0001`, and returns the generic invalid-credentials response before checking the unchanged password.

The former JWT login flow passed the supplied username directly to the same case-sensitive database lookup, so this is a session-cutover regression rather than a credential change.

## Decision

Use one canonical login identifier for both account lookup and identifier throttling: the supplied username with surrounding whitespace removed and letter case preserved.

The flow will be:

1. Validate the bounded username and password input as today.
2. Trim surrounding whitespace from the username without lowercasing it.
3. Derive the privacy-preserving identifier throttle bucket from that case-preserving value.
4. Query `users.username` with the same case-preserving value.
5. Continue the existing bcrypt verification, account-status check, session creation, cookie response, and throttle cleanup unchanged.

This keeps authentication and throttling identity semantics aligned. `T0001` and `t0001` remain distinct identifiers, matching the current database constraint. Source-address throttling continues to limit attempts that vary the username.

## Alternatives Considered

### Lowercase only for throttling

This would restore uppercase account lookup while making `T0001` and `t0001` share a throttle bucket. It mixes two identity definitions and permits one case-distinct account to affect another, so it is not selected.

### Case-insensitive database lookup

Using `LOWER(username)` or `ILIKE` would make letter case optional, but the current case-sensitive unique constraint can permit case-distinct rows. Selecting case-insensitively before auditing duplicates and enforcing a matching unique identity constraint could be ambiguous. This requires a separate migration and rollout design if desired later.

## Scope

Only `backend-school` authentication policy/service code and its tests are in scope. There is no password reset or rehash, database migration, API-contract change, environment-variable change, frontend change, or backend/frontend admin change.

## Error and Security Behavior

Public authentication errors remain generic so they do not reveal account existence. Passwords, usernames, source addresses, throttle hashes, and session credentials remain absent from logs. Existing per-identifier and per-source throttles, bcrypt verification, active-status enforcement, opaque session creation, CSRF generation, and secure cookie attributes remain unchanged.

## Testing and Rollout

A regression test will insert an active user whose username contains uppercase letters, log in with the same case and the existing password, and assert that authentication succeeds and a session is created. Policy tests will assert that normalization trims surrounding whitespace while preserving letter case. Existing generic-error and throttle tests must remain green.

Verification will follow the repository change-type matrix for backend-school Rust changes. After pushing `main`, deploy only backend-school and run an authenticated school smoke flow when smoke credentials are available: login, receive the session cookie, and call `/api/auth/me`. If previous failed attempts have an active temporary throttle, wait for its maximum short backoff before retesting.
