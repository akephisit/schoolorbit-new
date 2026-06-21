# Staff Dashboard Overview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a real `/staff` operational overview dashboard that shows school-wide aggregate counts to every active staff user without exposing staff/student lists or PII.

**Architecture:** Add a dedicated authenticated aggregate endpoint at `GET /api/staff/dashboard`, backed by a small staff dashboard service. The frontend consumes that typed endpoint from `frontend-school/src/lib/api/staff.ts` and replaces placeholder stats on `/staff` with real loading, error, and overview states while keeping shortcuts permission-filtered.

**Tech Stack:** Rust + Axum + sqlx backend, SvelteKit 5 + TypeScript + shadcn-svelte/Tailwind frontend, existing `ApiResponse` envelope and `apiClient`.

---

## File Map

- Create: `backend-school/src/modules/staff/services/dashboard_service.rs`
  - Owns dashboard aggregate SQL, staff-user verification, typed dashboard response DTO, and service-level unit tests.
- Modify: `backend-school/src/modules/staff/services.rs`
  - Re-export `dashboard_service`.
- Modify: `backend-school/src/modules/staff/handlers/staff.rs`
  - Add thin `get_staff_dashboard` handler.
- Modify: `backend-school/src/main.rs`
  - Register `GET /api/staff/dashboard` before `GET /api/staff/{id}`.
- Modify: `backend-school/tests/static_architecture.rs`
  - Add a static guard that the dashboard endpoint exists, stays aggregate-only, and does not reuse list/profile data contracts.
- Modify: `frontend-school/src/lib/api/staff.ts`
  - Add `StaffDashboardOverview` interface and `getStaffDashboard()`.
- Modify: `frontend-school/src/routes/(app)/staff/+page.svelte`
  - Replace placeholder stats with real data loading and redesign the operational overview.
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
  - Add frontend API contract assertions for the typed dashboard endpoint.
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`
  - Extend the existing staff dashboard static check to require real dashboard loading and prevent placeholder stats/list API usage.

---

### Task 1: Backend Dashboard Service

**Files:**
- Create: `backend-school/src/modules/staff/services/dashboard_service.rs`
- Modify: `backend-school/src/modules/staff/services.rs`

- [ ] **Step 1: Write the failing service test and register the module**

Create `backend-school/src/modules/staff/services/dashboard_service.rs` with only the test below:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_response_from_counts_maps_snake_case_row_to_camel_case_dto_fields() {
        let row = DashboardCountRow {
            total_staff: 84,
            total_students: 1248,
            active_classrooms: 42,
        };

        let response = dashboard_response_from_counts(row);

        assert_eq!(response.total_staff, 84);
        assert_eq!(response.total_students, 1248);
        assert_eq!(response.active_classrooms, 42);
    }
}
```

Add the module export in `backend-school/src/modules/staff/services.rs`:

```rust
pub mod dashboard_service;
pub mod organization_delegation_service;
pub mod organization_member_service;
pub mod organization_permission_service;
pub mod organization_unit_service;
pub mod permission_service;
pub mod role_service;
pub mod staff_service;
pub mod user_role_service;
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```bash
cd backend-school
cargo test modules::staff::services::dashboard_service::tests::dashboard_response_from_counts_maps_snake_case_row_to_camel_case_dto_fields --bin backend-school
```

Expected: FAIL to compile because `DashboardCountRow` and `dashboard_response_from_counts` are not defined yet.

- [ ] **Step 3: Implement the minimal typed service**

Replace `backend-school/src/modules/staff/services/dashboard_service.rs` with:

```rust
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StaffDashboardOverview {
    pub total_staff: i64,
    pub total_students: i64,
    pub active_classrooms: i64,
}

#[derive(Debug, FromRow)]
struct DashboardCountRow {
    total_staff: i64,
    total_students: i64,
    active_classrooms: i64,
}

fn dashboard_response_from_counts(row: DashboardCountRow) -> StaffDashboardOverview {
    StaffDashboardOverview {
        total_staff: row.total_staff,
        total_students: row.total_students,
        active_classrooms: row.active_classrooms,
    }
}

pub async fn ensure_active_staff_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let is_active_staff: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM users
            WHERE id = $1
              AND user_type = 'staff'
              AND status = 'active'
        )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to verify active staff dashboard user: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ใช้งานได้".to_string())
    })?;

    if is_active_staff {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "แดชบอร์ดนี้สำหรับบุคลากรที่ใช้งานอยู่เท่านั้น".to_string(),
        ))
    }
}

pub async fn get_staff_dashboard(pool: &PgPool) -> Result<StaffDashboardOverview, AppError> {
    let row = sqlx::query_as::<_, DashboardCountRow>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM users WHERE user_type = 'staff' AND status = 'active') AS total_staff,
            (SELECT COUNT(*) FROM users WHERE user_type = 'student' AND status = 'active') AS total_students,
            (SELECT COUNT(*) FROM class_rooms WHERE is_active = true) AS active_classrooms
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load staff dashboard counts: {}", error);
        AppError::InternalServerError("ไม่สามารถโหลดภาพรวมโรงเรียนได้".to_string())
    })?;

    Ok(dashboard_response_from_counts(row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_response_from_counts_maps_snake_case_row_to_camel_case_dto_fields() {
        let row = DashboardCountRow {
            total_staff: 84,
            total_students: 1248,
            active_classrooms: 42,
        };

        let response = dashboard_response_from_counts(row);

        assert_eq!(response.total_staff, 84);
        assert_eq!(response.total_students, 1248);
        assert_eq!(response.active_classrooms, 42);
    }
}
```

- [ ] **Step 4: Run the focused test and verify it passes**

Run:

```bash
cd backend-school
cargo test modules::staff::services::dashboard_service::tests::dashboard_response_from_counts_maps_snake_case_row_to_camel_case_dto_fields --bin backend-school
```

Expected: PASS.

- [ ] **Step 5: Commit the backend service**

Run:

```bash
git add backend-school/src/modules/staff/services.rs backend-school/src/modules/staff/services/dashboard_service.rs
git commit -m "Add staff dashboard aggregate service"
```

---

### Task 2: Backend Handler, Route, And Static Guard

**Files:**
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `backend-school/src/modules/staff/handlers/staff.rs`
- Modify: `backend-school/src/main.rs`

- [ ] **Step 1: Write the failing static architecture test**

Add this test near the existing staff access tests in `backend-school/tests/static_architecture.rs`:

```rust
#[test]
fn staff_dashboard_endpoint_is_staff_scoped_and_aggregate_only() {
    let routes = strip_comments(&read_source(manifest_dir().join("src/main.rs")));
    let staff_handler = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/handlers/staff.rs"),
    ));
    let dashboard_service = strip_comments(&read_source(
        manifest_dir().join("src/modules/staff/services/dashboard_service.rs"),
    ));

    assert!(routes.contains("\"/api/staff/dashboard\""));
    assert!(routes.contains("get(modules::staff::handlers::staff::get_staff_dashboard)"));
    assert!(
        routes.find("\"/api/staff/dashboard\"") < routes.find("\"/api/staff/{id}\""),
        "dashboard route must be registered before /api/staff/{id}"
    );

    assert!(staff_handler.contains("dashboard_service::ensure_active_staff_user"));
    assert!(staff_handler.contains("dashboard_service::get_staff_dashboard"));
    assert!(staff_handler.contains("ApiResponse::ok(data)"));
    assert!(!staff_handler.contains("actor.require_permission(codes::STAFF_READ_ALL)"));

    assert!(dashboard_service.contains("COUNT(*)"));
    assert!(dashboard_service.contains("user_type = 'staff'"));
    assert!(dashboard_service.contains("user_type = 'student'"));
    assert!(dashboard_service.contains("class_rooms"));

    for forbidden in [
        "national_id",
        "phone",
        "email",
        "first_name",
        "last_name",
        "staff_service::list_staff",
        "student_service::list_students",
    ] {
        assert!(
            !dashboard_service.contains(forbidden),
            "staff dashboard aggregate service must not expose or select `{forbidden}`"
        );
    }
}
```

- [ ] **Step 2: Run the static test and verify it fails**

Run:

```bash
cd backend-school
cargo test staff_dashboard_endpoint_is_staff_scoped_and_aggregate_only --test static_architecture
```

Expected: FAIL because `/api/staff/dashboard` and `get_staff_dashboard` do not exist yet.

- [ ] **Step 3: Implement the thin handler**

In `backend-school/src/modules/staff/handlers/staff.rs`, update the service import:

```rust
use crate::modules::staff::services::{dashboard_service, staff_service};
```

Then add this handler after the `StaffListData` struct and before `list_staff`:

```rust
pub async fn get_staff_dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;

    dashboard_service::ensure_active_staff_user(&pool, actor.user_id).await?;
    let data = dashboard_service::get_staff_dashboard(&pool).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))).into_response())
}
```

- [ ] **Step 4: Register the route before `/api/staff/{id}`**

In `backend-school/src/main.rs`, insert this block between the existing `/api/staff` GET route and `/api/staff/{id}` GET route:

```rust
        .route(
            "/api/staff/dashboard",
            get(modules::staff::handlers::staff::get_staff_dashboard)
                .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)),
        )
```

- [ ] **Step 5: Run the static test and verify it passes**

Run:

```bash
cd backend-school
cargo test staff_dashboard_endpoint_is_staff_scoped_and_aggregate_only --test static_architecture
```

Expected: PASS.

- [ ] **Step 6: Run focused backend tests**

Run:

```bash
cd backend-school
cargo test modules::staff::services::dashboard_service::tests --bin backend-school
cargo test staff_dashboard_endpoint_is_staff_scoped_and_aggregate_only --test static_architecture
```

Expected: both commands PASS.

- [ ] **Step 7: Commit the backend route**

Run:

```bash
git add backend-school/src/main.rs backend-school/src/modules/staff/handlers/staff.rs backend-school/tests/static_architecture.rs
git commit -m "Expose staff dashboard aggregate endpoint"
```

---

### Task 3: Frontend Staff API Contract

**Files:**
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Modify: `frontend-school/src/lib/api/staff.ts`

- [ ] **Step 1: Write the failing frontend API contract test**

Add this test near the staff-related API contract tests in `frontend-school/tests/static/api-response-contract.test.mjs`:

```javascript
test('staff dashboard API uses a typed aggregate-only response', async () => {
	const frontendStaffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const backendService = await readRepoFile(
		'backend-school/src/modules/staff/services/dashboard_service.rs'
	);
	const backendHandler = await readRepoFile('backend-school/src/modules/staff/handlers/staff.rs');

	assert.match(frontendStaffApi, /interface\s+StaffDashboardOverview/);
	assert.match(frontendStaffApi, /totalStaff:\s*number/);
	assert.match(frontendStaffApi, /totalStudents:\s*number/);
	assert.match(frontendStaffApi, /activeClassrooms:\s*number/);
	assert.match(
		frontendStaffApi,
		/getStaffDashboard\(\):\s*Promise<ApiResponse<StaffDashboardOverview>>/
	);
	assert.match(frontendStaffApi, /apiClient\.get<StaffDashboardOverview>\('\/api\/staff\/dashboard'\)/);

	assert.match(backendService, /struct\s+StaffDashboardOverview/);
	assert.match(backendService, /#\[serde\(rename_all = "camelCase"\)\]/);
	assert.match(backendHandler, /ApiResponse::ok\(data\)/);

	assert.doesNotMatch(frontendStaffApi, /listStaff\(\{[\s\S]*page_size:\s*1/);
	assert.doesNotMatch(frontendStaffApi, /listStudents\(\{[\s\S]*page_size:\s*1/);
});
```

- [ ] **Step 2: Run the static test and verify it fails**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "staff dashboard API uses a typed aggregate-only response"
```

Expected: FAIL because `StaffDashboardOverview` and `getStaffDashboard()` do not exist yet.

- [ ] **Step 3: Implement the typed frontend API wrapper**

In `frontend-school/src/lib/api/staff.ts`, add this interface after `StaffListResponse`:

```typescript
export interface StaffDashboardOverview {
	totalStaff: number;
	totalStudents: number;
	activeClassrooms: number;
}
```

Add this function before `listStaff()`:

```typescript
export async function getStaffDashboard(): Promise<ApiResponse<StaffDashboardOverview>> {
	return apiClient.get<StaffDashboardOverview>('/api/staff/dashboard');
}
```

- [ ] **Step 4: Run the static test and verify it passes**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "staff dashboard API uses a typed aggregate-only response"
```

Expected: PASS.

- [ ] **Step 5: Commit the frontend API contract**

Run:

```bash
git add frontend-school/src/lib/api/staff.ts frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "Add typed staff dashboard frontend API"
```

---

### Task 4: Staff Dashboard Page

**Files:**
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/+page.svelte`

- [ ] **Step 1: Write the failing staff dashboard page static test**

Inside the existing `dashboard and self-view routes stay user-scoped with module-aware staff shortcuts` test in `frontend-school/tests/static/api-global-contract.test.mjs`, add these assertions after the existing `assert.match(staffDashboard, /from '\$lib\/permissions\/registry'/);` line:

```javascript
	assert.match(staffDashboard, /getStaffDashboard/);
	assert.match(staffDashboard, /StaffDashboardOverview/);
	assert.match(staffDashboard, /loadDashboard/);
	assert.match(staffDashboard, /<PageSkeleton\s+variant="cards"/);
	assert.match(staffDashboard, /<PageState[\s\S]*โหลดภาพรวมโรงเรียนไม่สำเร็จ/);
	assert.match(staffDashboard, /stats\.activeClassrooms/);
	assert.doesNotMatch(staffDashboard, /placeholder - should fetch from API/);
	assert.doesNotMatch(staffDashboard, /totalStaff:\s*0/);
	assert.doesNotMatch(staffDashboard, /listStaff\(/);
	assert.doesNotMatch(staffDashboard, /listStudents\(/);
```

- [ ] **Step 2: Run the static test and verify it fails**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "dashboard and self-view routes stay user-scoped"
```

Expected: FAIL because the staff page still uses placeholder stats and does not call `getStaffDashboard`.

- [ ] **Step 3: Replace the staff dashboard page implementation**

Replace all of `frontend-school/src/routes/(app)/staff/+page.svelte` with:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { getStaffDashboard, type StaffDashboardOverview } from '$lib/api/staff';
	import { PERMISSION_MODULES } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		BookOpen,
		Building2,
		Calendar,
		FileText,
		GraduationCap,
		Icon as LucideIcon,
		RefreshCw,
		Settings,
		ShieldCheck,
		Users
	} from 'lucide-svelte';

	let stats = $state<StaffDashboardOverview | null>(null);
	let loadingStats = $state(true);
	let statsError = $state('');

	let canOpenStaffModule = $derived($can.hasModule(PERMISSION_MODULES.STAFF_PROFILE));
	let canOpenStudentModule = $derived($can.hasModule(PERMISSION_MODULES.STUDENT));
	let canOpenRolesModule = $derived($can.hasModule(PERMISSION_MODULES.ROLES));
	let canOpenSettingsModule = $derived($can.hasModule(PERMISSION_MODULES.SETTINGS));

	const numberFormatter = new Intl.NumberFormat('th-TH');

	type StatCard = {
		label: string;
		value: number;
		description: string;
		toneClass: string;
		icon: typeof LucideIcon;
	};

	const summaryCards = $derived<StatCard[]>(
		stats
			? [
					{
						label: 'บุคลากรทั้งหมด',
						value: stats.totalStaff,
						description: 'ครูและเจ้าหน้าที่ที่ใช้งานอยู่',
						toneClass: 'bg-sky-500/10 text-sky-600',
						icon: Users
					},
					{
						label: 'นักเรียนทั้งหมด',
						value: stats.totalStudents,
						description: 'นักเรียนสถานะใช้งานอยู่',
						toneClass: 'bg-emerald-500/10 text-emerald-600',
						icon: GraduationCap
					},
					{
						label: 'ห้องเรียนที่เปิด',
						value: stats.activeClassrooms,
						description: 'ห้องเรียน active ในระบบ',
						toneClass: 'bg-amber-500/10 text-amber-600',
						icon: Building2
					}
				]
			: []
	);

	async function loadDashboard() {
		loadingStats = true;
		statsError = '';

		try {
			const response = await getStaffDashboard();
			if (!response.success || !response.data) {
				throw new Error(response.error || 'ไม่สามารถโหลดภาพรวมโรงเรียนได้');
			}
			stats = response.data;
		} catch (error) {
			statsError = error instanceof Error ? error.message : 'ไม่สามารถโหลดภาพรวมโรงเรียนได้';
		} finally {
			loadingStats = false;
		}
	}

	onMount(() => {
		void loadDashboard();
	});
</script>

<svelte:head>
	<title>แดชบอร์ดบุคลากร - SchoolOrbit</title>
</svelte:head>

<PageShell title="แดชบอร์ดบุคลากร" description="ภาพรวมโรงเรียนและทางลัดสำหรับการทำงานประจำวัน">
	{#if loadingStats}
		<PageSkeleton variant="cards" rows={3} />
	{:else if statsError}
		<PageState
			variant="error"
			title="โหลดภาพรวมโรงเรียนไม่สำเร็จ"
			description={statsError}
			actionLabel="ลองอีกครั้ง"
			onaction={loadDashboard}
		/>
	{:else if stats}
		<div class="grid gap-4 md:grid-cols-3">
			{#each summaryCards as item (item.label)}
				<Card class="overflow-hidden">
					<CardContent class="p-5">
						<div class="flex items-start justify-between gap-4">
							<div class="min-w-0 space-y-2">
								<p class="text-muted-foreground text-sm font-medium">{item.label}</p>
								<p class="text-foreground text-3xl font-semibold tracking-normal">
									{numberFormatter.format(item.value)}
								</p>
								<p class="text-muted-foreground text-xs">{item.description}</p>
							</div>
							<div class={`flex h-11 w-11 shrink-0 items-center justify-center rounded-lg ${item.toneClass}`}>
								{@const Icon = item.icon}
								<Icon class="h-5 w-5" />
							</div>
						</div>
					</CardContent>
				</Card>
			{/each}
		</div>
	{/if}

	<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
		<Card>
			<CardHeader>
				<CardTitle>เมนูด่วน</CardTitle>
				<CardDescription>ทางลัดจะแสดงตามสิทธิ์ของบัญชีที่ใช้งานอยู่</CardDescription>
			</CardHeader>
			<CardContent>
				<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
					<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/timetable">
						<Calendar class="h-5 w-5 text-sky-600" />
						<span class="text-left">ตารางสอนของฉัน</span>
					</Button>

					{#if canOpenStaffModule}
						<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/manage">
							<Users class="h-5 w-5 text-sky-600" />
							<span class="text-left">จัดการบุคลากร</span>
						</Button>
					{/if}

					{#if canOpenStudentModule}
						<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/students">
							<GraduationCap class="h-5 w-5 text-emerald-600" />
							<span class="text-left">จัดการนักเรียน</span>
						</Button>
					{/if}

					{#if canOpenRolesModule}
						<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/organization">
							<FileText class="h-5 w-5 text-violet-600" />
							<span class="text-left">โครงสร้างโรงเรียน</span>
						</Button>
					{/if}

					{#if canOpenSettingsModule}
						<Button
							variant="outline"
							class="h-auto justify-start gap-3 p-4"
							href="/staff/school-settings"
						>
							<ShieldCheck class="h-5 w-5 text-amber-600" />
							<span class="text-left">ตั้งค่าโรงเรียน</span>
						</Button>
					{/if}

					<Button variant="outline" class="h-auto justify-start gap-3 p-4" href="/staff/settings">
						<Settings class="h-5 w-5 text-muted-foreground" />
						<span class="text-left">ตั้งค่าบัญชี</span>
					</Button>
				</div>
			</CardContent>
		</Card>

		<Card>
			<CardHeader>
				<CardTitle>สถานะข้อมูล</CardTitle>
				<CardDescription>ตัวเลขรวมไม่เปิดเผยรายชื่อหรือข้อมูลส่วนบุคคล</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				<div class="rounded-lg border bg-muted/30 p-4">
					<div class="flex items-start gap-3">
						<BookOpen class="mt-0.5 h-5 w-5 text-emerald-600" />
						<div class="space-y-1">
							<p class="font-medium">ข้อมูลภาพรวมโรงเรียน</p>
							<p class="text-muted-foreground text-sm">
								ครูทุกคนเห็นจำนวนรวมของบุคลากร นักเรียน และห้องเรียนที่เปิดอยู่ได้
							</p>
						</div>
					</div>
				</div>

				<Button variant="outline" class="w-full gap-2" onclick={loadDashboard}>
					<RefreshCw class="h-4 w-4" />
					รีเฟรชข้อมูล
				</Button>
			</CardContent>
		</Card>
	</div>
</PageShell>
```

- [ ] **Step 4: Run the static test and verify it passes**

Run:

```bash
cd frontend-school
npm run test:static -- --test-name-pattern "dashboard and self-view routes stay user-scoped"
```

Expected: PASS.

- [ ] **Step 5: Run frontend Svelte check**

Run:

```bash
cd frontend-school
npm run check
```

Expected: PASS with no Svelte or TypeScript errors.

- [ ] **Step 6: Commit the staff dashboard page**

Run:

```bash
git add frontend-school/src/routes/'(app)'/staff/+page.svelte frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "Load real staff dashboard overview"
```

---

### Task 5: Full Verification

**Files:**
- No new files.

- [ ] **Step 1: Run backend static architecture tests**

Run:

```bash
cd backend-school
cargo test --test static_architecture
```

Expected: PASS.

- [ ] **Step 2: Run backend check**

Run:

```bash
cd backend-school
cargo check
```

Expected: PASS.

- [ ] **Step 3: Run frontend static tests**

Run:

```bash
cd frontend-school
npm run test:static
```

Expected: PASS.

- [ ] **Step 4: Run frontend type/Svelte checks**

Run:

```bash
cd frontend-school
npm run check
```

Expected: PASS.

- [ ] **Step 5: Run repository diff checks**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; `git status --short` should show only intentional committed branch changes or be clean.

- [ ] **Step 6: Commit final verification fixes when files changed during verification**

If verification fixes changed files after the task commits, run:

```bash
git add backend-school/src/modules/staff/services/dashboard_service.rs \
  backend-school/src/modules/staff/services.rs \
  backend-school/src/modules/staff/handlers/staff.rs \
  backend-school/src/main.rs \
  backend-school/tests/static_architecture.rs \
  frontend-school/src/lib/api/staff.ts \
  frontend-school/src/routes/'(app)'/staff/+page.svelte \
  frontend-school/tests/static/api-response-contract.test.mjs \
  frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "Verify staff dashboard overview"
```

Expected: any final verification-only changes are committed separately.

---

## Self-Review Notes

- Spec coverage: the plan adds the aggregate backend endpoint, keeps existing list/profile permission contracts intact, and updates `/staff` into the approved Operational Overview direction.
- Privacy coverage: static tests prevent use of list APIs and block obvious PII fields in the dashboard aggregate service.
- Type consistency: backend DTO fields use snake_case with `serde(rename_all = "camelCase")`; frontend contract uses `totalStaff`, `totalStudents`, and `activeClassrooms`.
- Scope: no charts, recent activity feed, attendance metrics, or management permission changes are included.
