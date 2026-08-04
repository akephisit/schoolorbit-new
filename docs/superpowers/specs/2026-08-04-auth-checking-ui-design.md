# Auth Checking UI Design

## Problem

Protected routes currently expose two consecutive full-screen checking states. The `(app)` layout checks the current session and route access with a CSS spinner, while the `staff`, `student`, and `parent` layouts render a second branded icon state while rechecking the same authentication store. The login page uses a third copy of the branded state.

This makes one authorization flow look like multiple permission systems and can flash between different loading presentations.

## Considered Approaches

1. Keep every guard and only make their markup identical. This is the smallest visual change, but it preserves duplicate full-screen states and still allows a second loading screen to flash.
2. Make the `(app)` layout the single protected-route guard and reuse one branded loading component in `(app)` and login. Portal layouts become presentation-only wrappers. This removes the root cause while keeping the existing `/api/auth/me`, route metadata, and backend authorization boundaries unchanged.
3. Move all redirects into server loaders. This could reduce client-only transitions, but it is a larger authentication architecture change involving cookie/backend routing behavior and is outside this UI consistency fix.

Approach 2 is selected. It is the design already approved in the preceding review.

## Design

Create `AuthCheckingState.svelte` in the shared app-state component directory. It renders one centered SchoolOrbit academic icon, a restrained pulse animation, and a caller-provided status message. It owns the full-screen loading presentation and accessibility attributes.

The protected `(app)` layout continues to call `authAPI.checkAuth()`, populate the auth and permission stores, evaluate `userCanAccessRoute`, and redirect to login or `/403`. Its existing inline spinner is replaced with `AuthCheckingState`.

The login page continues checking `/api/auth/me` so an already-authenticated user is redirected to the correct dashboard. Its copied icon markup is replaced with the same component.

The `staff`, `student`, and `parent` layouts stop reading `authStore`, redirecting, and rendering their own full-screen checking state. They render only their existing content wrapper. User-type access remains owned by route metadata and `userCanAccessRoute`; backend policies remain authoritative for data and mutations.

## Error and Redirect Behavior

- An invalid or expired session still redirects from `(app)` to `/login`.
- An authenticated user without route access still redirects to `/403`.
- Direct navigation to `/login` still checks for an existing session and redirects authenticated users.
- The shared component displays `กำลังตรวจสอบสิทธิ์...` while checking and `กำลังเปลี่ยนหน้า...` during redirects.

## Verification

- Add a focused frontend static architecture regression test proving that `(app)` and login consume the shared checking state and portal layouts no longer own authentication/loading boundaries.
- Run the focused test red before implementation and green after implementation.
- Run Svelte autofixer on every changed component.
- Run the frontend verification matrix from `.rules`: `npm run lint`, `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`, and `npm run test:static`.
- Run `git diff --check` and inspect the final diff and status.
