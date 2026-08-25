# Phase B Query Contract Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair every Phase B route currently capable of emitting an invalid or missing academic-year query, and make its runtime DTO, OpenAPI operation, generated TypeScript type, API wrapper, and role-scoped UI selection agree.

**Architecture:** Axum query DTOs remain the runtime source of truth and are referenced directly by `utoipa::IntoParams`. The frontend sends generated operation query objects through one central `apiClient` serializer; staff pages use the existing staff Topbar Academic Context, while student and parent pages resolve only role-authorized academic years.

**Tech Stack:** Rust, Axum, SQLx, utoipa/OpenAPI, TypeScript, SvelteKit 5 runes, Node test runner

**Spec:** `docs/superpowers/specs/2026-08-25-query-contract-alignment-design.md`

**Checkpoint boundary:** This plan implements the spec's deployable Phase B repair checkpoint. The application-wide wrapper inventory, remaining query migrations, and final no-manual-query guard receive their own checkpoint-2 implementation plan after this repair passes production verification.

## Global Constraints

- Run every command one at a time; do not run tests, generators, or builds concurrently in this workspace.
- Do not add a database migration or change academic data.
- Do not accept `academic_year_id`, `page_size`, or any other legacy query alias.
- Query DTOs use `#[serde(rename_all = "camelCase", deny_unknown_fields)]` and `utoipa::IntoParams`.
- Frontend API wrappers use generated `operations[...]` query types and the central query serializer.
- Keep the staff Topbar Academic Context staff-only; student and parent choices come from learner/parent-scoped endpoints.
- Preserve the current student-list snake_case response fields and document them exactly; do not invent `total` or `total_pages`.
- Keep national IDs out of logs, fixtures, errors, and committed source.
- Use `apply_patch` for edits and make one focused commit after each task passes its focused checks.

---

### Task 1: Add the central typed GET-query transport

**Files:**
- Create: `frontend-school/src/lib/api/query.ts`
- Modify: `frontend-school/src/lib/api/client.ts`
- Create: `frontend-school/tests/static/api-query-contract.test.mjs`

**Interfaces:**
- Produces: `ApiQueryPrimitive`, `ApiQueryValue`, `ApiQuery`, and `appendApiQuery(endpoint, query)`.
- Produces: `ApiRequestOptions.query?: ApiQuery`, consumed by every migrated wrapper.

- [ ] **Step 1: Write the failing serializer test**

Add a TypeScript-module loader and focused cases to `api-query-contract.test.mjs`:

```js
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function importTypescript(relativePath) {
	const source = await readFile(path.join(projectRoot, relativePath), 'utf8');
	const output = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
		fileName: relativePath
	}).outputText;
	return import(`data:text/javascript;base64,${Buffer.from(output).toString('base64')}#${Date.now()}`);
}

test('appendApiQuery encodes defined scalars, repeated arrays, and existing queries', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.equal(
		appendApiQuery('/api/students', {
			academicYearId: 'year/1',
			page: 2,
			activeOnly: false,
			search: undefined,
			status: null,
			tagId: ['tag-a', 'tag-b']
		}),
		'/api/students?academicYearId=year%2F1&page=2&activeOnly=false&tagId=tag-a&tagId=tag-b'
	);
	assert.equal(appendApiQuery('/api/students?view=compact', { pageSize: 20 }), '/api/students?view=compact&pageSize=20');
});

test('appendApiQuery rejects non-scalar query values', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.throws(() => appendApiQuery('/api/students', { filter: { status: 'active' } }));
});
```

- [ ] **Step 2: Run the focused test and verify the missing module failure**

Run from `frontend-school`:

```bash
node --test tests/static/api-query-contract.test.mjs
```

Expected: FAIL because `src/lib/api/query.ts` does not exist.

- [ ] **Step 3: Implement the pure serializer**

Create `query.ts` with this contract:

```ts
export type ApiQueryPrimitive = string | number | boolean;
export type ApiQueryValue =
	| ApiQueryPrimitive
	| readonly ApiQueryPrimitive[]
	| null
	| undefined;
export type ApiQuery = Readonly<Record<string, ApiQueryValue>>;

function appendValue(params: URLSearchParams, key: string, value: unknown): void {
	if (value === undefined || value === null) return;
	if (Array.isArray(value)) {
		for (const item of value) appendValue(params, key, item);
		return;
	}
	if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
		throw new TypeError(`Unsupported API query value for ${key}`);
	}
	params.append(key, String(value));
}

export function appendApiQuery(endpoint: string, query?: ApiQuery): string {
	if (!query) return endpoint;
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query)) appendValue(params, key, value);
	const encoded = params.toString();
	if (!encoded) return endpoint;
	return `${endpoint}${endpoint.includes('?') ? '&' : '?'}${encoded}`;
}
```

- [ ] **Step 4: Route `apiClient` GETs through the serializer**

Import `appendApiQuery` and `ApiQuery` in `client.ts`, extend the existing options, and keep signal behavior unchanged:

```ts
import { appendApiQuery, type ApiQuery } from '$lib/api/query';

export interface ApiRequestOptions {
	signal?: AbortSignal;
	query?: ApiQuery;
}

async get<T, E = never>(endpoint: string, options: ApiRequestOptions = {}) {
	return this.request<T, E>(appendApiQuery(endpoint, options.query), {
		method: 'GET',
		signal: options.signal
	});
}
```

Do the same for `getBlob`; leave `getExternalBlob` unchanged because it accepts an external URL rather than a backend endpoint.

- [ ] **Step 5: Run focused tests and type checking**

Run from `frontend-school`, one command at a time:

```bash
node --test tests/static/api-query-contract.test.mjs
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: both PASS.

- [ ] **Step 6: Commit the transport**

```bash
git add frontend-school/src/lib/api/query.ts frontend-school/src/lib/api/client.ts frontend-school/tests/static/api-query-contract.test.mjs
git commit -m "feat(api): add typed query transport"
```

---

### Task 2: Make existing runtime query DTOs visible in OpenAPI

**Files:**
- Modify: `backend-school/src/modules/students/models.rs`
- Modify: `backend-school/src/modules/students/handlers.rs`
- Modify: `backend-school/src/modules/parents/handlers.rs`
- Modify: `backend-school/src/modules/calendar/handlers.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Consumes: existing `ListStudentsQuery`, `StudentAcademicYearQuery`, and `CalendarEventQuery` runtime DTOs.
- Produces: OpenAPI operations `listStudents`, `getStudent`, `getStudentProfile`, `getParentProfile`, and `getParentChildProfile` with their exact query parameters.
- Produces: generated schemas `StudentListItem` and `StudentListResponse`.

- [ ] **Step 1: Replace stale OpenAPI assertions with exact failing query-contract assertions**

In `api_contract.rs` tests, add a helper that returns `(name, required)` for query parameters and a test named `documents_academic_year_scoped_profile_and_calendar_queries` containing these exact contracts:

```rust
fn query_contract(document: &Value, path: &str, method: &str) -> BTreeSet<(String, bool)> {
    document["paths"][path][method]["parameters"]
        .as_array()
        .expect("operation parameters must be an array")
        .iter()
        .filter(|parameter| parameter["in"] == "query")
        .map(|parameter| {
            (
                parameter["name"].as_str().expect("query name").to_string(),
                parameter["required"].as_bool().unwrap_or(false),
            )
        })
        .collect()
}

assert_eq!(
    query_contract(&document, "/api/students", "get"),
    BTreeSet::from([
        ("academicYearId".to_string(), true),
        ("page".to_string(), false),
        ("pageSize".to_string(), false),
        ("search".to_string(), false),
        ("status".to_string(), false),
    ])
);
assert_eq!(
    query_contract(&document, "/api/student/profile", "get"),
    BTreeSet::from([("academicYearId".to_string(), true)])
);
assert_eq!(
    query_contract(&document, "/api/parent/profile", "get"),
    BTreeSet::from([("academicYearId".to_string(), true)])
);
```

Assert the same single required query for `/api/students/{id}` and `/api/parent/students/{student_id}`. For each of the four calendar endpoints, assert `academicYearId` required and `academicTermId`, `from`, `to`, `categoryId`, `tagId`, `audience`, `visibility`, and `q` optional. Preserve the path-ID assertion on the parent-child calendar operation.

- [ ] **Step 2: Run the OpenAPI test and verify the documented/runtime mismatch**

Run from `backend-school`:

```bash
cargo test api_contract::tests::documents_academic_year_scoped_profile_and_calendar_queries -- --exact
```

Expected: FAIL because student GET operations are absent and profile/calendar query lists are incomplete or snake_case.

- [ ] **Step 3: Add schemas and typed handler annotations**

Add `utoipa::ToSchema` to `StudentListItem` and `StudentListResponse`. Add these annotations:

```rust
#[utoipa::path(
    get,
    path = "/api/students",
    operation_id = "listStudents",
    tag = "student",
    params(ListStudentsQuery),
    responses(
        (status = 200, description = "Students in the selected academic year", body = ApiResponse<StudentListResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student list access denied", body = ApiErrorResponse)
    )
)]
```

Document `getStudent` with its `id` path parameter plus `StudentAcademicYearQuery`. Add `params(StudentAcademicYearQuery)` to student self-profile and parent self-profile. Add the path parameter plus `StudentAcademicYearQuery` to parent child-profile.

Replace every manual calendar query list with `params(CalendarEventQuery)`. The parent-child calendar operation uses:

```rust
params(
    ("student_id" = Uuid, Path, description = "Linked student user ID"),
    crate::modules::calendar::models::CalendarEventQuery
)
```

- [ ] **Step 4: Register the missing paths and schemas**

In `api_contract.rs`:

- import `StudentListItem` and `StudentListResponse`;
- register `students::handlers::list_students` and `students::handlers::get_student` in `paths(...)`;
- register `StudentListItem`, `StudentListResponse`, and `ApiResponse<StudentListResponse>` in `components(schemas(...))`.

- [ ] **Step 5: Run focused OpenAPI tests and formatting**

Run from `backend-school`, one command at a time:

```bash
cargo test api_contract::tests::documents_academic_year_scoped_profile_and_calendar_queries -- --exact
```

```bash
cargo fmt --all -- --check
```

Expected: both PASS and no calendar assertion refers to `category_id` or `tag_id`.

- [ ] **Step 6: Commit the backend contract repair**

```bash
git add backend-school/src/modules/students/models.rs backend-school/src/modules/students/handlers.rs backend-school/src/modules/parents/handlers.rs backend-school/src/modules/calendar/handlers.rs backend-school/src/api_contract.rs
git commit -m "fix(api): align academic query documentation"
```

---

### Task 3: Add parent-scoped academic context options

**Files:**
- Modify: `backend-school/src/modules/academic/core/services/context.rs`
- Modify: `backend-school/src/modules/parents/services.rs`
- Modify: `backend-school/src/modules/parents/handlers.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Produces: `academic_context_service::list_options_for_parent(pool, parent_id)`.
- Produces: `parent_service::get_parent_academic_context_options(pool, parent_id)`.
- Produces: authenticated GET `/api/parent/academic-context/options`, operation ID `listParentAcademicContextOptions`, response `ApiResponse<AcademicContextOptions>`.

- [ ] **Step 1: Write the failing parent service test**

Add a database test beside `parent_profile_lists_child_in_the_caller_selected_academic_year`. Reuse the cutover fixture, create one active parent user, link the fixture student through `student_parents`, then assert:

```rust
let options = get_parent_academic_context_options(&pool, parent_id)
    .await
    .unwrap();
assert_eq!(
    options.years.iter().map(|year| year.year).collect::<Vec<_>>(),
    vec![2026, 2025]
);
assert!(options
    .terms
    .iter()
    .all(|term| options.years.iter().any(|year| year.id == term.academic_year_id)));
```

Also create a non-parent active user and assert the service returns `AppError::Forbidden` before returning options.

- [ ] **Step 2: Run the parent test and verify the missing-function failure**

Run from the repository root:

```bash
./scripts/test_backend_school.sh modules::parents::services::tests::parent_academic_context_contains_only_linked_student_years -- --exact
```

Expected: FAIL because `get_parent_academic_context_options` does not exist.

- [ ] **Step 3: Implement the parent-owned option query**

Add `list_options_for_parent` to the academic context service. Bind `parent_id` once and use this query so every returned year and term belongs to at least one active linked student:

```sql
WITH linked_years AS (
    SELECT DISTINCT
        year.id, year.year, year.name, year.start_date, year.end_date, year.status
    FROM student_parents parent_link
    JOIN users student
      ON student.id = parent_link.student_user_id
     AND student.user_type = 'student'
     AND student.status = 'active'
    JOIN student_academic_years student_year
      ON student_year.student_id = student.id
    JOIN academic_years year ON year.id = student_year.academic_year_id
    WHERE parent_link.parent_user_id = $1
),
linked_terms AS (
    SELECT term.*
    FROM academic_terms term
    JOIN linked_years year ON year.id = term.academic_year_id
)
SELECT
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'id', year.id,
                'year', year.year,
                'name', year.name,
                'startDate', year.start_date,
                'endDate', year.end_date,
                'status', year.status
            ) ORDER BY year.year DESC, year.start_date DESC, year.id
        )
        FROM linked_years year
    ), '[]'::jsonb),
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'id', term.id,
                'academicYearId', term.academic_year_id,
                'sequence', term.sequence_no,
                'code', term.code,
                'name', term.name,
                'termType', term.term_type,
                'startDate', term.start_date,
                'endDate', term.end_date,
                'includedInYearResult', term.included_in_year_result,
                'blocksYearClosure', term.blocks_year_closure,
                'status', term.status
            ) ORDER BY year.year DESC, term.sequence_no, term.start_date, term.id
        )
        FROM linked_terms term
        JOIN linked_years year ON year.id = term.academic_year_id
    ), '[]'::jsonb),
    (SELECT id FROM linked_years WHERE status = 'active'),
    (SELECT id FROM linked_terms WHERE status = 'active')
```

Deserialize the tuple as `(Json<Vec<AcademicYearOption>>, Json<Vec<AcademicTermOption>>, Option<Uuid>, Option<Uuid>)` and return those four values through `AcademicContextOptions`. On SQL failure, log `reason = "parent_academic_context_options_query_failed"` and the database error, then return `ไม่สามารถโหลดประวัติปีและภาคเรียนของบุตรหลานได้`.

Expose it through the parent service only after `ensure_parent_user(pool, parent_id).await?`:

```rust
pub async fn get_parent_academic_context_options(
    pool: &PgPool,
    parent_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    academic_context_service::list_options_for_parent(pool, parent_id).await
}
```

- [ ] **Step 4: Add the typed handler, runtime route, and OpenAPI registration**

Add this handler contract before the child-scoped context handler:

```rust
#[utoipa::path(
    get,
    path = "/api/parent/academic-context/options",
    operation_id = "listParentAcademicContextOptions",
    tag = "parent",
    responses(
        (status = 200, description = "Academic years and terms available across linked active students", body = ApiResponse<AcademicContextOptions>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Parent account required", body = ApiErrorResponse)
    )
)]
```

The handler resolves `actor_tenant_context_from_session`, passes `context.actor.user_id` to the parent service, and returns `ApiResponse::ok(options)`. Register the path in `app.rs` and the operation in `api_contract.rs`.

Add an OpenAPI test with the exact name used below:

```rust
#[test]
fn documents_parent_academic_context_options() {
    let document = school_api_value().expect("document should serialize");
    assert_eq!(
        document["paths"]["/api/parent/academic-context/options"]["get"]["operationId"],
        "listParentAcademicContextOptions"
    );
    assert_eq!(
        document["paths"]["/api/parent/academic-context/options"]["get"]["responses"]["200"]
            ["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/ApiResponse_AcademicContextOptions"
    );
}
```

- [ ] **Step 5: Run the service and OpenAPI tests**

Run one command at a time:

```bash
./scripts/test_backend_school.sh modules::parents::services::tests::parent_academic_context_contains_only_linked_student_years -- --exact
```

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::documents_parent_academic_context_options -- --exact
```

Expected: both PASS.

- [ ] **Step 6: Commit the parent context endpoint**

```bash
git add backend-school/src/modules/academic/core/services/context.rs backend-school/src/modules/parents/services.rs backend-school/src/modules/parents/handlers.rs backend-school/src/app.rs backend-school/src/api_contract.rs
git commit -m "feat(parent): expose linked academic context"
```

---

### Task 4: Regenerate and lock the corrected API contract

**Files:**
- Modify generated artifact: `contracts/openapi/school-api.json`
- Modify generated artifact: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`

**Interfaces:**
- Produces generated query types for all operations repaired in Tasks 2 and 3.
- Produces generated `StudentListItem` and `StudentListResponse` schemas.

- [ ] **Step 1: Add failing generated-contract assertions**

Read the generated TypeScript file in `api-query-contract.test.mjs` and assert it contains these operations and camelCase query keys:

```js
for (const operationId of [
	'listStudents',
	'getStudent',
	'getStudentProfile',
	'getParentProfile',
	'getParentChildProfile',
	'listParentAcademicContextOptions',
	'listCalendarEvents',
	'listMyCalendarEvents',
	'getParentChildCalendarEvents',
	'listPublicCalendarEvents'
]) {
	assert.match(generated, new RegExp(`\\b${operationId}: \\{`));
}
assert.match(generated, /academicYearId:\s*string/);
assert.match(generated, /pageSize\?:\s*number/);
assert.doesNotMatch(generated, /academic_year_id\?:|page_size\?:/);
assert.match(generated, /StudentListResponse:/);
assert.match(generated, /homeroom:/);
```

- [ ] **Step 2: Verify committed artifacts are stale**

Run from `frontend-school`:

```bash
npm run check:api-contracts
```

Expected: FAIL and report generated artifact differences.

- [ ] **Step 3: Regenerate the artifacts**

Run from `frontend-school`:

```bash
npm run generate:api-contracts
```

- [ ] **Step 4: Verify generated artifacts and focused tests**

Run one command at a time from `frontend-school`:

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

```bash
node --test tests/static/api-query-contract.test.mjs
```

Expected: all PASS.

- [ ] **Step 5: Commit generated contracts**

```bash
git add contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/tests/static/api-query-contract.test.mjs
git commit -m "chore(api): regenerate academic query contracts"
```

---

### Task 5: Migrate shared academic and calendar wrappers

**Files:**
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Modify: `frontend-school/src/lib/api/academic-context.ts`
- Modify: `frontend-school/src/lib/api/calendar.ts`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`

**Interfaces:**
- Consumes: generated `operations[...]` query types and `ApiRequestOptions.query`.
- Produces: a corrected `listGradeLevelOptions`, all four calendar-list wrappers, and `listParentAcademicContextOptions()`.

- [ ] **Step 1: Add failing real-wrapper recorder cases**

Add a helper to `api-query-contract.test.mjs` that transpiles a real wrapper, replaces only `$lib/api/client` with a data-URL module, and records `apiClient.get(endpoint, options)` calls in `globalThis.__schoolOrbitApiCalls`. Assert:

```js
await academicCore.listGradeLevelOptions('year-1');
assert.deepEqual(calls.pop(), {
	method: 'get',
	endpoint: '/api/lookup/grade-levels',
	options: { query: { academicYearId: 'year-1' } }
});

await calendar.listCalendarEvents({
	academicYearId: 'year-1',
	academicTermId: 'term-1',
	from: '2026-08-01',
	to: '2026-08-31',
	categoryId: 'category-1',
	tagId: 'tag-1',
	audience: 'student',
	visibility: 'private',
	q: 'สอบ'
});
assert.deepEqual(calls.pop().options.query, {
	academicYearId: 'year-1',
	academicTermId: 'term-1',
	from: '2026-08-01',
	to: '2026-08-31',
	categoryId: 'category-1',
	tagId: 'tag-1',
	audience: 'student',
	visibility: 'private',
	q: 'สอบ'
});
```

Add equivalent path/query assertions for `listMyCalendarEvents`, `listChildCalendarEvents`, and `listPublicCalendarEvents`.

- [ ] **Step 2: Run the recorder test and verify the manual-query failure**

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

Expected: FAIL because the wrappers still concatenate query strings.

- [ ] **Step 3: Migrate grade-level lookup and add the parent option wrapper**

Import `operations` alongside `components`. Keep the existing encoded helper for wrappers deferred to checkpoint 2, and add a query-object helper that returns a trimmed raw value:

```ts
function requiredContextValue(value: string, label: string): string {
	const selected = value.trim();
	if (!selected) throw new Error(`กรุณาเลือก${label}ก่อน`);
	return selected;
}
```

Use the generated operation query:

```ts
type LookupGradeLevelsQuery = NonNullable<
	operations['lookupGradeLevels']['parameters']['query']
>;

export const listGradeLevelOptions = (academicYearId: string) => {
	const query = {
		academicYearId: requiredContextValue(academicYearId, 'ปีการศึกษา')
	} satisfies LookupGradeLevelsQuery;
	return academicData(
		apiClient.get<GradeLevelOption[]>('/api/lookup/grade-levels', { query }),
		'ไม่สามารถโหลดระดับชั้นได้'
	);
};
```

Existing `requiredContext(...)` callers remain unchanged in this checkpoint. Checkpoint 2 removes that encoded helper after migrating their wrappers. Add `listParentAcademicContextOptions(signal?)` in `academic-context.ts`, using `/api/parent/academic-context/options` and the generated `AcademicContextOptions` response.

- [ ] **Step 4: Replace all four calendar query builders**

Derive every filter from its own generated operation. The management wrapper pattern is:

```ts
export type CalendarEventFilters = NonNullable<
	operations['listCalendarEvents']['parameters']['query']
>;

export async function listCalendarEvents(filters: CalendarEventFilters) {
	if (!filters.academicYearId.trim()) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	const response = await apiClient.get<CalendarEventDto[]>('/api/calendar/events', {
		query: { ...filters }
	});
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมปฏิทินได้').map(calendarEventFromDto);
}
```

Define and use each remaining operation explicitly:

```ts
type MyCalendarQuery = NonNullable<
	operations['listMyCalendarEvents']['parameters']['query']
>;
type ChildCalendarQuery = NonNullable<
	operations['getParentChildCalendarEvents']['parameters']['query']
>;
type GeneratedPublicCalendarQuery = NonNullable<
	operations['listPublicCalendarEvents']['parameters']['query']
>;
export type CalendarPublicEventFilters = Omit<
	GeneratedPublicCalendarQuery,
	'audience' | 'visibility'
>;

apiClient.get<CalendarViewerEvent[]>('/api/me/calendar/events', {
	query: { ...filters } satisfies MyCalendarQuery
});
apiClient.get<CalendarViewerEvent[]>(
	`/api/parent/students/${encodeURIComponent(studentId)}/calendar/events`,
	{ query: { ...filters } satisfies ChildCalendarQuery }
);
apiClient.get<CalendarPublicEvent[]>('/api/public/calendar/events', {
	query: { ...filters } satisfies GeneratedPublicCalendarQuery
});
```

- [ ] **Step 5: Update focused static expectations**

Require generated operation query types, client query objects, and the new parent context endpoint. Remove existing regex expectations for `params.set(...)`. Assert no `academic_year_id`, `category_id`, or `tag_id` occurs in `academic-core.ts` or `calendar.ts`.

- [ ] **Step 6: Run focused tests and type checking**

Run one command at a time from `frontend-school`:

```bash
node --test tests/static/api-query-contract.test.mjs
```

```bash
node --test tests/static/academic-context-contract.test.mjs
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all PASS.

- [ ] **Step 7: Commit the shared wrapper cutover**

```bash
git add frontend-school/src/lib/api/academic-core.ts frontend-school/src/lib/api/academic-context.ts frontend-school/src/lib/api/calendar.ts frontend-school/tests/static/api-query-contract.test.mjs frontend-school/tests/static/academic-context-contract.test.mjs
git commit -m "fix(frontend): type academic calendar queries"
```

---

### Task 6: Connect staff student routes to the Topbar year

**Files:**
- Modify: `frontend-school/src/lib/api/students.ts`
- Modify: `frontend-school/src/routes/(app)/staff/students/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/students/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/students/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/students/[id]/edit/+page.svelte`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`

**Interfaces:**
- Produces: generated `StudentListItem`, `StudentListResponse`, and `ListStudentsQuery` aliases.
- Produces: direct `listStudents(query): Promise<StudentListResponse>` and `getStudent(id, academicYearId): Promise<Student>` results.
- Consumes: existing `getAcademicContextStore()`.

- [ ] **Step 1: Add failing wrapper and route assertions**

Record and assert:

```js
await students.listStudents({
	academicYearId: 'year-1',
	page: 2,
	pageSize: 20,
	search: 'สมชาย',
	status: 'active'
});
assert.deepEqual(calls.pop().options.query, {
	academicYearId: 'year-1',
	page: 2,
	pageSize: 20,
	search: 'สมชาย',
	status: 'active'
});
await students.getStudent('student-1', 'year-1');
assert.deepEqual(calls.pop(), {
	method: 'get',
	endpoint: '/api/students/student-1',
	options: { query: { academicYearId: 'year-1' } }
});
```

Require `academicContext: 'year_required'`, `getAcademicContextStore`, and `academicYearId` in all three staff Svelte pages. Assert no `page_size`, `total_pages`, or `class_room` remains in the staff list page.

- [ ] **Step 2: Run focused tests and verify failure**

Run one command at a time:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

Expected: FAIL on the legacy student query and missing route context.

- [ ] **Step 3: Migrate only the staff-facing student wrappers**

Use generated types:

```ts
export type StudentListItem = Schemas['StudentListItem'];
export type StudentListResponse = Schemas['StudentListResponse'];
export type ListStudentsQuery = NonNullable<
	operations['listStudents']['parameters']['query']
>;

export async function listStudents(query: ListStudentsQuery): Promise<StudentListResponse> {
	return requireApiData(
		await apiClient.get<StudentListResponse>('/api/students', { query: { ...query } }),
		'Failed to list students'
	);
}

export async function getStudent(id: string, academicYearId: string): Promise<Student> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getStudent']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<Student>(`/api/students/${encodeURIComponent(id)}`, { query }),
		'Failed to get student'
	);
}
```

Leave `getOwnProfile` unchanged until Task 7 so this task remains type-clean without changing learner pages.

- [ ] **Step 4: Make the staff route own `year_required` and subscribe list data**

Add `academicContext: 'year_required'` beside `menu` in `staff/students/+page.ts`. In the list page:

```ts
const academicContext = getAcademicContextStore();
const academicYearId = $derived($academicContext.selected.academicYearId);
const PAGE_SIZE = 20;
let hasNextPage = $state(false);
let revision = 0;

async function loadStudents(yearId: string) {
	const current = ++revision;
	loading = true;
	try {
		const result = await listStudents({
			academicYearId: yearId,
			page: currentPage,
			pageSize: PAGE_SIZE,
			search: searchQuery || undefined,
			status: statusFilter === 'all' ? undefined : statusFilter
		});
		if (current !== revision) return;
		students = result.items;
		currentPage = result.page;
		hasNextPage = result.items.length === result.page_size;
	} finally {
		if (current === revision) loading = false;
	}
}
```

Subscribe on mount to year changes, resetting `currentPage` to 1. Search/reset/page/delete reloads guard and pass `academicYearId`. Replace `class_room` with `homeroom`; remove fake totals and use `currentPage > 1` plus `hasNextPage`. Detail links preserve `academicYearId`.

- [ ] **Step 5: Subscribe detail and edit pages to year changes**

Each page calls `getStudent(studentId, yearId)`, uses a revision guard, and subscribes to `academicContext` on mount. The detail-page edit link preserves `academicYearId`; the edit page reloads the selected year after saving.

- [ ] **Step 6: Validate the three Svelte pages**

Run from `frontend-school`, one command at a time:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/students/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/students/[id]/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/students/[id]/edit/+page.svelte' --svelte-version 5
```

Apply relevant diagnostics and rerun each affected file until clean.

- [ ] **Step 7: Run focused tests and type checking**

Run one command at a time:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all PASS.

- [ ] **Step 8: Commit the staff route cutover**

```bash
git add frontend-school/src/lib/api/students.ts 'frontend-school/src/routes/(app)/staff/students/+page.ts' 'frontend-school/src/routes/(app)/staff/students/+page.svelte' 'frontend-school/src/routes/(app)/staff/students/[id]/+page.svelte' 'frontend-school/src/routes/(app)/staff/students/[id]/edit/+page.svelte' frontend-school/tests/static/api-query-contract.test.mjs frontend-school/tests/static/academic-context-contract.test.mjs
git commit -m "fix(students): scope staff views by academic year"
```

---

### Task 7: Add authorized year selection to student profile pages

**Files:**
- Modify: `frontend-school/src/lib/api/students.ts`
- Create: `frontend-school/src/lib/academic-context/scoped-year.ts`
- Create: `frontend-school/src/lib/components/academic-context/ScopedAcademicYearSelect.svelte`
- Modify: `frontend-school/src/routes/(app)/student/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/student/profile/+page.svelte`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`

**Interfaces:**
- Produces: direct `getOwnProfile(academicYearId): Promise<Student>`.
- Produces: `resolveScopedAcademicYearUrl`, `urlWithAcademicYear`, and controlled `ScopedAcademicYearSelect`.

- [ ] **Step 1: Add failing wrapper, resolver, and student-page assertions**

Record `getOwnProfile('year-1')` and require `/api/student/profile` with `{ academicYearId: 'year-1' }`. Add pure resolver assertions for a missing year, unauthorized year, and empty option list. Require both student pages to call `listMyAcademicContextOptions()` and `getOwnProfile(selectedYearId)`.

- [ ] **Step 2: Run focused tests and verify failure**

Run one command at a time:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

Expected: FAIL because the own-profile query and scoped resolver do not exist.

- [ ] **Step 3: Migrate the own-profile wrapper**

```ts
export async function getOwnProfile(academicYearId: string): Promise<Student> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getStudentProfile']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<Student>('/api/student/profile', { query }),
		'Failed to get profile'
	);
}
```

- [ ] **Step 4: Implement the pure scoped-year resolver**

```ts
import type { AcademicContextOptionsResponse } from '$lib/api/academic-context';

export type ScopedAcademicYearResolution = {
	academicYearId: string | null;
	replaceUrl: URL | null;
};

export function urlWithAcademicYear(url: URL, academicYearId: string): URL {
	const next = new URL(url);
	next.searchParams.set('academicYearId', academicYearId);
	next.searchParams.delete('academicTermId');
	return next;
}

export function resolveScopedAcademicYearUrl(
	options: AcademicContextOptionsResponse,
	url: URL
): ScopedAcademicYearResolution {
	if (options.years.length === 0) return { academicYearId: null, replaceUrl: null };
	const requested = url.searchParams.get('academicYearId');
	const valid = options.years.find((year) => year.id === requested)?.id;
	if (valid) return { academicYearId: valid, replaceUrl: null };
	const selected =
		options.years.find((year) => year.id === options.activeAcademicYearId)?.id ??
		options.years[0].id;
	return { academicYearId: selected, replaceUrl: urlWithAcademicYear(url, selected) };
}
```

- [ ] **Step 5: Implement the controlled year selector**

Create `ScopedAcademicYearSelect.svelte` with typed rune props `id`, `years: AcademicYearOption[]`, `value`, `disabled`, and `onchange(academicYearId)`. Render a keyed option list and forward the selected ID:

```svelte
<select
	{id}
	class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
	{value}
	{disabled}
	onchange={(event) => onchange(event.currentTarget.value)}
>
	{#each years as year (year.id)}
		<option value={year.id}>{year.name}</option>
	{/each}
</select>
```

- [ ] **Step 6: Wire both student pages**

On mount, load `listMyAcademicContextOptions`, resolve `page.url`, replace invalid/missing URLs with `goto`, and call `getOwnProfile(selectedYearId)`. Use a monotonically increasing revision and assign student/error/loading only for the current revision. Changing the selector updates the URL and reloads. Preserve `academicYearId` in dashboard links to profile and timetable. After profile save, reload the same year. Empty options render `ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้` without calling the profile endpoint.

- [ ] **Step 7: Validate Svelte files**

Run from `frontend-school`, one command at a time:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-context/ScopedAcademicYearSelect.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/student/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/student/profile/+page.svelte' --svelte-version 5
```

- [ ] **Step 8: Run focused tests and type checking**

Run one command at a time:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all PASS.

- [ ] **Step 9: Commit the student portal cutover**

```bash
git add frontend-school/src/lib/api/students.ts frontend-school/src/lib/academic-context/scoped-year.ts frontend-school/src/lib/components/academic-context/ScopedAcademicYearSelect.svelte 'frontend-school/src/routes/(app)/student/+page.svelte' 'frontend-school/src/routes/(app)/student/profile/+page.svelte' frontend-school/tests/static/api-query-contract.test.mjs frontend-school/tests/static/academic-context-contract.test.mjs
git commit -m "fix(student): select authorized academic year"
```

---

### Task 8: Add linked-year selection to parent pages

**Files:**
- Modify: `frontend-school/src/lib/api/parents.ts`
- Modify: `frontend-school/src/routes/(app)/parent/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/parent/student/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/parent/student/[id]/timetable/+page.svelte`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`

**Interfaces:**
- Produces: direct `getOwnParentProfile(academicYearId)`, `getChildProfile(studentId, academicYearId)`, and typed-query `getChildTimetable(studentId, academicTermId)`.
- Consumes: parent/child context option wrappers and Task 7 scoped-year UI.

- [ ] **Step 1: Add failing parent-wrapper and page assertions**

Record and assert `/api/parent/profile`, `/api/parent/students/student-1`, and `/api/parent/students/student-1/timetable` receive generated camelCase query objects. Require parent home to use `listParentAcademicContextOptions`, child pages to use `listChildAcademicContextOptions`, and all profile calls to include `selectedYearId`.

- [ ] **Step 2: Run focused tests and verify failure**

Run one command at a time:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

Expected: FAIL because parent wrappers omit the year and timetable still embeds its query string.

- [ ] **Step 3: Migrate all three parent wrappers**

Use each generated operation query type:

```ts
export async function getOwnParentProfile(academicYearId: string): Promise<ParentProfile> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getParentProfile']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<ParentProfile>('/api/parent/profile', { query }),
		'Failed to get parent profile'
	);
}

export async function getChildProfile(studentId: string, academicYearId: string): Promise<Student> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getParentChildProfile']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<Student>(`/api/parent/students/${encodeURIComponent(studentId)}`, { query }),
		'Failed to get student profile'
	);
}
```

For timetable, validate the trimmed term ID, construct `operations['getParentChildTimetable']['parameters']['query']`, and use `apiClient.get(path, { query })`.

- [ ] **Step 4: Wire parent home and child detail**

Parent home loads `listParentAcademicContextOptions`, resolves with `resolveScopedAcademicYearUrl`, and calls `getOwnParentProfile(selectedYearId)`. Child cards preserve the year in their detail URL.

Child detail loads `listChildAcademicContextOptions(studentId)`, resolves the year, and calls `getChildProfile(studentId, selectedYearId)`. Parent back and timetable links preserve the selected year. Both pages render `ScopedAcademicYearSelect`, use revision guards, and avoid profile requests when no linked academic year exists.

- [ ] **Step 5: Correct timetable ordering and revision ownership**

Resolve options and selection before loading the profile. Make `loadTimetable` accept the caller's revision:

```ts
async function loadTimetable(termId: string, current = ++revision): Promise<void> {
	const loaded = await getChildTimetable(studentId, termId);
	if (current !== revision) return;
	periods = periodsFromTimetableEntries(loaded);
	entries = loaded;
}
```

`loadHistory` begins with `const current = ++revision`, then loads options, assigns the selected year/term, loads `getChildProfile(studentId, selectedYearId)`, and finally calls `loadTimetable(selectedTermId, current)`. Changing the year also begins one revision, reloads the year-scoped child profile, then loads the chosen term with the same revision.

- [ ] **Step 6: Validate parent Svelte pages**

Run from `frontend-school`, one command at a time:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/parent/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/parent/student/[id]/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/parent/student/[id]/timetable/+page.svelte' --svelte-version 5
```

- [ ] **Step 7: Run focused tests and type checking**

Run one command at a time:

```bash
node --test frontend-school/tests/static/api-query-contract.test.mjs
```

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all PASS with no missing required academic-year calls.

- [ ] **Step 8: Commit the parent portal cutover**

```bash
git add frontend-school/src/lib/api/parents.ts 'frontend-school/src/routes/(app)/parent/+page.svelte' 'frontend-school/src/routes/(app)/parent/student/[id]/+page.svelte' 'frontend-school/src/routes/(app)/parent/student/[id]/timetable/+page.svelte' frontend-school/tests/static/api-query-contract.test.mjs frontend-school/tests/static/academic-context-contract.test.mjs
git commit -m "fix(parent): select linked academic years"
```

---

### Task 9: Run the checkpoint verification matrix

**Files:**
- Verify only; modify touched files only when a check exposes a real defect.

**Interfaces:**
- Produces: a deployable Phase B repair checkpoint with synchronized backend runtime, OpenAPI, generated types, wrappers, and role-scoped pages.

- [ ] **Step 1: Run focused backend database tests**

From the repository root:

```bash
./scripts/test_backend_school.sh modules::parents::services::tests::parent_academic_context_contains_only_linked_student_years -- --exact
```

- [ ] **Step 2: Run backend formatting, architecture, and compilation checks**

From `backend-school`, one command at a time:

```bash
cargo fmt --all -- --check
```

```bash
cargo test --test static_architecture
```

```bash
cargo test api_contract::tests::documents_academic_year_scoped_profile_and_calendar_queries -- --exact
```

```bash
cargo check
```

- [ ] **Step 3: Run API contract generation checks**

From `frontend-school`, one command at a time:

```bash
npm run generate:api-contracts
```

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

- [ ] **Step 4: Run frontend checks**

From `frontend-school`, one command at a time:

```bash
npm run lint
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
npm run test:static
```

- [ ] **Step 5: Review repository integrity**

From the repository root, one command at a time:

```bash
git diff --check
```

```bash
git status --short
```

```bash
git log --oneline -8
```

Confirm there are no migrations, permission artifacts, plaintext identifiers, compatibility query aliases, unrelated edits, or uncommitted generated changes.

- [ ] **Step 6: Push and verify production only after explicit user authorization**

After the user approves the verified commits, push `main`. Wait for each deployment workflow rather than launching local commands concurrently. In authenticated staff, student, and parent sessions verify:

- staff dashboard and `/staff/students` load without HTTP 400;
- changing staff Topbar year reloads list/detail data and retains the year in the URL;
- student dashboard/profile can select an authorized year;
- parent home/child detail/timetable can select only linked years;
- calendar and grade-level calls emit camelCase queries;
- browser Network responses contain no `Failed to deserialize query string` message.

Record any new production mismatch as a failing contract test before changing code.
