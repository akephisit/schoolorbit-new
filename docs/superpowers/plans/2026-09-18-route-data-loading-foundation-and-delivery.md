# Route Data Loading Foundation and Academic Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the reusable route-loading standard and migrate Academic Delivery from serial `onMount` reads to one preloadable typed page-view request with lazy optional data and focused mutation refreshes.

**Architecture:** SvelteKit `+page.ts` owns the primary read and supplies the load event's `fetch` through the typed API client. A read-only `backend-school` page-view endpoint composes the homeroom workspace and term-change data concurrently, adding the offering overview only when visible change items need it; optional tabs and management options stay lazy. A shrinking static baseline prevents new `onMount` primary reads while later domain plans migrate the remaining routes.

**Tech Stack:** SvelteKit 5, Svelte 5, TypeScript, Vite, Node test runner, Playwright, Rust, Axum, SQLx, Tokio/Futures, utoipa/OpenAPI.

**Spec:** `docs/superpowers/specs/2026-09-18-frontend-route-data-loading-design.md`

## Global Constraints

- Read `.rules` and `docs/TESTING.md` before implementation; `.rules` remains authoritative.
- Never edit an applied migration. This plan requires no database migration.
- Never store or log plaintext national IDs, credentials, cookies, tokens, database URLs, or raw sensitive payloads.
- Backend authorization remains authoritative; read-only page loading must not request management-only data.
- Rust DTOs plus utoipa own the API wire contract. Regenerate `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts`; never edit either generated file directly.
- Frontend API wrappers use concrete generated response types and the shared API envelope.
- Primary route reads use the SvelteKit load event's `fetch`; mutations never run during preload.
- Academic menu destinations preserve the selected year and term. Collapsed non-link controls
  preload and navigate to the same resolved destination.
- Exactly one primary Delivery page-view request is allowed for the default route. Offering overview and management data remain lazy unless the default visible change-set panel needs offering labels.
- Independent backend reads run concurrently; sequential work must have a real data dependency.
- Mutation responses patch local state when their effect is local. Publishing or another intentionally broad delivery change invalidates only the named Delivery page dependency.
- This plan implements rollout wave 1 only. Separate plans migrate the remaining academic, staff, student, and parent menu domains before the design program is complete.

---

## File Structure

### Durable policy and regression ownership

- Modify `.rules` — own the durable route-loading, preload, page-view, mutation, and cache rules.
- Modify `frontend-school/tests/static/documentation-policy.test.mjs` — prove `.rules` contains the durable standard.
- Create `frontend-school/tests/static/route-data-loading-policy.test.mjs` — own the shrinking legacy baseline and prohibit new primary API reads in route `onMount`.

### Frontend transport and route-data foundation

- Modify `frontend-school/src/lib/api/client.ts` — allow a request to use the SvelteKit load event's `fetch` without bypassing session, tenant, maintenance, and envelope handling.
- Modify `frontend-school/tests/static/api-query-contract.test.mjs` — verify every shared transport forwards the supplied fetch implementation.
- Create `frontend-school/src/lib/navigation/route-load.ts` — provide a small typed success/error result for route loaders.
- Create `frontend-school/tests/static/route-load-result.test.mjs` — verify the route-load result helper.
- Modify `frontend-school/src/lib/academic-context/route-context.ts` — build context-bearing menu destinations from route metadata.
- Modify `frontend-school/src/lib/components/layout/Sidebar.svelte` — use those destinations for normal links and programmatic collapsed-menu preload/navigation.
- Modify `frontend-school/tests/static/academic-context-contract.test.mjs` — verify academic context survives menu navigation and the collapsed menu preloads the exact destination.

### Backend Delivery page view

- Modify `backend-school/crates/school-academic-delivery/Cargo.toml` — add the workspace `futures` runtime dependency.
- Modify `backend-school/crates/school-academic-delivery/src/models.rs` — define `LearningDeliveryPageView`.
- Modify `backend-school/crates/school-academic-delivery/src/services/workspaces.rs` — compose the page view and decide when its overview is required.
- Modify `backend-school/src/modules/academic/delivery/services_tests.rs` — cover the bounded page-view service against disposable PostgreSQL.
- Modify `backend-school/src/modules/academic/delivery/handlers.rs` — expose the authorized read handler.
- Modify `backend-school/src/modules/academic/delivery.rs` — register `/delivery/page-view`.
- Modify `backend-school/src/api_contract.rs` — register and test the path, query, response, and schemas.
- Modify `backend-school/tests/static_architecture.rs` — enforce read-only list authorization on the page-view handler.
- Regenerate `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts`.

### Frontend Delivery migration

- Modify `frontend-school/src/lib/api/learning-delivery.ts` — expose the generated page-view type and typed GET wrapper.
- Create `frontend-school/src/lib/academic/learning-delivery-page.ts` — own the named dependency, URL context parsing, and refresh-scope type.
- Modify `frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts` — start the primary request in `load` with the SvelteKit fetch implementation.
- Modify `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte` — hydrate from route data, keep optional overview loading lazy, and remove primary `onMount` reads.
- Modify `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte` — distinguish local change-set reconciliation from broad page refresh.
- Modify `frontend-school/src/lib/components/learning-delivery/AcademicChangeReadiness.svelte` — request a page refresh only after publishing.
- Modify `frontend-school/src/lib/components/learning-delivery/TeacherHandoffPanel.svelte` — use the shared refresh-scope callback type.
- Modify `frontend-school/tests/static/learning-delivery-workspace.test.mjs` — enforce loader ownership, typed contracts, lazy optional data, and focused refreshes.
- Modify `frontend-school/tests/static/academic-workspace-request-count.test.mjs` — enforce the single primary Delivery page-view request.
- Modify `frontend-school/tests/e2e/homeroom-delivery-workspace.spec.ts` — verify the browser request shape and tab-lazy behavior.

---

### Task 1: Add the Durable Route-Loading Standard

**Files:**
- Modify: `.rules`
- Modify: `frontend-school/tests/static/documentation-policy.test.mjs`

**Interfaces:**
- Consumes: the approved design's route-loading decisions.
- Produces: the authoritative `### Route data loading and navigation performance` rules used by every later task and rollout plan.

- [ ] **Step 1: Write the failing documentation-policy assertions**

Add this test to `frontend-school/tests/static/documentation-policy.test.mjs`:

```js
test('development rules own route data loading and navigation performance', async () => {
	const rules = await readFile(path.join(repoRoot, '.rules'), 'utf8');

	assert.match(rules, /### Route data loading and navigation performance/);
	assert.match(rules, /\+page\.ts.*\+layout\.ts[\s\S]*primary route data/);
assert.match(rules, /load event's `fetch`/);
assert.match(rules, /academic context in menu destinations/);
	assert.match(rules, /page-view endpoint/);
	assert.match(rules, /Independent initial reads[\s\S]*concurrently/);
	assert.match(rules, /invalidateAll\(\)/);
	assert.match(rules, /cache[\s\S]*tenant[\s\S]*invalidation/);
});
```

- [ ] **Step 2: Run the documentation test and confirm the red state**

Run:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: FAIL in `development rules own route data loading and navigation performance` because the heading is absent.

- [ ] **Step 3: Add the route-loading subsection to `.rules`**

Insert this subsection under `## 7. Frontend: SvelteKit 5`, immediately after `### Data, state, and components`:

```markdown
### Route data loading and navigation performance

- Load primary route data from `+page.ts` or the narrowest applicable `+layout.ts`; do not defer primary reads to component `onMount`. `onMount` remains valid for browser-only listeners, observers, and interaction-only features.
- Use the SvelteKit load event's `fetch` through typed feature API modules so safe read navigation can benefit from data preloading and dependency tracking. Preloaded operations must be bounded, idempotent GET requests with no state-changing side effects.
- Keep required academic context in menu destinations so hover preloading can issue the primary read. A navigation control that is not an ordinary link must preload and navigate to the same resolved destination.
- Keep hover preload for safe, reasonably sized, stable reads. Use tap or disable data preload for expensive or highly volatile routes where false-positive traffic or staleness is material.
- When datasets are always consumed together and share route parameters, authorization, and freshness, use a named typed page-view endpoint if measurement shows material round-trip overhead. Do not add generic batch endpoints or include inactive tabs, dialogs, exports, or action-only data in the page view.
- Run independent initial reads concurrently. Sequential awaits require a real data dependency.
- Load optional data and data guarded by create, update, approve, export, or other exact permissions only after the relevant permission passes and the user opens that workflow.
- After a typed mutation returns a resource, patch only the affected local state. Otherwise invalidate a named route dependency. Reserve `invalidateAll()` for deliberately session-wide dependencies or an explicit manual refresh.
- A client read cache must name its owner, tenant and route-context key, maximum age, mutation and realtime invalidation events, and logout/user-change cleanup. Do not add a generic cache for auth, permissions, or sensitive user data.
```

- [ ] **Step 4: Run the focused documentation test**

Run:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: PASS, 10 tests and 0 failures.

- [ ] **Step 5: Commit the durable policy**

```bash
git add .rules frontend-school/tests/static/documentation-policy.test.mjs
git commit -m "docs: define route data loading standard"
```

---

### Task 2: Add a Load-Aware Typed API Transport

**Files:**
- Modify: `frontend-school/src/lib/api/client.ts`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`

**Interfaces:**
- Consumes: SvelteKit's load event `fetch`, whose type is `typeof globalThis.fetch`.
- Produces: `ApiRequestOptions.requestFetch?: typeof globalThis.fetch`; every API client method forwards it to the actual network call.

- [ ] **Step 1: Extend the transport test with a failing fetch-forwarding contract**

Add this test to `frontend-school/tests/static/api-query-contract.test.mjs`:

```js
test('the shared client can execute through the SvelteKit load fetch', async () => {
	const source = await readFile(path.join(projectRoot, 'src/lib/api/client.ts'), 'utf8');

	assert.match(
		source,
		/interface ApiRequestOptions[\s\S]*requestFetch\?: typeof globalThis\.fetch/
	);
	assert.match(source, /response = await requestFetch\(`/);
	assert.doesNotMatch(source, /requestOptions[\s\S]*requestFetch:/);
	for (const method of [
		'get',
		'getBlob',
		'getExternalBlob',
		'post',
		'postPublic',
		'postBlob',
		'postBlobWithBody',
		'put',
		'patch',
		'delete',
		'deleteWithBody',
		'postMultipart'
	]) {
		const start = source.indexOf(`async ${method}`);
		assert.notEqual(start, -1, `missing ${method}`);
		const next = source.indexOf('\n\tasync ', start + 1);
		const block = source.slice(start, next === -1 ? source.length : next);
		assert.match(block, /options\.requestFetch/, `${method} must forward requestFetch`);
	}
});
```

- [ ] **Step 2: Run the focused test and confirm the red state**

Run:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

Expected: FAIL because `ApiRequestOptions` has no `requestFetch` property.

- [ ] **Step 3: Add the request fetch interface and transport parameter**

Update `ApiRequestOptions` and the internal transport signatures in `client.ts`:

```ts
export interface ApiRequestOptions {
	signal?: AbortSignal;
	query?: ApiQuery;
	requestFetch?: typeof globalThis.fetch;
}

private async fetchBackend(
	endpoint: string,
	options: RequestInit = {},
	transport: ApiTransport = 'session',
	requestFetch: typeof globalThis.fetch = globalThis.fetch
): Promise<Response> {
	// Preserve the existing header, credential, tenant, maintenance, and 401 behavior.
	response = await requestFetch(`${this.baseURL}${endpoint}`, requestOptions);
}

private async request<T, E = never>(
	endpoint: string,
	options: RequestInit = {},
	transport: ApiTransport = 'session',
	requestFetch: typeof globalThis.fetch = globalThis.fetch
): Promise<ApiResponse<T, E>> {
	const response = await this.fetchBackend(endpoint, { ...options, headers }, transport, requestFetch);
	// Keep the existing envelope parsing unchanged.
}
```

Pass `options.requestFetch` as the final argument from every public client method. For external blob reads, use:

```ts
const requestFetch = options.requestFetch ?? globalThis.fetch;
const response = await requestFetch(url, {
	method: 'GET',
	mode: 'cors',
	credentials: 'omit',
	referrerPolicy: 'no-referrer',
	signal: options.signal
});
```

Do not spread `requestFetch` into a `RequestInit` object.

Extend the currently option-less multipart method without breaking existing callers:

```ts
async postMultipart<T, E = never>(
	endpoint: string,
	body: FormData,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<T, E>> {
	const response = await this.fetchBackend(
		appendApiQuery(endpoint, options.query),
		{ method: 'POST', body, signal: options.signal },
		'session',
		options.requestFetch
	);
	// Preserve the existing response parsing below this call.
}
```

- [ ] **Step 4: Run the focused transport tests and type checker**

Run:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm --prefix frontend-school run check
```

Expected: both commands PASS with 0 failures and 0 Svelte errors.

- [ ] **Step 5: Commit the load-aware transport**

```bash
git add frontend-school/src/lib/api/client.ts frontend-school/tests/static/api-query-contract.test.mjs
git commit -m "feat(frontend): support load-scoped API fetch"
```

---

### Task 3: Compose the Delivery Page View in the Domain Crate

**Files:**
- Modify: `backend-school/crates/school-academic-delivery/Cargo.toml`
- Modify: `backend-school/crates/school-academic-delivery/src/models.rs`
- Modify: `backend-school/crates/school-academic-delivery/src/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: `HomeroomDeliveryWorkspace`, `AcademicTermChangeSet`, `LearningDeliveryOverview`, `AcademicResourceListFilter`.
- Produces: `LearningDeliveryPageView` and `workspaces::delivery_page_view(&PgPool, Uuid, Uuid, Option<Uuid>, &AcademicResourceListFilter) -> Result<LearningDeliveryPageView, AppError>`.

- [ ] **Step 1: Write the failing domain tests**

Extend the existing `#[cfg(test)] mod tests` in `workspaces.rs`: add `chrono::{NaiveDate, Utc}` and
`AcademicTermChangeSetStatus` to that module's imports, then add this helper and test inside the
existing module. Add the database test separately to
`backend-school/src/modules/academic/delivery/services_tests.rs`:

```rust
fn change_set_with(items: Vec<AcademicTermChangeItem>) -> AcademicTermChangeSet {
    let now = Utc::now();
    AcademicTermChangeSet {
        id: Uuid::nil(),
        academic_term_id: Uuid::nil(),
        academic_year_id: Uuid::nil(),
        effective_from: NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
        reason: "test".to_string(),
        status: AcademicTermChangeSetStatus::Draft,
        base_timetable_version_id: Uuid::nil(),
        target_timetable_version_id: Uuid::nil(),
        row_version: 1,
        created_by: Uuid::nil(),
        published_by: None,
        published_at: None,
        cancelled_by: None,
        cancelled_at: None,
        created_at: now,
        updated_at: now,
        items,
    }
}

#[test]
fn page_view_requires_overview_only_for_visible_offering_labels() {
    assert!(!page_view_requires_overview(&[]));
    assert!(!page_view_requires_overview(&[change_set_with(Vec::new())]));

    let now = Utc::now();
    let offering_item = AcademicTermChangeItem::AddOffering {
        id: Uuid::nil(),
        learning_offering_id: Uuid::nil(),
        weekly_period_target: 4,
        row_version: 1,
        created_by: Uuid::nil(),
        created_at: now,
        updated_at: now,
    };
    assert!(page_view_requires_overview(&[change_set_with(vec![offering_item])]));
}

#[tokio::test]
async fn delivery_page_view_batches_primary_reads_without_forcing_overview() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_page_view").await;
    let context = planning_runtime_context(&pool).await;
    assert!(change_sets::list_change_sets(&pool, context.term_id)
        .await
        .unwrap()
        .is_empty());

    let view = workspaces::delivery_page_view(
        &pool,
        context.year_id,
        context.term_id,
        None,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .expect("delivery page view should load");

    assert_eq!(view.workspace.academic_year_id, context.year_id);
    assert_eq!(view.workspace.academic_term_id, context.term_id);
    assert!(view.change_sets.is_empty());
    assert!(view.overview.is_none());
}
```

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-delivery page_view_requires_overview_only_for_visible_offering_labels
./scripts/test_backend_school.sh modules::academic::delivery::services_tests::delivery_page_view_batches_primary_reads_without_forcing_overview -- --nocapture
```

Expected: compilation FAIL because `LearningDeliveryPageView`, `page_view_requires_overview`, and `delivery_page_view` do not exist.

- [ ] **Step 3: Add the page-view DTO and runtime dependency**

Add `futures = { workspace = true }` to the crate's normal dependencies and add this model after `LearningDeliveryOverview`:

```rust
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningDeliveryPageView {
    pub workspace: HomeroomDeliveryWorkspace,
    pub change_sets: Vec<AcademicTermChangeSet>,
    #[schema(required = true)]
    pub overview: Option<LearningDeliveryOverview>,
}
```

- [ ] **Step 4: Implement concurrent primary composition with conditional overview loading**

Import `AcademicTermChangeItem`, `AcademicTermChangeSet`, and `LearningDeliveryPageView` in
`workspaces.rs`, import `super::change_sets`, and extend the existing test module's
model import to:

```rust
use crate::models::{
    AcademicTermChangeSetStatus, HomeroomGroupMode, HomeroomTeacherState,
    HomeroomTimetableState,
};
```

Then add:

```rust
fn page_view_requires_overview(change_sets: &[AcademicTermChangeSet]) -> bool {
    change_sets.iter().flat_map(|change_set| &change_set.items).any(|item| {
        matches!(
            item,
            AcademicTermChangeItem::AddOffering { .. }
                | AcademicTermChangeItem::StopOffering { .. }
                | AcademicTermChangeItem::AdjustWeeklyPeriodTarget { .. }
        )
    })
}

pub async fn delivery_page_view(
    pool: &PgPool,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    requested_timetable_version_id: Option<Uuid>,
    filter: &AcademicResourceListFilter,
) -> Result<LearningDeliveryPageView, AppError> {
    let (workspace, change_sets) = futures::try_join!(
        homeroom_delivery_workspace_for_version(
            pool,
            academic_year_id,
            academic_term_id,
            requested_timetable_version_id,
            filter,
        ),
        change_sets::list_change_sets(pool, academic_term_id),
    )?;
    let overview = if page_view_requires_overview(&change_sets) {
        Some(delivery_overview(pool, academic_term_id, filter).await?)
    } else {
        None
    };

    Ok(LearningDeliveryPageView {
        workspace,
        change_sets,
        overview,
    })
}
```

The overview remains absent when there are no visible change items; the offerings tab and change-item editor will use the existing lazy endpoint.

- [ ] **Step 5: Run the focused crate and database tests**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-delivery page_view_requires_overview_only_for_visible_offering_labels
./scripts/test_backend_school.sh modules::academic::delivery::services_tests::delivery_page_view_batches_primary_reads_without_forcing_overview -- --nocapture
```

Expected: both commands PASS.

- [ ] **Step 6: Commit the domain page view**

```bash
git add backend-school/crates/school-academic-delivery/Cargo.toml \
  backend-school/crates/school-academic-delivery/src/models.rs \
  backend-school/crates/school-academic-delivery/src/services/workspaces.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/Cargo.lock
git commit -m "feat(delivery): compose route page view"
```

---

### Task 4: Expose and Generate the Typed Page-View Contract

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/modules/academic/delivery.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**
- Consumes: `workspaces::delivery_page_view` from Task 3.
- Produces: GET `/api/academic/delivery/page-view`, operation ID `getLearningDeliveryPageView`, query `HomeroomDeliveryQuery`, response `ApiResponse<LearningDeliveryPageView>`.

- [ ] **Step 1: Add failing OpenAPI assertions**

Extend `academic_workspace_reads_are_documented` in `backend-school/src/api_contract.rs` with:

```rust
(
    "/api/academic/delivery/page-view",
    "getLearningDeliveryPageView",
    "#/components/schemas/ApiResponse_LearningDeliveryPageView",
),
```

Add `LearningDeliveryPageView` and `ApiResponse<LearningDeliveryPageView>` to the schema registration lists, register `get_learning_delivery_page_view` in the path list, and extend the query test:

```rust
assert_eq!(
    query_contract(&document, "/api/academic/delivery/page-view", "get"),
    BTreeSet::from([
        ("academicTermId".to_string(), true),
        ("academicYearId".to_string(), true),
        ("timetableVersionId".to_string(), false),
    ])
);
```

In `learning_delivery_handlers_are_thin_policy_owned_and_signal_after_mutation` in
`backend-school/tests/static_architecture.rs`, extract the new handler and require the read-only
list policy:

```rust
let page_view = extract_braced_block(
    &handlers,
    "pub async fn get_learning_delivery_page_view",
    false,
);
assert!(page_view.contains("require_learning_offering_list_access"));
assert!(page_view.contains("OfferingAction::Read"));
assert!(!page_view.contains("OfferingAction::Manage"));
```

- [ ] **Step 2: Run the API-contract test and confirm the red state**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::academic_workspace_reads_are_documented -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture learning_delivery_handlers_are_thin_policy_owned_and_signal_after_mutation -- --nocapture
```

Expected: FAIL because the handler and registered page-view schema do not exist.

- [ ] **Step 3: Add the authorized thin handler and router entry**

Add this handler beside the existing Delivery GET handlers:

```rust
#[utoipa::path(
    get,
    path = "/api/academic/delivery/page-view",
    operation_id = "getLearningDeliveryPageView",
    tag = "academic",
    params(HomeroomDeliveryQuery),
    responses(
        (status = 200, description = "Primary learning delivery page view", body = ApiResponse<LearningDeliveryPageView>),
        (status = 400, description = "Invalid academic year or term query", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found in the selected year", body = ApiErrorResponse)
    )
)]
pub async fn get_learning_delivery_page_view(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<HomeroomDeliveryQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(workspaces::delivery_page_view(
        &context.tenant.pool,
        query.academic_year_id,
        query.academic_term_id,
        query.timetable_version_id,
        &filter,
    )
    .await?))
}
```

Register it in `delivery.rs`:

```rust
.route(
    "/delivery/page-view",
    get(handlers::get_learning_delivery_page_view),
)
```

- [ ] **Step 4: Run the focused backend contract tests**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::academic_workspace_reads_are_documented -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::curriculum_alignment_and_clone_handoff_are_typed -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture learning_delivery_handlers_are_thin_policy_owned_and_signal_after_mutation -- --nocapture
```

Expected: both commands PASS.

- [ ] **Step 5: Regenerate and validate the API contract**

Run from `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: all three commands PASS; generated TypeScript contains `getLearningDeliveryPageView` and `LearningDeliveryPageView`.

- [ ] **Step 6: Commit the endpoint and generated artifacts**

```bash
git add backend-school/src/modules/academic/delivery/handlers.rs \
  backend-school/src/modules/academic/delivery.rs \
  backend-school/src/api_contract.rs \
  backend-school/tests/static_architecture.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat(api): expose delivery page view"
```

---

### Task 5: Add Reusable Route-Load Results, Contextual Menu Preload, and the Delivery API Wrapper

**Files:**
- Create: `frontend-school/src/lib/navigation/route-load.ts`
- Create: `frontend-school/tests/static/route-load-result.test.mjs`
- Modify: `frontend-school/src/lib/academic-context/route-context.ts`
- Modify: `frontend-school/src/lib/components/layout/Sidebar.svelte`
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`
- Create: `frontend-school/src/lib/academic/learning-delivery-page.ts`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`

**Interfaces:**
- Consumes: generated `LearningDeliveryPageView` and `getLearningDeliveryPageView` operation.
- Produces: `RouteLoadResult<T>`, `captureRouteLoad<T>()`, `academicContextualMenuPath()`, `LEARNING_DELIVERY_PAGE_DEPENDENCY`, `LearningDeliveryRefreshScope`, `readLearningDeliveryRouteContext()`, and `getLearningDeliveryPageView()`.

- [ ] **Step 1: Write failing route-helper and wrapper tests**

Create `route-load-result.test.mjs`:

```js
import assert from 'node:assert/strict';
import test from 'node:test';
import { captureRouteLoad } from '../../src/lib/navigation/route-load.ts';

test('captureRouteLoad preserves typed success data', async () => {
	assert.deepEqual(await captureRouteLoad(Promise.resolve({ id: 'one' }), 'โหลดไม่สำเร็จ'), {
		ok: true,
		data: { id: 'one' },
		error: null
	});
});

test('captureRouteLoad returns the concrete error message or fallback', async () => {
	assert.deepEqual(await captureRouteLoad(Promise.reject(new Error('เครือข่ายช้า')), 'โหลดไม่สำเร็จ'), {
		ok: false,
		data: null,
		error: 'เครือข่ายช้า'
	});
	assert.equal(
		(await captureRouteLoad(Promise.reject('failed'), 'โหลดไม่สำเร็จ')).error,
		'โหลดไม่สำเร็จ'
	);
});
```

Extend `learning-delivery-workspace.test.mjs`:

```js
assert.match(api, /Schemas\['LearningDeliveryPageView'\]/);
assert.match(api, /operations\['getLearningDeliveryPageView'\]/);
assert.match(api, /getLearningDeliveryPageView/);
assert.match(api, /requestFetch/);
```

Add this contract test to `academic-context-contract.test.mjs`:

```js
test('menu destinations carry only the academic context required by the target route', async () => {
	const { academicContextualMenuPath } = await importRouteContext();
	const selected = { academicYearId: 'year-active', academicTermId: 'term-active' };
	const requirement = (routeId) =>
		routeId.endsWith('/delivery')
			? 'term_required'
			: routeId.endsWith('/student-years')
				? 'year_required'
				: 'none';

	assert.equal(
		academicContextualMenuPath('/staff/academic/delivery', selected, requirement),
		'/staff/academic/delivery?academicYearId=year-active&academicTermId=term-active'
	);
	assert.equal(
		academicContextualMenuPath('/staff/academic/student-years', selected, requirement),
		'/staff/academic/student-years?academicYearId=year-active'
	);
	assert.equal(
		academicContextualMenuPath('/staff/work', selected, requirement),
		'/staff/work'
	);
});

test('sidebar preloads and navigates to the same context-bearing destination', async () => {
	const sidebar = await readProjectFile('src/lib/components/layout/Sidebar.svelte');
	assert.match(sidebar, /academicContextualMenuPath/);
	assert.match(sidebar, /href=\{menuHref\(item\)\}/);
	assert.match(sidebar, /preloadData\(resolve\(href/);
	assert.match(sidebar, /goto\(resolve\(href/);
	assert.match(sidebar, /onpointerenter=\{\(\) => preloadMenuItem\(item\)\}/);
	assert.match(sidebar, /onfocus=\{\(\) => preloadMenuItem\(item\)\}/);
	assert.match(sidebar, /ontouchstart=\{\(\) => preloadMenuItem\(item\)\}/);
});
```

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run:

```bash
node --test frontend-school/tests/static/route-load-result.test.mjs \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/static/academic-context-contract.test.mjs
```

Expected: FAIL because `route-load.ts`, the page-view wrapper, and the contextual menu helper are absent.

- [ ] **Step 3: Implement the typed route result**

Create `route-load.ts`:

```ts
export type RouteLoadResult<T> =
	| { ok: true; data: T; error: null }
	| { ok: false; data: null; error: string };

export async function captureRouteLoad<T>(
	operation: Promise<T>,
	fallbackMessage: string
): Promise<RouteLoadResult<T>> {
	try {
		return { ok: true, data: await operation, error: null };
	} catch (error) {
		return {
			ok: false,
			data: null,
			error: error instanceof Error && error.message ? error.message : fallbackMessage
		};
	}
}
```

- [ ] **Step 4: Implement Delivery route context and refresh ownership**

Create `learning-delivery-page.ts`:

```ts
export const LEARNING_DELIVERY_PAGE_DEPENDENCY = 'schoolorbit:learning-delivery-page';

export type LearningDeliveryRefreshScope = 'local' | 'page';

export type LearningDeliveryRouteContext = {
	academicYearId: string;
	academicTermId: string;
	timetableVersionId?: string;
};

export function readLearningDeliveryRouteContext(url: URL): LearningDeliveryRouteContext | null {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '';
	const academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
	const timetableVersionId = url.searchParams.get('timetableVersionId')?.trim() || undefined;
	if (!academicYearId || !academicTermId) return null;
	return { academicYearId, academicTermId, timetableVersionId };
}
```

Append these direct assertions to `route-load-result.test.mjs`:

```js
import { readLearningDeliveryRouteContext } from '../../src/lib/academic/learning-delivery-page.ts';

test('delivery route context requires year and term and preserves an optional version', () => {
	assert.equal(
		readLearningDeliveryRouteContext(new URL('https://school.test/staff/academic/delivery')),
		null
	);
	assert.deepEqual(
		readLearningDeliveryRouteContext(
			new URL(
				'https://school.test/staff/academic/delivery?academicYearId=year-1&academicTermId=term-1'
			)
		),
		{ academicYearId: 'year-1', academicTermId: 'term-1', timetableVersionId: undefined }
	);
	assert.deepEqual(
		readLearningDeliveryRouteContext(
			new URL(
				'https://school.test/staff/academic/delivery?academicYearId=year-1&academicTermId=term-1&timetableVersionId=version-1'
			)
		),
		{
			academicYearId: 'year-1',
			academicTermId: 'term-1',
			timetableVersionId: 'version-1'
		}
	);
});
```

- [ ] **Step 5: Preserve academic context in sidebar destinations and preload collapsed controls**

Change the existing `AcademicContextRouteResolver` type alias to `export type`, then add this pure
helper to `academic-context/route-context.ts`:

```ts
export function academicContextualMenuPath(
	path: string,
	selected: SelectedAcademicContext,
	resolveRequirement: AcademicContextRouteResolver = getAcademicContextRequirement
): string {
	const target = new URL(path, 'https://schoolorbit.invalid');
	const requirement = resolveRequirement(`/(app)${target.pathname}`);
	if (requirement === 'none') return path;

	target.searchParams.delete('academicYearId');
	target.searchParams.delete('academicTermId');
	if (selected.academicYearId) {
		target.searchParams.set('academicYearId', selected.academicYearId);
	}
	if (
		selected.academicYearId &&
		selected.academicTermId &&
		(requirement === 'term_required' || requirement === 'term_optional')
	) {
		target.searchParams.set('academicTermId', selected.academicTermId);
	}
	return `${target.pathname}${target.search}${target.hash}`;
}
```

In `Sidebar.svelte`, obtain the existing academic context store and make every expanded-menu link
use one `menuHref(item)` helper. For the collapsed dropdown, preload and navigate with the exact
same value:

```ts
import { goto, preloadData } from '$app/navigation';
import { getAcademicContextStore } from '$lib/academic-context/store';
import { academicContextualMenuPath } from '$lib/academic-context/route-context';

const academicContext = getAcademicContextStore();

function menuHref(item: SidebarMenuItem): string {
	return academicContextualMenuPath(item.path, $academicContext.selected);
}

function preloadMenuItem(item: SidebarMenuItem) {
	const href = menuHref(item);
	void preloadData(resolve(href as any)).catch(() => undefined);
}

function navigateToMenuItem(item: SidebarMenuItem) {
	handleNavClick();
	const href = menuHref(item);
	void goto(resolve(href as any));
}
```

Set the expanded `Button` to `href={menuHref(item)}`. Add `onpointerenter`, `onfocus`, and
`ontouchstart` handlers that call `preloadMenuItem(item)` to each collapsed `DropdownMenu.Item`.
Do not call `preloadData` for the expanded anchor because the body-level hover policy already owns it.

- [ ] **Step 6: Add the typed page-view wrapper**

In `learning-delivery.ts`, add:

```ts
export type LearningDeliveryPageView = Schemas['LearningDeliveryPageView'];
type LearningDeliveryPageViewQuery = NonNullable<
	operations['getLearningDeliveryPageView']['parameters']['query']
>;

export const getLearningDeliveryPageView = (
	academicYearId: string,
	academicTermId: string,
	options: HomeroomDeliveryRequestOptions = {}
) => {
	const yearId = academicYearId.trim();
	if (!yearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	const timetableVersionId = options.timetableVersionId?.trim();
	const { timetableVersionId: _selectedVersion, ...requestOptions } = options;
	const query = {
		academicYearId: yearId,
		academicTermId: selectedTerm(academicTermId),
		...(timetableVersionId ? { timetableVersionId } : {})
	} satisfies LearningDeliveryPageViewQuery;
	return deliveryData(
		apiClient.get<LearningDeliveryPageView>('/api/academic/delivery/page-view', {
			...requestOptions,
			query
		}),
		'โหลดหน้าจัดการการเปิดสอนไม่สำเร็จ'
	);
};
```

- [ ] **Step 7: Run the focused tests and frontend type checker**

Run:

```bash
node --test frontend-school/tests/static/route-load-result.test.mjs \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/static/academic-context-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm --prefix frontend-school run check
```

Expected: all commands PASS.

- [ ] **Step 8: Commit the route-data foundation**

```bash
git add frontend-school/src/lib/navigation/route-load.ts \
  frontend-school/src/lib/academic/learning-delivery-page.ts \
  frontend-school/src/lib/academic-context/route-context.ts \
  frontend-school/src/lib/api/learning-delivery.ts \
  frontend-school/src/lib/components/layout/Sidebar.svelte \
  frontend-school/tests/static/academic-context-contract.test.mjs \
  frontend-school/tests/static/route-load-result.test.mjs \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs
git commit -m "feat(frontend): add typed route load foundation"
```

---

### Task 6: Move Academic Delivery Primary Data into the Route Loader

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/AcademicChangeReadiness.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/TeacherHandoffPanel.svelte`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`
- Modify: `frontend-school/tests/static/academic-workspace-request-count.test.mjs`
- Create: `frontend-school/tests/static/route-data-loading-policy.test.mjs`

**Interfaces:**
- Consumes: `captureRouteLoad`, `getLearningDeliveryPageView`, `LEARNING_DELIVERY_PAGE_DEPENDENCY`, and `LearningDeliveryRefreshScope` from Task 5.
- Produces: `PageData.context`, `PageData.pageView`,
  `onChanged(changeSet, refreshScope?: LearningDeliveryRefreshScope)`, and a 71-route shrinking
  legacy baseline.

- [ ] **Step 1: Write failing route-ownership assertions**

Update the Delivery tests to read both `+page.ts` and `+page.svelte` and assert:

```js
assert.match(loader, /type PageLoad/);
assert.match(loader, /depends\(LEARNING_DELIVERY_PAGE_DEPENDENCY\)/);
assert.match(loader, /getLearningDeliveryPageView/);
assert.match(loader, /requestFetch:\s*fetch/);
assert.match(loader, /captureRouteLoad/);
assert.doesNotMatch(page, /getHomeroomDeliveryWorkspace|listAcademicTermChangeSets/);
assert.doesNotMatch(page, /getAcademicContextStore|\bonMount\b/);
assert.match(page, /invalidate\(LEARNING_DELIVERY_PAGE_DEPENDENCY\)/);
assert.doesNotMatch(page, /\binvalidateAll\s*\(/);
assert.match(page, /getLearningDeliveryOverview/);
assert.match(page, /viewMode === 'offerings'/);
assert.match(readiness, /onChanged\(updated,\s*'page'\)/);
```

Change `academic-workspace-request-count.test.mjs` so the Delivery case expects `getLearningDeliveryPageView` in `+page.ts`, rejects the two former primary wrapper names from `+page.svelte`, and still verifies the optional overview wrapper is not called per row.

Create `route-data-loading-policy.test.mjs` before changing the page:

```js
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const appRoutes = path.join(projectRoot, 'src/routes/(app)');

async function pageFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await pageFiles(fullPath)));
		else if (entry.name === '+page.svelte') files.push(fullPath);
	}
	return files;
}

async function sourceFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await sourceFiles(fullPath)));
		else if (/\.(?:svelte|ts)$/.test(entry.name)) files.push(fullPath);
	}
	return files;
}

test('legacy onMount primary reads only shrink during route migration', async () => {
	const violations = [];
	for (const file of await pageFiles(appRoutes)) {
		const source = await readFile(file, 'utf8');
		if (/\bonMount\b/.test(source) && /\$lib\/api\//.test(source)) {
			violations.push(path.relative(projectRoot, file));
		}
	}
	violations.sort();
	assert.equal(violations.length, 71, violations.join('\n'));
	assert.equal(
		createHash('sha256').update(violations.join('\n')).digest('hex'),
		'3f6cfffc91f19190281984451c57d0cc6a4780cdd63b0283c65cdbb174c12734',
		violations.join('\n')
	);
	assert.ok(
		!violations.includes('src/routes/(app)/staff/academic/delivery/+page.svelte'),
		'Delivery must keep its primary read in +page.ts'
	);
});

test('the application keeps safe hover data preload enabled', async () => {
	const app = await readFile(path.join(projectRoot, 'src/app.html'), 'utf8');
	assert.match(app, /<body[^>]*data-sveltekit-preload-data="hover"/);
});

test('route and component code uses focused invalidation', async () => {
	const allowedInvalidateAllOwners = new Set([]);
	const offenders = [];
	for (const file of await sourceFiles(path.join(projectRoot, 'src'))) {
		const relative = path.relative(projectRoot, file);
		if (
			/\binvalidateAll\s*\(/.test(await readFile(file, 'utf8')) &&
			!allowedInvalidateAllOwners.has(relative)
		) {
			offenders.push(relative);
		}
	}
	assert.deepEqual(offenders.sort(), []);
});
```

For the Delivery case in `academic-workspace-request-count.test.mjs`, read both route files and do
not call `assertCancellable`, because cancellation belongs to SvelteKit after the migration:

```js
const page = await readPage('delivery');
const loader = await readFile(
	path.join(projectRoot, academicRoutes, 'delivery', '+page.ts'),
	'utf8'
);
assert.match(loader, /getLearningDeliveryPageView/);
assert.doesNotMatch(page, /getHomeroomDeliveryWorkspace|listAcademicTermChangeSets|workspaceRequest/);
assert.match(page, /overviewRequest\s*=\s*new LatestRequest/);
assert.match(page, /getLearningDeliveryOverview/);
```

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run:

```bash
node --test frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/static/academic-workspace-request-count.test.mjs \
  frontend-school/tests/static/route-data-loading-policy.test.mjs
```

Expected: FAIL because primary reads still start from `onMount`; the baseline reports 72 routes
instead of the post-migration expectation of 71.

- [ ] **Step 3: Implement the route loader**

Replace the metadata-only loader in `+page.ts` with:

```ts
import type { PageLoad } from './$types';
import {
	LEARNING_DELIVERY_PAGE_DEPENDENCY,
	readLearningDeliveryRouteContext
} from '$lib/academic/learning-delivery-page';
import { getLearningDeliveryPageView } from '$lib/api/learning-delivery';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required',
	menu: {
		title: 'รายวิชาและกิจกรรมที่เปิดสอน',
		icon: 'Workflow',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load: PageLoad = async ({ depends, fetch, url }) => {
	depends(LEARNING_DELIVERY_PAGE_DEPENDENCY);
	const context = readLearningDeliveryRouteContext(url);
	return {
		title: _meta.menu.title,
		context,
		pageView: context
			? await captureRouteLoad(
					getLearningDeliveryPageView(context.academicYearId, context.academicTermId, {
						timetableVersionId: context.timetableVersionId,
						requestFetch: fetch
					}),
					'โหลดหน้าจัดการการเปิดสอนไม่สำเร็จ'
				)
			: null
	};
};
```

- [ ] **Step 4: Hydrate the page from loader data and retain only optional reads**

In `+page.svelte`:

- receive typed `data` with `$props()`;
- derive year and term IDs from `data.context`;
- remove `getAcademicContextStore`, `workspaceRequest`, `loadWorkspace`, and the subscription in `onMount`;
- preserve `overviewRequest` and `getLearningDeliveryOverview` for the offerings tab and editor-only fallback;
- apply `data.pageView.data.workspace`, `.changeSets`, and `.overview` in a synchronous `$effect` whenever loader data changes;
- abort the optional `overviewRequest` when that effect is replaced or the page unmounts;
- render `PageState` from `data.pageView.error` when the route result fails; and
- retry with `invalidate(LEARNING_DELIVERY_PAGE_DEPENDENCY)`.

The loader-data application must keep the existing active change-set selection rule:

```ts
function applyPageView(loaded: LearningDeliveryPageView) {
	workspace = loaded.workspace;
	changeSets = loaded.changeSets;
	overview = loaded.overview;
	const requestedId = page.url.searchParams.get('changeSetId')?.trim() ?? '';
	selectedChangeSetId =
		loaded.changeSets.find((item) => item.id === requestedId)?.id ??
		loaded.changeSets.find((item) => item.status === 'draft')?.id ??
		loaded.changeSets[0]?.id ??
		'';
}
```

Pass an `ensureOfferings: () => Promise<void>` prop to `AcademicChangeSetPanel`. In its
`showItemForm`, request `loadManagementOptions()` and `ensureOfferings()` together after the manage
permission and draft-status checks. This keeps stop/adjust choices correct for an empty draft while
leaving editor-only reads out of the page view:

```ts
onChanged: (
	changeSet: AcademicTermChangeSet,
	refreshScope?: LearningDeliveryRefreshScope
) => void | Promise<void>;
ensureOfferings: () => Promise<void>;

async function ensureOverview() {
	if (!academicTermId || overview || overviewLoading) return;
	await loadOverview(academicTermId);
}

async function showItemForm() {
	if (!canManage || changeSet.status !== 'draft') return;
	teacherFormOpen = false;
	handoffItemId = '';
	itemFormOpen = true;
	await Promise.all([loadManagementOptions(), ensureOfferings()]);
}
```

- [ ] **Step 5: Replace broad post-mutation reloads with local patching or named invalidation**

Use this callback contract in the page and all three child components:

```ts
onChanged: (
	changeSet: AcademicTermChangeSet,
	refreshScope?: LearningDeliveryRefreshScope
) => void | Promise<void>;
```

Implement the parent callback as:

```ts
async function updateChangeSet(
	updated: AcademicTermChangeSet,
	refreshScope: LearningDeliveryRefreshScope = 'local'
) {
	selectedChangeSetId = updated.id;
	changeSets = changeSets
		.map((changeSet) => (changeSet.id === updated.id ? updated : changeSet))
		.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
	if (updated.items.length > 0 && !overview && academicTermId) {
		await loadOverview(academicTermId);
	}
	if (refreshScope === 'page') {
		await invalidate(LEARNING_DELIVERY_PAGE_DEPENDENCY);
	}
}
```

`AcademicChangeReadiness.publishChangeSet` calls `onChanged(updated, 'page')`. Conflict recovery,
draft edits, and cancellation call the default local scope. `AcademicChangeSetPanel.handoffApplied`
requests page scope after timetable entries are applied.

Use one helper for broad Delivery consequences:

```ts
async function refreshDeliveryPage(refreshOverview = viewMode === 'offerings') {
	await invalidate(LEARNING_DELIVERY_PAGE_DEPENDENCY);
	if (refreshOverview && academicTermId) await loadOverview(academicTermId);
}
```

Replace the former broad reload call sites with this mapping:

| Call site | Reconciliation |
|---|---|
| `addCreated` | patch the returned overview item, then `void refreshDeliveryPage(true)` because the homeroom workspace may change |
| curriculum `onApplied` | `refreshDeliveryPage(true)` |
| `includeOfferingInTimetable` | after the mutation, `refreshDeliveryPage()` |
| `handleTimetableRevisionCreated` | update the URL with `replaceState`, then `refreshDeliveryPage()` before opening the pending workflow |
| published change set | `onChanged(updated, 'page')` |
| teacher handoff apply | call page-scoped `onChanged` from `handoffApplied` |
| draft edit, cancellation, or conflict reload | patch the returned change set locally |

Remove `loadWorkspace` and `reloadAfterApply` completely. Do not use `invalidateAll()`.

- [ ] **Step 6: Run the focused static tests and Svelte checker**

Run:

```bash
node --test frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/static/academic-workspace-request-count.test.mjs \
  frontend-school/tests/static/route-load-result.test.mjs \
  frontend-school/tests/static/route-data-loading-policy.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm --prefix frontend-school run check
```

Expected: all commands PASS with no Svelte or TypeScript errors.

- [ ] **Step 7: Commit the route migration**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte' \
  frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte \
  frontend-school/src/lib/components/learning-delivery/AcademicChangeReadiness.svelte \
  frontend-school/src/lib/components/learning-delivery/TeacherHandoffPanel.svelte \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/static/academic-workspace-request-count.test.mjs \
  frontend-school/tests/static/route-data-loading-policy.test.mjs
git commit -m "perf(frontend): preload delivery route data"
```

---

### Task 7: Verify the Browser Request Shape

**Files:**
- Modify: `frontend-school/tests/e2e/homeroom-delivery-workspace.spec.ts`

**Interfaces:**
- Consumes: the migrated Delivery route and `/api/academic/delivery/page-view`.
- Produces: browser evidence that Delivery performs one primary request, starts the next contextual
  page view on menu hover, reuses that preload on click, and keeps its optional overview lazy.

- [ ] **Step 1: Update the mocked browser endpoint and request counters**

In `homeroom-delivery-workspace.spec.ts`, replace the two initial endpoint mocks with one page-view response:

Move the existing inline homeroom response into `homeroomWorkspace()` without changing its fields,
and add `version` to the `ids` fixture. Return a menu containing the current Delivery route and a
second Delivery link whose `timetableVersionId` is `ids.version`:

```ts
if (url.pathname === '/api/menu/user') {
	await fulfill(route, {
		groups: [
			{
				code: 'academic_delivery',
				displayOrder: 1,
				icon: 'Workflow',
				name: 'การจัดการเรียนการสอน',
				workspaceCode: 'academic',
				workspaceIcon: 'GraduationCap',
				workspaceName: 'วิชาการ',
				workspaceOrder: 1,
				items: [
					{
						id: 'a0000000-0000-4000-8000-000000000001',
						code: 'delivery',
						name: 'การเปิดสอน',
						icon: 'Workflow',
						path: '/staff/academic/delivery'
					},
					{
						id: 'a0000000-0000-4000-8000-000000000002',
						code: 'delivery-version',
						name: 'การเปิดสอนรุ่นถัดไป',
						icon: 'Workflow',
						path: `/staff/academic/delivery?timetableVersionId=${ids.version}`
					}
				]
			}
		]
	});
	return;
}
```

Then use these counters for the page-view and retired endpoints:

```ts
let pageViewRequests = 0;
let legacyPrimaryRequests = 0;
let offeringOverviewRequests = 0;

if (url.pathname === '/api/academic/delivery/page-view') {
	pageViewRequests += 1;
	expect(url.searchParams.get('academicYearId')).toBe(ids.year);
	expect(url.searchParams.get('academicTermId')).toBe(ids.term);
	await fulfill(route, {
		workspace: homeroomWorkspace(),
		changeSets: [],
		overview: null
	});
	return;
}
if (
	url.pathname === '/api/academic/delivery/homerooms' ||
	url.pathname === '/api/academic/term-change-sets'
) {
	legacyPrimaryRequests += 1;
}
```

Return all three counters from `mockDelivery`. Assert after navigation:

```ts
expect(pageViewRequestCount()).toBe(1);
expect(legacyPrimaryRequestCount()).toBe(0);
expect(overviewRequestCount()).toBe(0);
```

Hover the `การเปิดสอนรุ่นถัดไป` link and assert `pageViewRequestCount()` becomes 2 before clicking.
Click it, wait for the URL to contain `timetableVersionId=${ids.version}`, and assert the count
remains 2, proving that click reused the preload. Then select the offerings tab and assert
`overviewRequestCount()` becomes 1 while the page-view count remains 2.

- [ ] **Step 2: Run the focused static and browser tests**

Run the static test:

```bash
node --test frontend-school/tests/static/route-data-loading-policy.test.mjs
```

Build from the repository root, then start a local preview from `frontend-school` in terminal A:

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm --prefix frontend-school run build
cd frontend-school
npm run preview -- --host 127.0.0.1
```

With terminal A still running, execute in terminal B from `frontend-school`:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/homeroom-delivery-workspace.spec.ts --project=chromium
```

Expected: static test PASS; Playwright PASS with one initial page-view request, a second request on
hover, no third request on click, zero legacy primary requests, and one overview request only after
the tab click.

- [ ] **Step 3: Commit the browser regression guard**

```bash
git add frontend-school/tests/e2e/homeroom-delivery-workspace.spec.ts
git commit -m "test(frontend): guard route data request shape"
```

---

### Task 8: Run the Exact-Tree Verification Matrix

**Files:**
- Review: every file changed by Tasks 1–7.

**Interfaces:**
- Consumes: the complete wave-1 candidate tree.
- Produces: verification evidence for the exact tree; no code changes unless a check exposes a defect.

- [ ] **Step 1: Run focused frontend tests**

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs \
  frontend-school/tests/static/api-query-contract.test.mjs \
  frontend-school/tests/static/route-load-result.test.mjs \
  frontend-school/tests/static/route-data-loading-policy.test.mjs \
  frontend-school/tests/static/academic-context-contract.test.mjs \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/static/academic-workspace-request-count.test.mjs
```

Expected: PASS with 0 failures.

- [ ] **Step 2: Run focused backend tests**

```bash
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-delivery page_view_requires_overview_only_for_visible_offering_labels
./scripts/test_backend_school.sh modules::academic::delivery::services_tests::delivery_page_view_batches_primary_reads_without_forcing_overview -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests -- --nocapture
```

Expected: PASS with 0 failures. The database command must use its disposable Podman PostgreSQL instance.

- [ ] **Step 3: Run API generation and frontend matrix checks**

From `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build
```

Expected: all commands PASS. If generation changes tracked files, inspect them, commit them with the owning contract task, and rerun the affected checks.

- [ ] **Step 4: Run the backend matrix**

From `backend-school`:

```bash
cargo fmt --all -- --check
cargo test -p school-academic-delivery -- --test-threads=8
cargo test --test static_architecture
cargo check --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
```

Expected: all commands PASS.

- [ ] **Step 5: Run the local browser request-shape test**

From `frontend-school`, start the verified build with
`npm run preview -- --host 127.0.0.1`. In a second terminal, also from `frontend-school`, run:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/homeroom-delivery-workspace.spec.ts --project=chromium
```

Expected: PASS. No live credentials are required because the test intercepts authenticated API traffic.

- [ ] **Step 6: Review the final diff and repository state**

From the repository root:

```bash
git diff --check
git status --short
git diff origin/main...HEAD --stat
git diff origin/main...HEAD
```

Expected: no whitespace errors; only the approved route-loading foundation, Delivery reference implementation, generated API contract, tests, `.rules`, spec, and plan are changed.

- [ ] **Step 7: Resolve any verification finding at its owning task**

If a check exposes a defect, return to the task that owns that interface, add or tighten its failing
test, apply the smallest fix, use that task's explicit `git add` list and commit boundary, and rerun
Steps 1–6. If no tracked fix is required, do not create an empty commit.

---

## Subsequent Plan Set

After this plan passes on the exact candidate tree, create and execute these separate implementation plans against the proven interfaces:

1. `academic-route-data-loading` — migrate the remaining authenticated academic menu routes and shrink the legacy baseline after each bounded group.
2. `staff-route-data-loading` — migrate work, people, organization, facilities, certificates, and settings routes.
3. `student-parent-route-data-loading` — migrate authenticated student and parent menu routes.
4. `route-data-loading-acceptance` — remove the legacy baseline, run cross-domain request-shape coverage, and collect controlled deployed before/after timings.

The overall design is complete only after the final plan reports zero authenticated menu pages with primary API reads owned by route `onMount`.
