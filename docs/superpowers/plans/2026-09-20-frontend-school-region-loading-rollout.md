# Frontend School Region-Owned Route Loading Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate every authenticated `frontend-school` route to an explicit, measurable data-loading contract so visible independent regions render as soon as they are ready, optional work stays lazy, mutations refresh only affected state, and measured backend bottlenecks use efficient bounded SQL.

**Architecture:** Academic Delivery is the reference implementation. Each route is classified as component-owned legacy primary data, route-owned primary data, shared-layout data, interaction-only data, or no-data; visible independent reads start from the narrowest SvelteKit loader as typed in-flight region results, while dependent reads wait only for their required identifier. Backend work remains in cohesive domain endpoints, never route-wide page-view endpoints, and SQL changes require phase timing plus representative query-plan evidence.

**Tech Stack:** SvelteKit 2/Svelte 5, TypeScript, typed OpenAPI clients, Node test runner, Playwright, Rust/Axum/SQLx, PostgreSQL, Podman-backed database tests.

**Spec:** [`.rules`](../../../.rules), especially “Required Analysis Workflow”, “Backend: Rust, Axum, SQLx”, “Frontend: SvelteKit 5 → Route data loading and navigation performance”, and “Verification Matrix”. The approved design rationale is retained in Git history at `d088ac86:docs/superpowers/specs/2026-09-19-region-owned-route-data-design.md`.

## Global Constraints

- This is a rollout program, not one long-lived implementation branch. Execute each wave on a fresh branch from the latest `origin/main`, verify it independently, squash it into `main`, and remove its temporary workflow artifacts after acceptance.
- Scope is all 104 authenticated pages under `frontend-school/src/routes/(app)`. Public routes and `frontend-admin` are outside this program unless a shared change is strictly required.
- Audit all 104 pages, including pages without a direct `$lib/api/` import; child components, layouts, stores, and re-exports may own hidden primary reads.
- Treat the existing 71 `onMount` plus API pages as a migration baseline, not as the complete route inventory and not as proof that every `onMount` call is wrong.
- Preserve business behavior, permissions, tenant isolation, URL/deep-link semantics, accessibility, mutation outcomes, realtime reconciliation, and generated API contracts.
- Primary reads belong to `+page.ts` or the narrowest applicable `+layout.ts`. `onMount` remains valid only for browser-only listeners, observers, and interaction-only work.
- Start visible independent region reads concurrently and expose typed in-flight results. Sequential reads require an exact data dependency.
- Keep inactive tabs, dialogs, editors, exports, expanded details, and action-only data lazy and behind their exact permission.
- Do not create `/page-view`, generic batch, route-proxy, or generic client-cache abstractions.
- Reuse an existing cohesive endpoint when it already returns the bounded data one region needs. Add or split an endpoint only when UI ownership, authorization, freshness, or measured backend work requires it.
- Do not optimize for HTTP request count alone. Optimize time to first useful region, total transferred bytes, backend time, database work, pool pressure, and correctness together.
- SQL changes must select only used fields, eliminate N+1 work, normalize/deduplicate identifiers, preserve bounds, and minimize measured round trips without monopolizing the five-connection tenant pool.
- Do not add an index without representative `EXPLAIN (ANALYZE, BUFFERS)` evidence. Add every justified index through a new sequential migration; never edit an applied migration.
- Never store or log plaintext national IDs. Performance evidence must not contain PII, credentials, raw tenant rows, database URLs, or SQL parameter values.
- Rust DTOs and OpenAPI annotations own wire contracts. Regenerate tracked OpenAPI and TypeScript clients in the same change; never edit generated contracts directly.
- Use TDD for every route group: add request-shape/state tests first, observe the focused failure, make the smallest coherent change, then run the group and matrix checks.
- A route is not complete merely because it has a `+page.ts`. It is complete only when its primary ownership, skeleton/error/retry state, cancellation, context transitions, mutation reconciliation, and optional-data policy are verified.

## Current Baseline

The source audit on 2026-09-20 found:

- 104 authenticated `+page.svelte` files;
- 81 pages with direct frontend API imports;
- 71 pages containing both `onMount` and a direct frontend API import;
- one route loader with a direct frontend API import: Academic Delivery; and
- Academic Delivery already implements the reference split: `/homerooms`, change-set summaries plus selected detail, and lazy `/workspace`.

Recompute these numbers at the start of execution. They are planning evidence, not permanent target counts.

## Completion Definition

The program is complete only when:

1. every authenticated page is represented in the executable inventory;
2. every primary read is owned by a route/layout loader or documented as shared-layout data;
3. every component-owned API read is proven interaction-only, browser-only, or mutation-related;
4. visible independent regions do not have a serial request waterfall;
5. optional/action-only reads remain lazy and permission-gated;
6. route-context changes never paint stale or false-prerequisite data;
7. each region has stable loading, error, retry, supersession, and mutation behavior;
8. measured backend bottlenecks have bounded SQL evidence or an explicit no-change conclusion;
9. the temporary legacy baseline is zero and removed; and
10. focused, contract, frontend, backend, browser, diff, and status checks pass on the exact final tree.

## Review Focus

- **Context transitions:** Direct links, missing/stale academic context, year/term switches, back/forward, and preloaded navigation must not show data from the previous context.
- **Permissions:** A read-only user must render the readable region without eager create/update/approve/export requests; denied optional actions must not make the page fail.
- **Supersession:** Slow old requests must be aborted or ignored and must never replace newer route/selection data.
- **Mutation and realtime:** Returned typed resources should patch their owning region; otherwise only its named dependency is invalidated, and missed realtime events reconcile through authoritative HTTP data.
- **Database capacity:** Parallel query work must remain bounded within the five-connection tenant pool; a lower query count or higher concurrency is not accepted without better measured critical-path behavior.

---

### Task 1: Establish the Executable Route Inventory

**Files:**

- Create: `frontend-school/tests/fixtures/route-data-loading-inventory.json`
- Create: `frontend-school/tests/static/route-data-loading-inventory.test.mjs`
- Modify: `frontend-school/tests/static/route-data-loading-policy.test.mjs`
- Modify: `frontend-school/package.json`

**Interfaces:**

- Consumes: all `src/routes/(app)/**/+page.svelte`, adjacent loaders/layouts, child component imports, and the current 71-page legacy baseline.
- Produces: one reviewed record per authenticated page and a failing guard whenever an unclassified route appears, a route disappears without inventory reconciliation, or a completed route regresses to component-owned primary reads.

- [ ] **Step 1: Write the failing inventory coverage test**

Create a test that recursively discovers authenticated `+page.svelte` files, loads the JSON inventory, and compares exact sorted route sets. Use this schema:

```json
{
  "version": 1,
  "routes": [
    {
      "route": "staff/academic/delivery",
      "wave": "reference",
      "dataOwner": "route",
      "preload": "hover",
      "context": "term_required",
      "status": "complete",
      "backendOwner": "school-academic-delivery",
      "notes": "Visible homerooms and change-set regions; offerings workspace stays lazy"
    }
  ]
}
```

Validate exact enum values:

```js
const dataOwners = new Set([
  "component-primary",
  "route",
  "shared-layout",
  "interaction-only",
  "no-data",
]);
const preloads = new Set(["hover", "tap", "none"]);
const statuses = new Set(["audit", "planned", "complete"]);
```

The test must also require non-empty `wave`, `context`, `backendOwner`, and `notes`, reject duplicate routes, and require `status: "complete"` when `dataOwner` is `no-data`.

- [ ] **Step 2: Run the test and confirm the red state**

Run:

```bash
node --test frontend-school/tests/static/route-data-loading-inventory.test.mjs
```

Expected: FAIL because the inventory file does not yet exist.

- [ ] **Step 3: Audit and classify all 104 pages**

For every page, inspect the page, adjacent `+page.ts`/`+page.server.ts`, nearest layouts, API wrappers, imported child components, stores, mutations, and backend handler/service owners. Classify:

- `component-primary`: default visible data still starts from the page or an imported child and must migrate;
- `route`: default visible data starts from the route loader;
- `shared-layout`: the narrowest parent layout already owns the data for multiple child routes;
- `interaction-only`: the page has no default visible API read and calls the API only after a user action; or
- `no-data`: the page renders from local/static/shared state and performs no page-owned read.

Set `status: "complete"` only for Academic Delivery and routes whose current implementation already satisfies every completion condition. Set audited migration candidates to `planned`; use `audit` only while the same branch is actively inspecting a record and leave no `audit` entry at the task commit.

- [ ] **Step 4: Replace the hash-only baseline with inventory-backed assertions**

Keep the existing policy that legacy component-owned primary reads may only shrink, but derive the candidate set from inventory records whose status is not `complete`. The test must continue rejecting `invalidateAll()` and `/page-view`, and must explicitly assert that Academic Delivery remains `complete` and route-owned.

Do not attempt to ban all `onMount` or all API imports. The guard should report a route for review when a non-complete page combines lifecycle startup with API access, while completed pages must have no component-owned primary startup read.

- [ ] **Step 5: Add the focused inventory command**

Add:

```json
"test:route-loading": "node --test tests/static/route-data-loading-inventory.test.mjs tests/static/route-data-loading-policy.test.mjs"
```

to `frontend-school/package.json`.

- [ ] **Step 6: Run focused and documentation-policy tests**

Run:

```bash
npm --prefix frontend-school run test:route-loading
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: PASS with every discovered route inventoried and no unreviewed inventory entries. The baseline tree has 104 routes; newly discovered routes must be audited instead of forcing the old count.

- [ ] **Step 7: Commit the inventory foundation**

```bash
git add frontend-school/package.json \
  frontend-school/tests/fixtures/route-data-loading-inventory.json \
  frontend-school/tests/static/route-data-loading-inventory.test.mjs \
  frontend-school/tests/static/route-data-loading-policy.test.mjs
git commit -m "test(frontend): inventory authenticated route data ownership"
```

---

### Task 2: Add Repeatable Navigation and Backend Measurement Gates

**Files:**

- Create: `frontend-school/tests/e2e/route-region-loading.spec.ts`
- Create: `frontend-school/tests/e2e/helpers/route-performance.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Modify when a backend owner lacks phase timing: that owner's Rust handler/service and focused tests

**Interfaces:**

- Consumes: route inventory records, SvelteKit navigation, browser Performance APIs, response headers, and existing `Server-Timing` conventions.
- Produces: reusable assertions for request start order, first-region rendering, lazy requests, stale-result rejection, and sanitized median measurements from five warm runs.

- [ ] **Step 1: Write a failing mocked reference test around Academic Delivery**

The test must delay `/homerooms`, resolve change-set summaries first, and assert the change-set region becomes usable without waiting for homerooms. Repeat with the delays reversed. It must also assert:

```ts
expect(requests.workspace).toBe(0); // before opening the offerings tab
expect(requests.pageView).toBe(0);
```

Then open the offerings tab and require exactly one workspace read.

- [ ] **Step 2: Add reusable request and paint helpers**

The helper must record, per route and region:

```ts
export type RouteRegionSample = {
  route: string;
  region: string;
  navigationStartedAt: number;
  requestStartedAt: number;
  firstUsefulPaintAt: number;
  responseBytes: number | null;
  serverTiming: Record<string, number>;
};
```

Use stable `data-testid` region-ready markers in migrated pages. Keep raw URLs, query values, headers, response bodies, and tenant identifiers out of recorded output.

- [ ] **Step 3: Run the reference test and confirm it fails before helper wiring**

```bash
cd frontend-school
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/route-region-loading.spec.ts --project=chromium
```

Expected: FAIL until the helper and region-ready markers are connected.

- [ ] **Step 4: Implement sanitized median reporting**

Run five warm navigations per representative route in the same environment and connection profile. Report medians for navigation-to-request, request duration, first useful region, transferred bytes, decoded bytes when available, and named server phases. Do not commit production measurement output; attach it to the review or release evidence.

- [ ] **Step 5: Add backend phase timing only at measured owners**

If an endpoint lacks enough timing to locate the bottleneck, add bounded phase timing such as `context`, `resources`, `assembly`, and `total`. Do not expose SQL text, identifiers, parameter values, or row data. Add a focused Rust test that validates the timing structure without asserting unstable millisecond values.

- [ ] **Step 6: Run focused browser and static tests**

```bash
npm --prefix frontend-school run build
npm --prefix frontend-school run test:route-loading
cd frontend-school
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/route-region-loading.spec.ts --project=chromium
```

Expected: PASS. A missing live credential may skip only deployed measurements, not the mocked request-shape test.

- [ ] **Step 7: Commit the measurement foundation**

```bash
git add frontend-school/tests/e2e/route-region-loading.spec.ts \
  frontend-school/tests/e2e/helpers/route-performance.ts \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte'
git commit -m "test(frontend): measure region-owned route loading"
```

If Rust timing changed, include its owner and focused test in a separate preceding commit.

---

### Task 3: Roll Out Academic Foundation and Catalog Routes

**Files:**

- Create/modify each route's `+page.ts` and modify its `+page.svelte` under:
  - `staff/academic/core`
  - `staff/academic/catalog/subject-groups`
  - `staff/academic/catalog/subjects`
  - `staff/academic/catalog/activities`
  - `staff/academic/curricula`
  - `staff/academic/curricula/[id]`
  - `staff/academic/homerooms`
  - `staff/academic/student-years`
- Modify: the exact feature API wrapper, Rust domain service/handler, OpenAPI registration, generated contracts, and focused tests only when the route audit proves they are needed
- Modify: `frontend-school/tests/fixtures/route-data-loading-inventory.json`
- Modify/create: focused static and Playwright tests owned by these routes

**Interfaces:**

- Consumes: load-aware API wrappers, `captureRouteLoad`, `LatestRequest` for remaining interaction-only reads, shared page-state components, academic context in the URL, and existing bounded workspace endpoints.
- Produces: route-owned visible regions for academic structure/catalog pages with no per-row reads, no stale year/term paint, and no eager editor/action data.

- [ ] **Step 1: Create one fresh feature branch per coherent route group**

Use these reviewable groups in order:

1. catalog lists: subject groups, subjects, activities;
2. academic setup and curriculum list;
3. curriculum detail workspace;
4. homerooms and student years.

Do not combine all eight routes in one branch.

- [ ] **Step 2: Capture before evidence and write failing request-shape tests**

For each group, record the initial visible regions, exact current API requests, dependencies, optional workflows, payload sizes, and server phases. Add tests that fail until primary reads move to `+page.ts`, independent reads start without serial awaits, and optional editors/dialogs remain unrequested.

Preserve existing batch boundaries already guarded by `academic-workspace-request-count.test.mjs`, including one curriculum structure workspace, one academic-year advisor/placement collection, and no per-room/per-program traversal.

- [ ] **Step 3: Move primary reads into typed route loaders**

Use the existing Academic Delivery loader as the concrete scheduling pattern. It starts both visible independent reads immediately and makes only selected detail dependent on the summary when the URL does not supply an ID:

```ts
const homerooms = captureRouteLoad(
  getHomeroomDeliveryWorkspace(context.academicYearId, context.academicTermId, {
    timetableVersionId: context.timetableVersionId,
    requestFetch: fetch,
  }),
  "โหลดภาพรวมรายห้องประจำชั้นไม่สำเร็จ",
);
const changeSetSummaries = captureRouteLoad(
  listAcademicTermChangeSets(context.academicTermId, { requestFetch: fetch }),
  "โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ",
);
const selectedChangeSet = context.changeSetId
  ? loadSelectedChangeSet(context.changeSetId)
  : changeSetSummaries.then((result) => {
      if (!result.ok) {
        return {
          ok: true,
          data: null,
          error: null,
        } satisfies RouteLoadResult<null>;
      }
      const selected = selectAcademicTermChangeSetSummary(result.data);
      return selected
        ? loadSelectedChangeSet(selected.id)
        : ({
            ok: true,
            data: null,
            error: null,
          } satisfies RouteLoadResult<null>);
    });
```

Each group-specific child plan must name its actual context parser, dependency constants, typed API wrappers, region result keys, and default-selection helper before implementation. Return independent promises without serial `await`. Await only a required identifier such as a selected version or row ID that cannot be resolved from the URL.

- [ ] **Step 4: Give each region first-paint and refresh behavior**

When no usable data exists, synchronously enter the focused skeleton before first paint. On background refresh, keep usable data visible with `RegionUpdatingState` and `aria-busy`. Clear data from a different year, term, curriculum, or selection before the new context is painted.

- [ ] **Step 5: Reconcile mutations locally**

Patch typed returned rows/options/workspaces when the response is authoritative. Otherwise invalidate only the named region dependency. Keep management options and editor data lazy and permission-gated. Do not introduce `invalidateAll()`.

- [ ] **Step 6: Profile backend work and optimize only measured bottlenecks**

For a slow cohesive endpoint, inspect the service and representative plan. Eliminate per-row reads with bounded `ANY($1)`, joins, filtered aggregates, or pre-aggregated CTEs as supported by evidence. Start genuinely independent reads concurrently only when the pool cost is justified. Add an index only through a new migration with before/after plan evidence.

- [ ] **Step 7: Regenerate contracts when an API shape changes**

```bash
npm --prefix frontend-school run generate:api-contracts
npm --prefix frontend-school run check:api-contracts
npm --prefix frontend-school run test:api-contracts
```

Expected: generated output matches Rust DTO/OpenAPI ownership and contains no handwritten edits.

- [ ] **Step 8: Verify and commit each group**

Run the group-specific static/browser tests, `npm --prefix frontend-school run test:route-loading`, Svelte checks, the owning Rust package tests when changed, then the applicable matrix in Task 9. Mark only verified routes `complete` in the inventory and commit the route group with its tests and contract changes.

---

### Task 4: Roll Out Teaching, Delivery Detail, Timetable, Exams, and Assessment Routes

**Files:**

- Modify route-owned files under:
  - `staff/academic/delivery/[offeringId]`
  - `staff/academic/timetable`
  - `staff/academic/timetable/templates`
  - `staff/academic/timetable/today`
  - `staff/academic/exam-schedules`
  - `staff/academic/exam-schedules/[id]`
  - `staff/academic/assessments`
  - `staff/academic/question-bank`
  - `staff/exams`
  - `staff/timetable`
- Verify without regressing: `staff/academic/delivery`
- Modify the owning `school-academic-delivery`, `school-academic-timetable`, `school-academic-assessment`, question-bank, exam-schedule, and API-contract files only when measured evidence requires it
- Modify inventory and focused tests

**Interfaces:**

- Consumes: Task 1 inventory, Task 2 measurement helpers, Academic Delivery's region lifecycle, timetable version context, and exact action permissions.
- Produces: preloadable teaching/scheduling primary views while drag/drop boards, selected offering detail, editors, generation actions, exports, and document/file work remain lazy.

- [ ] **Step 1: Split execution into five coherent branches**

1. delivery offering detail;
2. personal/today timetables;
3. timetable board and templates;
4. exam rounds and exam schedule detail;
5. assessments and question bank.

- [ ] **Step 2: Test route context and independent region scheduling first**

Cover year/term/version IDs, direct detail URLs, hover/tap preload policy, missing/stale context fallback, delayed siblings, failed siblings, retries, back/forward, and aborted navigation. For heavy drag/drop or volatile boards, select `tap` instead of `hover` in the inventory and assert the policy.

- [ ] **Step 3: Migrate default visible reads to loaders**

Keep only the minimal visible board/list/summary in loader-owned regions. Selected detail may start concurrently when its ID is in the URL; otherwise wait only for the list that chooses the default ID. Do not load editor options, generation inputs, export libraries, question attachments, or timetable mutation resources until opened.

- [ ] **Step 4: Preserve interaction state and cancellation**

Use route navigation for deep-linkable selections and `LatestRequest` for in-page volatile selections that remain interaction-only. A superseded timetable version, offering, exam round, or assessment phase must not overwrite the latest state.

- [ ] **Step 5: Optimize measured backend phases**

Check for per-block, per-group, per-teacher, per-room, per-question, and per-assessment reads. Batch bounded identifiers and aggregate repeated counts. Keep timetable consistency fields in one cohesive region or transaction-backed read when they require the same snapshot.

- [ ] **Step 6: Verify lazy boundaries**

Browser tests must prove that exports, generators, editors, upload/download helpers, and action-only options are absent before the user action and requested/imported exactly once when opened.

- [ ] **Step 7: Regenerate contracts, run focused checks, and complete inventory records**

Use the same contract, frontend, backend-owner, browser, diff, and status gates as Task 3. Commit each branch independently.

---

### Task 5: Roll Out Results, Gradebook, Promotion, and Academic Lifecycle Routes

**Files:**

- Modify route-owned files under:
  - `staff/academic/gradebook`
  - `staff/academic/results`
  - `staff/academic/result-locks`
  - `staff/academic/result-corrections`
  - `staff/academic/results/aggregates`
  - `staff/academic/results/annual`
  - `staff/academic/promotion`
  - `staff/academic/promotion/[id]`
  - `staff/academic/promotion/policies`
  - `staff/academic/term-lifecycle`
  - `staff/academic/year-lifecycle`
- Modify the owning assessment/results/lifecycle services, typed API contracts, inventory, and focused tests when required

**Interfaces:**

- Consumes: selected-group/detail loading patterns, readiness endpoints, lifecycle guards, typed mutation results, and exact approve/lock/reopen permissions.
- Produces: fast read-first summaries with only selected group/learner/run detail loaded, while approval, lock, correction, calculation, reopen, and export workflows remain lazy and exact-permission guarded.

- [ ] **Step 1: Execute four branches**

1. gradebook and result preparation;
2. result locks and corrections;
3. aggregate and annual results;
4. promotion plus term/year lifecycle.

- [ ] **Step 2: Write failure-first request-count tests**

Preserve the existing guarantees that gradebook/results load subject collections once and fetch only the selected group or learner workspace. Reject `Promise.all(collection.map(detailRequest))`, full-detail hydration for every row, and eager approval/action data.

- [ ] **Step 3: Move summary/readiness regions to route loaders**

Start independently visible summary/readiness regions together. Load a selected group, student, aggregate revision, promotion run, or lifecycle consequence only after its exact URL/default-selection dependency is known. Prefer URL ownership for selections that need deep links/history.

- [ ] **Step 4: Isolate high-risk mutation regions**

Locking, correction approval, recalculation, publishing, promotion, closure, and reopening must keep their existing backend authorization and lifecycle checks. Patch returned status/revision resources or invalidate only the affected summary/detail dependency; re-read authoritative state after incomplete realtime events.

- [ ] **Step 5: Profile aggregate SQL safely**

Use sanitized representative cardinalities and `EXPLAIN (ANALYZE, BUFFERS)` for slow readiness, learner aggregation, annual snapshots, and lifecycle consequences. Avoid giant joins that multiply learners, subjects, phases, and revisions. Prefer pre-aggregation and bounded selected-detail reads when it reduces repeated work.

- [ ] **Step 6: Run database-backed lifecycle/result tests**

Use `./scripts/test_backend_school.sh` for changed database-backed owners and run package tests for `school-academic-assessment`, `school-academic-results`, and `school-academic-lifecycle` as applicable. Never use a persistent production tenant or pooled Neon URL for routine tests.

- [ ] **Step 7: Regenerate contracts, verify each branch, and update inventory**

Apply the same exact-tree gates as Tasks 3–4 and commit only routes whose complete behavior is verified.

---

### Task 6: Roll Out Admission and Supervision Routes

**Files:**

- Modify all pages under:
  - `staff/academic/admission`
  - `staff/academic/supervision`
- Modify owning admission/supervision API modules, Rust services/handlers, contracts, inventory, and focused tests when required

**Interfaces:**

- Consumes: route/detail/list ownership, shared round/cycle context, permission-gated workflow data, file boundaries, and existing child-route layouts.
- Produces: round/cycle summaries available early, selected application/observation details isolated, and scoring/selection/enrollment/approval actions loaded only inside their workflows.

- [ ] **Step 1: Execute admission in four branches**

1. round list/create/detail shell;
2. applications list and application detail;
3. exam rooms, scores, and selections;
4. enrollment, student IDs, and report.

- [ ] **Step 2: Execute supervision in three branches**

1. overview, cycles, templates;
2. requests, approvals, evaluate;
3. selected supervision detail.

- [ ] **Step 3: Test nested route ownership before moving reads**

Identify which data belongs in the round/cycle layout and which belongs to the child page. The narrowest shared layout may own stable shared context, but must not load data unused by most children. Verify direct child URLs and parent-to-child navigation do not duplicate the shared request.

- [ ] **Step 4: Migrate visible list/summary/detail regions**

Lists and the selected default detail follow the same independent/dependent rules as Academic Delivery. Search/filter/pagination parameters must be URL-owned when users need refresh/back/forward behavior. Uploads, scoring tools, bulk actions, approval data, and report/export generation remain lazy.

- [ ] **Step 5: Optimize measured batch work**

Check application counts/status summaries, room assignments, scoring coverage, selection results, enrollment readiness, supervision assignments, rubric responses, and approval queues for per-row database reads. Batch only within bounded round/cycle scopes and preserve authorization ownership.

- [ ] **Step 6: Verify permissions and sensitive-data minimization**

Test read-only and denied paths. Ensure preload/list responses omit unnecessary applicant/student/staff PII and never contain national IDs or blind indexes. File access remains authorized through the canonical file platform.

- [ ] **Step 7: Run owner packages, contract generation, browser workflows, and inventory gates**

Run `school-admission` and `school-supervision` package/database tests as applicable, then the shared matrix. Commit each branch separately.

---

### Task 7: Roll Out Non-Academic Staff Routes

**Files:**

- Modify route-owned files under:
  - `staff` dashboard
  - `staff/achievements/**`
  - `staff/calendar`
  - `staff/certificate-requests/**`
  - `staff/certificates/**`
  - `staff/facility/buildings`
  - `staff/features`
  - `staff/manage/**`
  - `staff/menu`
  - `staff/organization/**`
  - `staff/profile`
  - `staff/roles/**`
  - `staff/school-fonts`
  - `staff/school-settings`
  - `staff/settings`
  - `staff/students/**`
  - `staff/view/[id]`
  - `staff/work/**`
- Modify owned API/backend/contracts/tests only when required
- Modify inventory records

**Interfaces:**

- Consumes: route-region foundation, `/api/auth/me` as the sole current-user permission source, typed file-platform URLs, and each domain's list/detail/mutation APIs.
- Produces: fast read-first staff operations without eager administrative options, editor assets, exports, uploads, or mutation-only data.

- [ ] **Step 1: Execute independent domain branches**

Use these boundaries:

1. staff dashboard and personal profile/view;
2. staff directory, students, organization, and roles;
3. work and calendar;
4. certificates, certificate requests, templates, recipients, and issued records;
5. achievements;
6. facilities, school settings, fonts, features, and menu configuration.

- [ ] **Step 2: Test list/detail and exact-permission behavior first**

For each domain, test that a readable list/detail appears without create/update/delete/approve/export option requests. Test focused retry, empty state, pagination/filter transitions, direct IDs, permission denial, and superseded search.

- [ ] **Step 3: Migrate primary reads and keep heavy tools lazy**

Move default visible lists, summaries, and details into route loaders. Keep certificate editors, font/image assets, spreadsheet/PDF/document libraries, bulk selectors, uploads, previews, exports, and management options lazy after the exact user action.

- [ ] **Step 4: Prevent duplicate shared reads**

Do not refetch `/api/auth/me`, menu permissions, or existing layout-owned identity state per page. Put genuinely shared stable child-route data in the narrowest layout only when multiple children consume it and share freshness/invalidation.

- [ ] **Step 5: Profile domain SQL where navigation is backend-bound**

Inspect list/detail endpoints for per-person roles, per-student relationships, organization descendants, certificate counts, work assignees, file metadata, or calendar expansions. Use bounded set-based reads and maintain tenant/resource authorization. Do not merge separate domains merely to reduce request count.

- [ ] **Step 6: Run domain owner tests and frontend acceptance**

Run `school-staff`, `school-students`, `school-calendar`, `school-fonts`, `school-certificates`, `school-file-platform`, and other changed package suites as applicable, plus contracts, Svelte checks, static tests, and focused Playwright flows.

- [ ] **Step 7: Complete inventory records and commit each domain branch**

Only mark a domain route complete after its child-component reads and mutation paths have been audited, not merely its top-level page.

---

### Task 8: Roll Out Student, Parent, Account, Consent, and Diagnostic Routes

**Files:**

- Modify route-owned files under:
  - `student/**`
  - `parent/**`
  - `account/security`
  - `settings/consent`
  - `403`
  - `debug`
- Modify owned API/backend/contracts/tests only when required
- Modify inventory records

**Interfaces:**

- Consumes: authenticated identity/layout state, academic context, child/student resource authorization, safe preload policy, and shared calendar/timetable/exam endpoints.
- Produces: independently loading student/parent regions without duplicate profile/context reads and with protected child ownership on every backend request.

- [ ] **Step 1: Execute four branches**

1. student home/profile/settings/certificates;
2. student timetable/calendar/exams/activities;
3. parent home, child summary, child timetable/calendar/exams;
4. account security, consent, 403, and diagnostic classification.

- [ ] **Step 2: Test identity and academic-context reuse**

Ensure routes reuse existing authenticated identity and already resolved academic context rather than issuing duplicate year/term/current-user reads. Missing or stale context uses a neutral prerequisite skeleton and URL repair fallback without painting a false empty state.

- [ ] **Step 3: Test resource authorization and navigation**

Cover a parent authorized for one child, a denied child ID, student self-only access, direct URLs, back/forward, hover or tap preload, logout during a request, and stale-response rejection. Frontend filtering never substitutes for backend authorization.

- [ ] **Step 4: Migrate visible regions and preserve optional behavior**

Load only visible summary/schedule/event regions. Keep downloads, security mutations, consent submission, profile updates, and optional detail panels lazy or mutation-owned. `403` and genuinely static routes are recorded as `no-data`; the diagnostic route uses `none` preload and remains excluded from production performance conclusions.

- [ ] **Step 5: Profile shared schedule/calendar reads**

Where student and parent schedules are slow, inspect bounded term/date predicates, repeated profile lookups, event expansion, and per-child/per-round reads. Share backend helpers only within the existing domain authorization boundary; do not expose a broader child aggregate endpoint.

- [ ] **Step 6: Run auth/session-sensitive checks when touched**

If account/session behavior changes, run the focused auth/session static, Rust, browser, and proxy smoke checks required by `.rules` and `docs/TESTING.md`. Otherwise do not expand this performance rollout into authentication changes.

- [ ] **Step 7: Complete inventory, verify, and commit each branch**

Run focused browser coverage for student and parent roles with dedicated disposable accounts when live credentials are required. Report unavailable live runs explicitly.

---

### Task 9: Remove the Migration Baseline and Run Final Acceptance

**Files:**

- Modify: `frontend-school/tests/fixtures/route-data-loading-inventory.json`
- Modify: `frontend-school/tests/static/route-data-loading-inventory.test.mjs`
- Modify: `frontend-school/tests/static/route-data-loading-policy.test.mjs`
- Modify: `frontend-school/tests/e2e/route-region-loading.spec.ts`
- Delete only temporary migration allowances that have reached zero
- Review: every route and backend owner changed by Tasks 1–8

**Interfaces:**

- Consumes: all completed inventory records, per-wave verification evidence, generated contracts, and representative deployed samples.
- Produces: a durable zero-regression guard and final evidence that the program improved progressive rendering without permission, correctness, payload, or database regressions.

- [ ] **Step 1: Make the final guard fail on any incomplete route**

Change the inventory test to require every record to be `complete` and reject any remaining legacy component-owned primary startup read. Keep explicit support for valid interaction-only/browser-only API calls and require their inventory classification.

- [ ] **Step 2: Confirm the red state if any route remains**

```bash
npm --prefix frontend-school run test:route-loading
```

Expected: FAIL listing exact remaining routes, or PASS only when the inventory is actually complete.

- [ ] **Step 3: Remove zero-value migration counters and hashes**

Delete the temporary count/hash baseline after it reaches zero. Retain durable semantic guards for full inventory coverage, `/page-view` prohibition, focused invalidation, safe preload, route-owned primary data, and reviewed interaction-only exceptions.

- [ ] **Step 4: Run representative cross-domain browser acceptance**

Cover at least one route from each completed branch group with delayed sibling responses, one failed sibling plus retry, one context/selection supersession, one mutation reconciliation, and one optional lazy workflow. Run five warm deployed samples for the high-use or previously slow representative in every domain.

Acceptance compares the same tenant, user permissions, route context, browser, connection profile, and warm-run count. Use medians; never select only the fastest sample.

- [ ] **Step 5: Review every SQL performance conclusion**

For each changed query, retain sanitized before/after endpoint phase timing and representative query-plan evidence in review history. Confirm response bounds and pool concurrency. If measurement did not improve, revert the speculative SQL change or document why correctness required it without claiming a performance win.

- [ ] **Step 6: Run the frontend matrix**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:route-loading
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build
```

Expected: every command PASS.

- [ ] **Step 7: Run the backend matrix for all changed owners**

From `backend-school`:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
```

Also run `cargo test -p <changed-package> -- --test-threads=8` for every changed domain package and `./scripts/test_backend_school.sh` for database-backed application/integration tests. Run:

```bash
cargo test api_contract::tests -- --nocapture
```

when any wire contract changed.

- [ ] **Step 8: Run final browser tests**

Run mocked request-shape coverage unconditionally. Run credentialed Playwright flows with dedicated disposable accounts for affected roles; report missing credentials as unrun rather than replacing them with weaker tests.

- [ ] **Step 9: Review exact-tree state**

```bash
git diff --check
git status --short
git diff origin/main...HEAD --stat
git diff origin/main...HEAD
```

Expected: no whitespace errors, no generated-contract drift, no temporary migration allowance, and only the approved final-wave changes.

- [ ] **Step 10: Complete the program**

After the final branch is integrated and deployed acceptance passes, remove this temporary implementation plan and any superseded rollout workflow artifact in the cleanup change, as required by `.rules`. Preserve the durable standards in `.rules`, executable guards, generated contracts, and Git history.

---

## Rollout Order and Stop/Go Gates

Execute in this order:

1. inventory;
2. measurement foundation;
3. academic foundation/catalog;
4. teaching/timetable/exams/assessment;
5. results/lifecycle/promotion;
6. admission/supervision;
7. non-academic staff;
8. student/parent/account;
9. final acceptance.

Each branch must stop instead of integrating when any of these occurs:

- the route's data ownership is still ambiguous;
- moving a read would broaden authorization or expose action-only data;
- a contract change is not represented by generated Rust/OpenAPI/TypeScript artifacts;
- a SQL change lacks representative evidence or exhausts pool capacity;
- direct navigation, context switching, cancellation, retry, or mutation reconciliation regresses;
- required focused or matrix checks fail; or
- `origin/main` advanced and conflicts or a different tree invalidate earlier verification.

## Planning Decisions

- Do not migrate 71 files mechanically. Audit all 104 authenticated pages and change only actual primary-read owners.
- Do not promise one request per page. The target is the earliest correct useful region with bounded total work.
- Keep Academic Delivery as the behavior reference, not a generic shared component requirement.
- Reuse current utilities until at least three migrated domains demonstrate the same missing abstraction; do not introduce a speculative route-region framework.
- Make SQL optimization part of each affected domain wave, not a disconnected global rewrite.
- Use separate branches and review gates because route groups have different authorization, contracts, database owners, and operational risk.
