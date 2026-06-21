# Daily Teaching Overview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only whole-school daily teaching table at `/staff/academic/timetable/today` so staff can see which teachers teach each period, class, subject, and room for a selected date.

**Architecture:** Add a dedicated `GET /api/academic/timetable/daily-teaching` endpoint authorized by either `academic_timetable_today.read.school` or existing course-plan read access. A new academic service queries timetable rows, active teachers, and periods, then groups them into typed camelCase DTOs for a table-first Svelte page.

**Tech Stack:** Rust + Axum + sqlx backend, SvelteKit 5 + TypeScript frontend, existing `ApiResponse` envelope, `apiClient`, permission registry, `PageShell`, app-state components, and local shadcn-svelte UI primitives.

---

## File Map

- Create: `backend-school/migrations/011_daily_teaching_overview_permission.sql`
  - Inserts the new read-only school-scoped permission for clean tenant upgrades.
- Modify: `backend-school/src/permissions/registry.rs`
  - Adds `ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL` and `PermissionDef`.
- Modify: `frontend-school/src/lib/permissions/registry.ts`
  - Adds `PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY` and `PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL`.
- Create: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
  - Owns typed daily overview DTOs, query structs, date/day helpers, grouping logic, SQL loading, and unit tests.
- Modify: `backend-school/src/modules/academic/services.rs`
  - Re-exports `daily_teaching_service`.
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
  - Adds a thin `daily_teaching_overview` handler.
- Modify: `backend-school/src/modules/academic.rs`
  - Registers `/timetable/daily-teaching` before dynamic `/timetable/{id}` routes.
- Modify: `backend-school/tests/static_architecture.rs`
  - Adds static guards for route registration, authorization, typed DTOs, and PII-safe SQL.
- Modify: `frontend-school/src/lib/api/timetable.ts`
  - Adds daily overview interfaces and `getDailyTeachingOverview()`.
- Create: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts`
  - Adds real menu metadata under the academic timetable section.
- Create: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`
  - Builds the table-first desktop/mobile daily teaching overview page.
- Modify: `frontend-school/src/lib/components/layout/sidebar-navigation.ts`
  - Groups the new menu route with the timetable sidebar section.
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
  - Adds API contract checks for typed daily overview backend/frontend wrappers.
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`
  - Adds page/menu/static UI expectations for the new route.

---

### Task 1: Permission Registry And Migration

**Files:**
- Create: `backend-school/migrations/011_daily_teaching_overview_permission.sql`
- Modify: `backend-school/src/permissions/registry.rs`
- Modify: `frontend-school/src/lib/permissions/registry.ts`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`

- [ ] **Step 1: Add a failing static expectation for the new permission**

Add this expectation near the existing permission-registry static checks in `frontend-school/tests/static/api-global-contract.test.mjs`:

```js
test('daily teaching overview permission is registered across backend and frontend', async () => {
	const backendRegistry = await readFile(
		path.join(repoRoot, 'backend-school/src/permissions/registry.rs'),
		'utf8'
	);
	const frontendRegistry = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/permissions/registry.ts'),
		'utf8'
	);
	const migration = await readFile(
		path.join(repoRoot, 'backend-school/migrations/011_daily_teaching_overview_permission.sql'),
		'utf8'
	);

	for (const source of [backendRegistry, frontendRegistry, migration]) {
		assert.match(source, /academic_timetable_today\.read\.school/);
		assert.match(source, /academic_timetable_today/);
	}
});
```

- [ ] **Step 2: Run the static test and verify it fails**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "daily teaching overview permission"
```

Expected: FAIL because the migration file and permission constants do not exist yet.

- [ ] **Step 3: Add the backend permission constant and definition**

In `backend-school/src/permissions/registry.rs`, add:

```rust
pub const ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL: &str =
    "academic_timetable_today.read.school";
```

Add this `PermissionDef` after the course-plan permissions:

```rust
PermissionDef {
    code: codes::ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
    name: "ดูตารางสอนวันนี้ทั้งโรงเรียน",
    module: "academic_timetable_today",
    action: "read",
    scope: "school",
    description: "ดูภาพรวมตารางสอนรายวันของครูทั้งโรงเรียนแบบอ่านอย่างเดียว",
},
```

- [ ] **Step 4: Add the frontend permission constants**

In `frontend-school/src/lib/permissions/registry.ts`, add:

```ts
ACADEMIC_TIMETABLE_TODAY: 'academic_timetable_today',
```

and:

```ts
ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL: 'academic_timetable_today.read.school',
```

- [ ] **Step 5: Add the migration**

Create `backend-school/migrations/011_daily_teaching_overview_permission.sql`:

```sql
-- Read-only daily teaching overview permission.

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES (
    'academic_timetable_today.read.school',
    'ดูตารางสอนวันนี้ทั้งโรงเรียน',
    'academic_timetable_today',
    'read',
    'school',
    'ดูภาพรวมตารางสอนรายวันของครูทั้งโรงเรียนแบบอ่านอย่างเดียว'
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description;
```

- [ ] **Step 6: Run the static test and verify it passes**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "daily teaching overview permission"
```

Expected: PASS.

---

### Task 2: Backend Daily Teaching Service

**Files:**
- Create: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Modify: `backend-school/src/modules/academic/services.rs`

- [ ] **Step 1: Write failing service tests**

Create `backend-school/src/modules/academic/services/daily_teaching_service.rs` with tests first:

```rust
use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn day_code_from_date_maps_chrono_weekdays_to_timetable_codes() {
        let monday = NaiveDate::from_ymd_opt(2026, 6, 22).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2026, 6, 28).unwrap();

        assert_eq!(day_code_from_date(monday), "MON");
        assert_eq!(day_code_from_date(sunday), "SUN");
    }

    #[test]
    fn build_overview_groups_team_teaching_entry_under_each_assigned_teacher() {
        let period_id = id(1);
        let semester_id = id(2);
        let entry_id = id(3);
        let teacher_a = id(10);
        let teacher_b = id(11);

        let overview = build_daily_teaching_overview(
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
            "MON".to_string(),
            semester_id,
            vec![DailyTeachingPeriod {
                id: period_id,
                name: Some("คาบ 1".to_string()),
                start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                order_index: 1,
            }],
            vec![
                DailyTeachingTeacherSeed {
                    id: teacher_a,
                    display_name: "ครูก".to_string(),
                    organization_unit_names: vec!["คณิตศาสตร์".to_string()],
                    sort_order: 10,
                },
                DailyTeachingTeacherSeed {
                    id: teacher_b,
                    display_name: "ครูข".to_string(),
                    organization_unit_names: vec!["คณิตศาสตร์".to_string()],
                    sort_order: 10,
                },
            ],
            vec![
                DailyTeachingEntrySeed {
                    teacher_id: teacher_a,
                    period_id,
                    entry_id,
                    entry_type: "COURSE".to_string(),
                    subject_code: Some("ค21101".to_string()),
                    subject_name: Some("คณิตศาสตร์".to_string()),
                    subject_group_name: Some("คณิตศาสตร์".to_string()),
                    classroom_name: Some("ม.1/1".to_string()),
                    room_code: Some("321".to_string()),
                    title: None,
                    note: None,
                    instructor_count: 2,
                    period_order_index: 1,
                },
                DailyTeachingEntrySeed {
                    teacher_id: teacher_b,
                    period_id,
                    entry_id,
                    entry_type: "COURSE".to_string(),
                    subject_code: Some("ค21101".to_string()),
                    subject_name: Some("คณิตศาสตร์".to_string()),
                    subject_group_name: Some("คณิตศาสตร์".to_string()),
                    classroom_name: Some("ม.1/1".to_string()),
                    room_code: Some("321".to_string()),
                    title: None,
                    note: None,
                    instructor_count: 2,
                    period_order_index: 1,
                },
            ],
            false,
        );

        assert_eq!(overview.summary.total_teacher_count, 2);
        assert_eq!(overview.summary.teachers_teaching_count, 2);
        assert_eq!(overview.summary.lesson_count, 2);
        assert_eq!(overview.teachers.len(), 2);
        assert!(overview.teachers[0].periods[0].entries[0].is_team_teaching);
        assert!(overview.teachers[1].periods[0].entries[0].is_team_teaching);
    }

    #[test]
    fn build_overview_includes_empty_teachers_only_when_requested() {
        let period_id = id(1);
        let semester_id = id(2);
        let teacher_with_lesson = id(10);
        let empty_teacher = id(11);
        let period = DailyTeachingPeriod {
            id: period_id,
            name: Some("คาบ 1".to_string()),
            start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            order_index: 1,
        };
        let teachers = vec![
            DailyTeachingTeacherSeed {
                id: teacher_with_lesson,
                display_name: "ครูมีคาบ".to_string(),
                organization_unit_names: vec![],
                sort_order: 0,
            },
            DailyTeachingTeacherSeed {
                id: empty_teacher,
                display_name: "ครูว่าง".to_string(),
                organization_unit_names: vec![],
                sort_order: 0,
            },
        ];
        let entries = vec![DailyTeachingEntrySeed {
            teacher_id: teacher_with_lesson,
            period_id,
            entry_id: id(3),
            entry_type: "HOMEROOM".to_string(),
            subject_code: None,
            subject_name: None,
            subject_group_name: None,
            classroom_name: Some("ม.1/1".to_string()),
            room_code: None,
            title: Some("โฮมรูม".to_string()),
            note: None,
            instructor_count: 1,
            period_order_index: 1,
        }];

        let without_empty = build_daily_teaching_overview(
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
            "MON".to_string(),
            semester_id,
            vec![period.clone()],
            teachers.clone(),
            entries.clone(),
            false,
        );
        let with_empty = build_daily_teaching_overview(
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
            "MON".to_string(),
            semester_id,
            vec![period],
            teachers,
            entries,
            true,
        );

        assert_eq!(without_empty.teachers.len(), 1);
        assert_eq!(without_empty.summary.empty_teacher_count, 1);
        assert_eq!(with_empty.teachers.len(), 2);
        assert_eq!(with_empty.summary.displayed_teacher_count, 2);
    }
}
```

Add the module export in `backend-school/src/modules/academic/services.rs`:

```rust
pub mod daily_teaching_service;
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
cd backend-school
cargo test modules::academic::services::daily_teaching_service::tests --bin backend-school
```

Expected: FAIL to compile because the helper functions and DTOs are not implemented yet.

- [ ] **Step 3: Implement typed DTOs, helpers, and SQL service**

Implement `DailyTeachingQuery`, `DailyTeachingOverview`, `DailyTeachingPeriod`, `DailyTeachingTeacher`, `DailyTeachingPeriodCell`, `DailyTeachingEntry`, `DailyTeachingSummary`, seed structs, `day_code_from_date()`, `build_daily_teaching_overview()`, `resolve_semester_id()`, `list_periods_for_semester()`, `list_daily_teachers()`, `list_daily_entries()`, and:

```rust
pub async fn get_daily_teaching_overview(
    pool: &PgPool,
    query: DailyTeachingQuery,
    include_empty_teachers_allowed: bool,
) -> Result<DailyTeachingOverview, AppError>
```

The SQL must select only operational fields:

```sql
SELECT
    tei.instructor_id AS teacher_id,
    te.period_id,
    te.id AS entry_id,
    te.entry_type,
    s.code AS subject_code,
    s.name_th AS subject_name,
    sg.name AS subject_group_name,
    cr.name AS classroom_name,
    r.code AS room_code,
    te.title,
    te.note,
    COUNT(*) OVER (PARTITION BY te.id) AS instructor_count,
    ap.order_index AS period_order_index
FROM academic_timetable_entries te
JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
JOIN academic_periods ap ON ap.id = te.period_id
LEFT JOIN classroom_courses cc ON cc.id = te.classroom_course_id
LEFT JOIN subjects s ON s.id = cc.subject_id
LEFT JOIN subject_groups sg ON sg.id = s.subject_group_id
LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
LEFT JOIN rooms r ON r.id = te.room_id
WHERE te.is_active = true
  AND te.academic_semester_id = $1
  AND te.day_of_week = $2
  AND te.entry_type = ANY($3)
ORDER BY ap.order_index, tei.created_at, te.id
```

The teacher SQL must select `users.id`, `concat(users.first_name, ' ', users.last_name)`, active organization unit names, and a minimum organization display order. It must not select `national_id`, `phone`, `email`, `username`, `address`, or roster/student fields.

- [ ] **Step 4: Run the focused tests and verify they pass**

Run:

```bash
cd backend-school
cargo test modules::academic::services::daily_teaching_service::tests --bin backend-school
```

Expected: PASS.

---

### Task 3: Backend Handler, Route, And Static Guard

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/tests/static_architecture.rs`

- [ ] **Step 1: Add a failing static guard**

Add a backend static test:

```rust
#[test]
fn daily_teaching_overview_endpoint_is_read_only_and_pii_safe() {
    let routes = strip_comments(&read_source(manifest_dir().join("src/modules/academic.rs")));
    let handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/handlers/timetable.rs"),
    ));
    let service = strip_comments(&read_source(
        manifest_dir().join("src/modules/academic/services/daily_teaching_service.rs"),
    ));
    let registry = read_source(manifest_dir().join("src/permissions/registry.rs"));

    assert!(routes.contains("\"/timetable/daily-teaching\""));
    assert!(routes.contains("get(handlers::timetable::daily_teaching_overview)"));
    assert!(
        routes.find("\"/timetable/daily-teaching\"") < routes.find("\"/timetable/{id}\""),
        "daily teaching route must be registered before /timetable/{id}"
    );

    assert!(handler.contains("actor_tenant_context(&state, &headers).await?"));
    assert!(handler.contains("ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL"));
    assert!(handler.contains("ACADEMIC_COURSE_PLAN_READ_ALL"));
    assert!(!handler.contains("ACADEMIC_COURSE_PLAN_MANAGE_ALL"));
    assert!(handler.contains("daily_teaching_service::get_daily_teaching_overview"));
    assert!(handler.contains("ApiResponse::ok(data)"));

    assert!(service.contains("#[serde(rename_all = \"camelCase\")]"));
    assert!(service.contains("DailyTeachingOverview"));
    assert!(service.contains("timetable_entry_instructors"));
    assert!(service.contains("subject_group_name"));
    assert!(registry.contains("academic_timetable_today.read.school"));

    for forbidden in ["national_id", "phone", "email", "username", "address", "student_"] {
        assert!(
            !service.contains(forbidden),
            "daily teaching service must not expose or select `{forbidden}`"
        );
    }
}
```

- [ ] **Step 2: Run the static guard and verify it fails**

Run:

```bash
cd backend-school
cargo test --test static_architecture daily_teaching_overview_endpoint_is_read_only_and_pii_safe
```

Expected: FAIL because the route and handler are not implemented yet.

- [ ] **Step 3: Add the handler**

Add to `backend-school/src/modules/academic/handlers/timetable.rs`:

```rust
/// GET /api/academic/timetable/daily-teaching
pub async fn daily_teaching_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<daily_teaching_service::DailyTeachingQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;

    actor.require_any_permission(&[
        codes::ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
    ])?;

    let include_empty_teachers_allowed =
        actor.has_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL);
    let data = daily_teaching_service::get_daily_teaching_overview(
        &pool,
        query,
        include_empty_teachers_allowed,
    )
    .await?;

    Ok(Json(ApiResponse::ok(data)).into_response())
}
```

Import `daily_teaching_service` from `crate::modules::academic::services`.

- [ ] **Step 4: Register the route**

In `backend-school/src/modules/academic.rs`, add before `/timetable/{id}`:

```rust
.route(
    "/timetable/daily-teaching",
    get(handlers::timetable::daily_teaching_overview),
)
```

- [ ] **Step 5: Run backend focused checks**

Run:

```bash
cd backend-school
cargo test --test static_architecture daily_teaching_overview_endpoint_is_read_only_and_pii_safe
cargo test modules::academic::services::daily_teaching_service::tests --bin backend-school
```

Expected: PASS.

---

### Task 4: Frontend API, Route Metadata, And Page

**Files:**
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`
- Modify: `frontend-school/src/lib/components/layout/sidebar-navigation.ts`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`

- [ ] **Step 1: Add failing frontend API and page static tests**

In `frontend-school/tests/static/api-response-contract.test.mjs`, add:

```js
test('daily teaching overview API uses typed response contracts', async () => {
	const frontendTimetableApi = await readRepoFile('frontend-school/src/lib/api/timetable.ts');
	const backendService = await readRepoFile(
		'backend-school/src/modules/academic/services/daily_teaching_service.rs'
	);
	const backendHandler = await readRepoFile(
		'backend-school/src/modules/academic/handlers/timetable.rs'
	);

	assert.match(frontendTimetableApi, /interface\s+DailyTeachingOverview/);
	assert.match(frontendTimetableApi, /interface\s+DailyTeachingTeacher/);
	assert.match(frontendTimetableApi, /interface\s+DailyTeachingEntry/);
	assert.match(frontendTimetableApi, /getDailyTeachingOverview/);
	assert.match(frontendTimetableApi, /apiClient\.get<DailyTeachingOverview>/);
	assert.match(frontendTimetableApi, /\/api\/academic\/timetable\/daily-teaching/);
	assert.match(backendService, /struct\s+DailyTeachingOverview/);
	assert.match(backendService, /#\[serde\(rename_all = "camelCase"\)\]/);
	assert.match(backendHandler, /ApiResponse::ok\(data\)/);
});
```

In `frontend-school/tests/static/api-global-contract.test.mjs`, extend the academic timetable static test with a route expectation:

```js
{
	file: 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte',
	imports: ['$lib/components/app-state', '$lib/components/app-layout'],
	permissions: [
		'PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL',
		'PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL',
		'PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL'
	],
	identifiers: [
		'getDailyTeachingOverview',
		'canReadDailyTeaching',
		'canUseAcademicFilters',
		'includeEmptyTeachers'
	]
}
```

Add a page-specific static test:

```js
test('daily teaching overview page is table based and read only', async () => {
	const page = stripComments(
		await readFile(
			path.join(
				repoRoot,
				'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
			),
			'utf8'
		)
	);
	const meta = stripComments(
		await readFile(
			path.join(
				repoRoot,
				'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts'
			),
			'utf8'
		)
	);
	const sidebar = stripComments(
		await readFile(
			path.join(repoRoot, 'frontend-school/src/lib/components/layout/sidebar-navigation.ts'),
			'utf8'
		)
	);

	assert.match(meta, /PERMISSION_MODULES\.ACADEMIC_TIMETABLE_TODAY/);
	assert.match(meta, /title:\s*'ตารางสอนวันนี้'/);
	assert.match(page, /<PageShell/);
	assert.match(page, /<PageSkeleton\s+variant="table"/);
	assert.match(page, /<PageState/);
	assert.match(page, /\* as Table/);
	assert.match(page, /sticky left-0/);
	assert.match(page, /overflow-x-auto/);
	assert.match(page, /Dialog\.Root/);
	assert.match(page, /href="\/staff\/academic\/timetable"/);
	assert.match(sidebar, /\/staff\/academic\/timetable\/today/);
	assert.doesNotMatch(page, /listStaff\(|listStudents\(|createTimetableEntry|updateTimetableEntry|deleteTimetableEntry/);
});
```

- [ ] **Step 2: Run the frontend static tests and verify they fail**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "daily teaching"
```

Expected: FAIL because the API wrapper and route do not exist yet.

- [ ] **Step 3: Add frontend API types and wrapper**

In `frontend-school/src/lib/api/timetable.ts`, add typed interfaces:

```ts
export interface DailyTeachingPeriod {
	id: string;
	name: string | null;
	startTime: string;
	endTime: string;
	orderIndex: number;
}

export interface DailyTeachingEntry {
	entryId: string;
	entryType: 'COURSE' | 'BREAK' | 'ACTIVITY' | 'HOMEROOM' | 'ACADEMIC';
	subjectCode: string | null;
	subjectName: string | null;
	subjectGroupName: string | null;
	classroomName: string | null;
	roomCode: string | null;
	title: string | null;
	note: string | null;
	isTeamTeaching: boolean;
}

export interface DailyTeachingPeriodCell {
	periodId: string;
	entries: DailyTeachingEntry[];
}

export interface DailyTeachingTeacher {
	id: string;
	displayName: string;
	organizationUnitNames: string[];
	periods: DailyTeachingPeriodCell[];
}

export interface DailyTeachingSummary {
	totalTeacherCount: number;
	displayedTeacherCount: number;
	teachersTeachingCount: number;
	lessonCount: number;
	emptyTeacherCount: number;
}

export interface DailyTeachingOverview {
	date: string;
	dayOfWeek: string;
	academicSemesterId: string;
	periods: DailyTeachingPeriod[];
	teachers: DailyTeachingTeacher[];
	summary: DailyTeachingSummary;
}
```

Add:

```ts
export const getDailyTeachingOverview = async (
	filters: {
		date?: string;
		academicSemesterId?: string;
		includeEmptyTeachers?: boolean;
	} = {}
): Promise<LoadedApiResponse<DailyTeachingOverview>> => {
	const params = new URLSearchParams();
	if (filters.date) params.append('date', filters.date);
	if (filters.academicSemesterId) params.append('academic_semester_id', filters.academicSemesterId);
	if (filters.includeEmptyTeachers) params.append('include_empty_teachers', 'true');

	const queryString = params.toString() ? `?${params.toString()}` : '';
	const response = await apiClient.get<DailyTeachingOverview>(
		`/api/academic/timetable/daily-teaching${queryString}`
	);
	const data = requireApiData(response, 'ไม่สามารถโหลดตารางสอนวันนี้ได้');
	return { success: true, data, message: response.message };
};
```

- [ ] **Step 4: Add route metadata and sidebar grouping**

Create `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts`:

```ts
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตารางสอนวันนี้',
		icon: 'CalendarClock',
		group: 'academic',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY,
		order: 50,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
```

Add `/staff/academic/timetable/today` to the `academic-timetable` paths in `frontend-school/src/lib/components/layout/sidebar-navigation.ts`.

- [ ] **Step 5: Build the Svelte page**

Create `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte` with:

- `PageShell` title `ตารางสอนวันนี้`.
- Date input, previous/next buttons, semester select, refresh button.
- Search input for every user.
- Academic-only organization/subject/classroom selects derived from loaded data.
- Academic-only switch for `includeEmptyTeachers`.
- Print button and planner link for academic users.
- Summary row.
- `Table.Root` inside `overflow-x-auto`.
- Sticky teacher column using `sticky left-0`.
- Sticky header row using `sticky top-0`.
- Compact chips per occupied cell.
- `Dialog.Root` for tapped cell details.
- No timetable mutations and no generic staff/student list APIs.

- [ ] **Step 6: Run frontend static tests**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "daily teaching"
```

Expected: PASS.

---

### Task 5: Full Verification, Commit, And Push

**Files:**
- All modified files from Tasks 1-4.

- [ ] **Step 1: Run backend checks**

Run:

```bash
cd backend-school
cargo test modules::academic::services::daily_teaching_service::tests --bin backend-school
cargo test --test static_architecture
cargo check
```

Expected: all pass.

- [ ] **Step 2: Run frontend checks**

Run:

```bash
cd frontend-school
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all pass with zero Svelte/TypeScript errors.

- [ ] **Step 3: Run repository diff checks**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; status shows only intentional files.

- [ ] **Step 4: Commit and push**

Run:

```bash
git add backend-school frontend-school docs/superpowers/plans/2026-06-22-daily-teaching-overview.md
git commit -m "Add daily teaching overview"
git push origin main
```

Expected: push succeeds.
