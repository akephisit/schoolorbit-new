# Frontend Route Data Loading Design

Date: 2026-09-18

Status: Approved in chat and after written-spec review

## Purpose

Make navigation between authenticated `frontend-school` menu routes feel fast and consistent. The
change moves primary page reads into the SvelteKit route lifecycle, removes independent request
waterfalls, introduces typed page-view endpoints where one payload has one UI lifecycle, and keeps
post-mutation refreshes focused on the resource that changed.

This is a cross-stack performance change. `frontend-school` owns route loading and local state,
while `backend-school` may add read-only aggregate endpoints when several datasets are always
required together. Existing authorization, OpenAPI generation, and typed API boundaries remain
authoritative.

## Current Evidence

The source audit on 2026-09-18 found:

- 104 `+page.svelte` files under `frontend-school/src/routes/(app)`;
- 75 of those pages use `onMount`;
- 72 pages both use `onMount` and import a frontend API module;
- no authenticated `+page.ts` currently imports a frontend API module; and
- `frontend-school/src/app.html` already enables `data-sveltekit-preload-data="hover"`.

The existing hover setting can preload route code and run SvelteKit `load`, but it cannot see API
calls that begin only after the page component mounts. As a result, most menu links preload code and
metadata without preloading their primary data.

The representative route `/staff/academic/delivery` demonstrates a second problem. It currently
loads the homeroom delivery workspace, waits for that request, then loads academic term change sets.
Depending on the result and permissions, it may start the learning-delivery overview only after the
first two requests. Independent network latency therefore accumulates across two or three stages.

The production URL supplied during discovery could not be profiled from this session because no
interactive browser was available. Runtime request timing remains an explicit implementation and
acceptance gate; source structure alone is not evidence that a latency target has been met.

## Goals

- Start safe primary reads early enough for SvelteKit navigation preloading to perform useful work.
- Give each menu route a deliberate data-loading contract rather than component-local startup
  effects.
- Reduce primary navigation to one typed page-view request when the returned data is always consumed
  together and has the same authorization and freshness lifecycle.
- Run remaining independent initial reads concurrently.
- Load optional, expensive, or action-only data only after the relevant permission and user action.
- Update local state from typed mutation responses instead of broadly reloading the route.
- Apply the pattern to every navigable authenticated menu route, not only Academic Delivery.
- Preserve existing behavior, authorization, security, and API type guarantees.
- Record durable requirements in `.rules` and add automated regression guards.

## Non-goals

- Mandating exactly one HTTP request for every page regardless of payload size or usage.
- Creating a generic batch, proxy, GraphQL, or arbitrary multi-request endpoint.
- Eagerly loading dialog contents, inactive-tab data, exports, or other optional workflows.
- Caching authentication or permissions outside their existing canonical stores and invalidation
  rules.
- Changing business rules, permission scopes, database schemas, or migrations.
- Optimizing public routes or first-login rendering as part of this change unless a shared change is
  required for authenticated menu navigation.

## Data Classification

Every authenticated route must classify reads before implementation:

1. **Primary page data** is required for the default visible state. The route loader owns it.
2. **Shared route context** is reused across routes, such as academic year and term options. Its
   existing shared store may retain a session-scoped in-flight result, but the route declares the
   context it depends on.
3. **Optional interaction data** is needed only for an inactive tab, dialog, editor, export, or
   explicit user action. It stays lazy.
4. **Action-only data** requires a create, update, approve, export, or other exact permission. It is
   never part of a read-only page's eager load.
5. **Rapidly changing data** may use tap preloading or no data preloading when hover preloading would
   create excessive false positives or unacceptable staleness.

This classification prevents a nominal one-request design from becoming a large overfetch that is
slower than several focused reads.

## Route Loading Architecture

### Loader ownership

Primary page data moves to the route's `+page.ts` or the narrowest shared `+layout.ts`. The loader:

- parses and validates only the route params and search params it owns;
- uses the SvelteKit load event's `fetch`, or an explicitly declared load dependency, through a typed
  API wrapper;
- returns a typed page-data model to the Svelte component;
- starts independent reads together with `Promise.all`; and
- maps expected load failures into a typed state that the page can render with the shared
  `PageState` components.

Components may still use `onMount` for browser-only listeners, observers, and interaction-only
features. They must not use it as the normal owner of primary route data.

The API client will gain a narrow way to use the load event's `fetch` without weakening its response
envelope, session-security headers, tenant hint, maintenance handling, or concrete generic response
types. Route modules continue to consume generated wire DTOs through feature API modules rather
than calling raw endpoints ad hoc.

### Preload policy

The existing body-level hover preload remains the default for idempotent, reasonably sized,
permission-safe reads. It allows SvelteKit to import route code and run `load` before the click when
the user's connection settings permit it.

Context-scoped menu links must carry the currently selected academic year and term when the target
route declares that requirement. Otherwise the speculative loader cannot issue its primary request
until a second navigation repairs the URL. Navigation controls that are not rendered as ordinary
links, such as the collapsed sidebar menu, call SvelteKit `preloadData` on pointer hover, keyboard
focus, or touch start with the same context-bearing destination, and then navigate to that exact
destination so the preloaded result can be reused.

Routes with expensive or highly volatile primary reads override the link to `tap`. Routes must not
use hover preload for mutations. Backend authorization remains authoritative even when a permitted
menu link speculatively preloads a GET request.

Programmatic preloading is reserved for navigation controls that are not rendered as ordinary links
or for a measured high-value path. It must not duplicate the normal link preload.

### Page-view endpoints

A read-only page-view endpoint is appropriate when all of these conditions hold:

- the datasets are required for the route's default visible state;
- they share the same route parameters, authorization boundary, and freshness lifecycle;
- separate requests create measurable round-trip or query-planning overhead; and
- the combined response remains bounded and useful as one typed view model.

The backend service composes the datasets and runs independent database work concurrently where the
pool and query design make that safe. The endpoint returns one named response DTO in the standard API
envelope, is registered in OpenAPI, and is consumed through generated frontend types.

Page-view endpoints do not absorb optional tabs, dialogs, exports, or action-only datasets. They are
route-facing read models, not new business-data owners. Existing domain services remain the owners
of queries and authorization decisions.

No generic batch endpoint will be introduced. Such an endpoint would obscure authorization,
contracts, caching, errors, and query cost without reducing the underlying work.

## Representative Academic Delivery Flow

The default Academic Delivery route becomes the reference implementation:

1. The route loader reads `academicYearId`, `academicTermId`, and the optional timetable version.
2. It calls one typed read-only delivery page-view endpoint.
3. The backend authorizes each included view and composes the homeroom workspace, term change sets,
   and only the overview data required by the default view.
4. The page renders from loader data without starting a second primary load in `onMount`.
5. Inactive-tab or management-only data that is not required for the default view loads when the
   user opens that workflow.
6. Context changes rerun the route loader through URL dependencies and cancel superseded client
   navigation work.
7. Mutations return typed affected resources. The page patches them locally or invalidates only the
   delivery page dependency when the mutation has intentionally broad consequences.

The exact endpoint payload will be finalized from the existing backend service outputs during the
implementation plan. It must not duplicate domain models or include action-only management options.

## Mutation and Invalidation Rules

Typed mutation responses are the preferred source for immediate UI reconciliation:

- replace or insert the affected row, card, or aggregate entry;
- update a bounded parent summary when the response carries enough information;
- invalidate a named route dependency when the change affects an intentionally broad page view; and
- reserve `invalidateAll()` for session-wide or deliberately broad dependencies and manual refresh.

Every broad invalidation needs a documented reason in code review. A mutation must not silently
trigger unrelated menu, auth, permission, or route reloads.

Realtime events continue to signal that authoritative data changed. Their consumers invalidate the
narrow route or store owner unless the existing event contract explicitly requires a wider refresh.

## Cache Policy

This design does not add a generic GET cache. A cache is permitted only when it names:

- its data owner;
- its key, including tenant and all route context;
- its maximum age;
- its mutation and realtime invalidation events; and
- its logout and user-change cleanup behavior.

Auth, permission, and sensitive user data remain governed by their existing stores and cache rules.
Stale-while-revalidate behavior may be added later only to a measured stable read model with focused
tests. Preloading and request parallelism do not depend on such a cache.

## Authorization and Security

- Backend policies remain the source of truth for every page-view field.
- Frontend menu filtering and route guards remain convenience UX, not authorization.
- Read-only users must not fail because a page view eagerly fetches action-only data.
- Page-view responses must minimize PII and omit national IDs, blind indexes, and unrelated contact,
  medical, guardian, document, or credential data.
- Preloaded requests are GET-only and must have no state-changing side effects.
- Existing session, tenant-origin, CSRF, maintenance, and 401 handling must remain intact when route
  loaders use the API client.

## Error and Cancellation Behavior

- A failed primary page view renders the existing shared error state with a focused retry.
- A failed optional read affects only its panel or dialog.
- Superseded URL or context loads are aborted or ignored through the existing latest-request pattern
  or SvelteKit navigation lifecycle.
- One optional failure must not discard successfully loaded primary data.
- A page-view endpoint reports one safe top-level failure for an unusable primary view. It must not
  expose raw database or internal query errors.
- Unauthorized and forbidden responses continue through the canonical login and `/403` behavior.

## Durable `.rules` Changes

The Frontend: SvelteKit 5 section will gain a route-loading subsection with these requirements:

- Load primary route data from `+page.ts` or the narrowest applicable `+layout.ts`; do not defer it to
  component `onMount`.
- Use the SvelteKit load event's `fetch` through typed API modules so safe read navigation can benefit
  from data preloading and dependency tracking.
- Carry required academic context in menu destinations; non-link navigation controls preload the
  same resolved destination that they later navigate to.
- Use hover preload only for safe, bounded, idempotent reads; use tap or disable data preload for
  expensive or highly volatile routes.
- Combine always-co-consumed datasets behind a named typed page-view endpoint when measurement shows
  that round trips are material. Do not add generic batch endpoints or overfetch optional and
  action-only data.
- Run independent initial reads concurrently. Sequential awaits require a real data dependency.
- Load optional and exact-permission data lazily after the relevant action or permission check.
- Patch typed mutation results into local state or invalidate a named dependency. Reserve broad
  reloads for intentionally broad dependencies or manual refresh.
- Give any client cache an explicit owner, tenant/context key, lifetime, invalidation events, and
  logout cleanup. Do not introduce a generic auth or permission cache.

These rules supplement, rather than replace, the existing requirements to load route-specific data
only, keep API boundaries typed, and prevent read-only pages from fetching admin-only data.

## Regression Guards and Verification

### Static guards

Frontend static tests will enforce durable boundaries without banning legitimate browser effects:

- authenticated menu pages may not start primary route reads from `onMount`;
- route loaders that fetch page data use typed feature API modules and the load-aware transport;
- independent primary reads in approved loaders are not expressed as serial awaits;
- new broad `invalidateAll()` use is rejected unless it is in an explicitly reviewed owner; and
- the body-level preload policy and route-specific overrides remain intentional.

The implementation plan will define an explicit migration inventory in executable test data rather
than a Markdown progress report. Temporary allowlists may only shrink during rollout and must be
empty for menu-route primary reads before completion.

### Focused behavior tests

- API-client tests cover load-event `fetch`, session headers, tenant hints, maintenance responses,
  aborts, and unauthorized handling.
- Route tests cover URL parsing, one primary load per dependency change, cancellation, retry, and
  error-state preservation.
- Backend tests cover page-view authorization, read-only access, action-only field exclusion, DTO
  shape, and concurrent composition behavior where applicable.
- Academic Delivery tests verify one primary page-view request for the default route and lazy
  requests for optional workflows.
- Mutation tests verify local patching or named invalidation without unrelated route reloads.

### Runtime acceptance

A disposable authenticated Playwright account will measure representative routes in each menu
domain. The harness records route-module completion, primary request count, request start order,
navigation completion, and the time until primary content replaces the skeleton.

Acceptance requires:

- every navigable authenticated menu route has an explicit primary-data classification;
- no route has an independent serial initial-request waterfall;
- primary reads owned by migrated routes begin in the route lifecycle rather than after mount;
- Academic Delivery uses one primary page-view request for its default visible state;
- optional and action-only data remains lazy;
- broad mutation reloads are removed unless explicitly justified; and
- required frontend, backend, API-contract, static, and browser checks pass on the exact final tree.

No universal millisecond threshold is asserted without a controlled deployed baseline. Each domain
records before-and-after timings on the same environment and connection profile, and completion
requires a clear improvement without increasing transferred primary data unreasonably.

## Rollout

The implementation remains one architecture program but lands in reviewable domain waves:

1. Add the load-aware typed transport, `.rules` requirements, static guard framework, runtime
   measurement harness, and Academic Delivery reference implementation.
2. Migrate the remaining academic menu routes, adding page-view endpoints only where the criteria
   are met.
3. Migrate staff operations, people, organization, work, certificates, facilities, and settings.
4. Migrate student and parent authenticated menus.
5. Remove the temporary migration inventory, run cross-domain runtime acceptance, and verify every
   authenticated menu route against the final guard.

Each wave keeps the application runnable and preserves behavior. A route leaves the migration
inventory only after its focused tests and request-shape verification pass. Generated contracts are
updated in the same commit as any endpoint change.

## Required Verification Matrix

At minimum, the final affected tree runs:

From `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
npm run build
```

Focused Playwright route-performance coverage runs with a dedicated disposable authenticated
account. Missing credentials are reported as unrun rather than replaced with weaker evidence.

From `backend-school` when page-view endpoints change:

```bash
cargo fmt --all -- --check
cargo test api_contract::tests -- --nocapture
cargo test --test static_architecture
cargo check --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
```

Every affected backend package also runs its focused unit and database tests through the repository's
documented test runner where required.

From the repository root:

```bash
git diff --check
git status --short
```

## Risks and Mitigations

- **Speculative hover traffic:** use bounded GET page views, override expensive or volatile links to
  tap, and measure false-positive traffic.
- **Larger aggregate payloads:** include only default visible data and retain optional lazy reads.
- **Stale UI after mutation:** prefer returned resources and named invalidation with explicit tests.
- **Permission overreach:** authorize every backend component and exclude action-only fields for
  read-only users.
- **Large migration surface:** use a shrinking executable inventory and domain waves, with the final
  guard requiring zero remaining menu-route primary reads in `onMount`.
- **Navigation behavior regression:** test deep links, hover, touch/tap, back/forward, URL context
  changes, aborted navigation, and 401/403 paths.

## Decisions

- The target is one primary request where data shares a lifecycle, not one request at any cost.
- SvelteKit route loaders own primary navigation reads.
- Optional and action-only reads stay lazy.
- Page-view endpoints are named and typed; no generic batching layer is added.
- Backend changes are limited to read aggregation and supporting typed contracts unless a separately
  approved finding requires more.
- The durable standard is added to `.rules` and enforced by tests.
- The scope covers every navigable authenticated `frontend-school` menu route through staged domain
  migration.
