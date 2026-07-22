# Module Permission UI Rollout Implementation Plan

> **Superseded scheduler guidance (2026-07-23):** Scheduler routes listed in this historical inventory were removed. Manual timetable editing is the only supported construction workflow; see `2026-07-23-auto-scheduler-removal.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Standardize staff-facing module pages so route/menu access is module-level, page controls progressively appear by specific permission, and refactored UI uses the local shadcn-svelte component set.

**Architecture:** Keep backend authorization as the source of truth. Frontend route/menu metadata decides whether a user can enter a workspace, while page-level capability checks decide which panels, buttons, columns, and API calls are available. Roll out from `staff/manage` first, then apply the same audit and refactor pattern module by module.

**Tech Stack:** SvelteKit 5, TypeScript, local shadcn-svelte components under `frontend-school/src/lib/components/ui`, Tailwind CSS for layout only, Rust Axum, sqlx/PostgreSQL, existing permission registries and static contract tests.

---

## Scope And Rollout Rule

This is a multi-module rollout. Do not land it as one large diff. Land the foundation and `staff/manage` pilot first, then one module wave per commit or PR. Each wave must leave the app shippable.

The required pattern for every module page is:

1. `_meta.menu.permission` uses the target `PERMISSION_MODULES` value listed in the Module Inventory table for real workspace menu routes whenever the backend registry has that module.
2. `_meta.access` is used for child/detail/action routes that need guards but must not create menu records.
3. The page derives capability booleans from `$can.has`, `$can.hasAny`, `$can.hasAll`, or `$can.hasModule`.
4. The page only calls APIs that the current capability set is allowed to call.
5. Backend endpoints keep exact permission and resource-scope enforcement.
6. Visible controls use the local shadcn-svelte components for buttons, inputs, cards, tables, badges, alerts, dialogs, tabs, selects, switches, checkboxes, commands, tooltips, and scroll areas. Tailwind classes remain valid for grid, spacing, typography, and responsive layout.

## Existing Local UI Components To Prefer

Use these existing local exports before adding any new dependency:

- `Button`: `frontend-school/src/lib/components/ui/button`
- `Input`: `frontend-school/src/lib/components/ui/input`
- `Textarea`: `frontend-school/src/lib/components/ui/textarea`
- `Label`: `frontend-school/src/lib/components/ui/label`
- `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter`, `CardAction`: `frontend-school/src/lib/components/ui/card`
- `Table`, `TableHeader`, `TableBody`, `TableRow`, `TableHead`, `TableCell`, `TableCaption`, `TableFooter`: `frontend-school/src/lib/components/ui/table`
- `Badge`: `frontend-school/src/lib/components/ui/badge`
- `Alert`, `AlertTitle`, `AlertDescription`: `frontend-school/src/lib/components/ui/alert`
- `Dialog`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter`, `DialogTrigger`, `DialogClose`: `frontend-school/src/lib/components/ui/dialog`
- `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent`: `frontend-school/src/lib/components/ui/tabs`
- `Select`, `SelectTrigger`, `SelectContent`, `SelectItem`, `SelectGroup`, `SelectLabel`: `frontend-school/src/lib/components/ui/select`
- `Switch`: `frontend-school/src/lib/components/ui/switch`
- `Checkbox`: `frontend-school/src/lib/components/ui/checkbox`
- `Tooltip`: `frontend-school/src/lib/components/ui/tooltip`
- `Command`: `frontend-school/src/lib/components/ui/command`
- `ScrollArea`: `frontend-school/src/lib/components/ui/scroll-area`
- `Progress`: `frontend-school/src/lib/components/ui/progress`
- `Avatar`: `frontend-school/src/lib/components/ui/avatar`

Do not replace every layout `div`; replace ad-hoc controls and semantic UI surfaces.

## Module Inventory

Use this table to drive the rollout. For each row, update the route metadata, page capability gates, API calls, and shadcn-svelte surfaces.

| Wave | Route area | Primary files | Target route gate | Notes |
| --- | --- | --- | --- | --- |
| 1 | Staff management | `frontend-school/src/routes/(app)/staff/manage/**` | `PERMISSION_MODULES.STAFF_PROFILE` for list/detail workspace; exact `PERMISSIONS.STAFF_CREATE_ALL`, `STAFF_UPDATE_ALL`, `STAFF_DELETE_ALL`, `STAFF_PII_READ_*` inside pages | Pilot pattern. Base read uses `staff_profile`; mutations use `staff`; sensitive fields use `staff_pii`. |
| 2 | Roles and organization | `staff/roles/**`, `staff/organization/**` | `PERMISSION_MODULES.ROLES` | Already close to target. Refactor remaining ad-hoc UI surfaces and confirm action gates. |
| 3 | Menu, features, school settings | `staff/menu/**`, `staff/features/**`, `staff/school-settings/**` | `PERMISSION_MODULES.MENU`, `PERMISSION_MODULES.FEATURES`, `PERMISSION_MODULES.SETTINGS` | Split menu/features from settings instead of treating all as settings. |
| 4 | Students | `staff/students/**` | `PERMISSION_MODULES.STUDENT` | Keep PII fields behind `student_pii.*`; use child `_meta.access` for create/edit/detail routes. |
| 5 | Achievements | `staff/achievements/**` | `PERMISSION_MODULES.ACHIEVEMENT` | Already module-gated. Standardize UI and verify own/all action gates. |
| 6 | Academic curriculum | `staff/academic/subjects/**`, `subject-groups/**`, `study-plans/**` | `PERMISSION_MODULES.ACADEMIC_CURRICULUM` | Large pages. Split pass into list/actions first, then forms/dialogs. |
| 7 | Academic structure | `staff/academic/structure/**`, `periods/**` | `PERMISSION_MODULES.ACADEMIC_STRUCTURE` | `periods` is currently manage-gated; route should open by module and hide mutation controls without manage. |
| 8 | Classrooms and enrollment | `staff/academic/classrooms/**`, `enrollments/**` | `PERMISSION_MODULES.ACADEMIC_CLASSROOM`, `PERMISSION_MODULES.ACADEMIC_ENROLLMENT` | Verify list endpoints and mutation controls line up with capability checks. |
| 9 | Course planning and timetable | `staff/academic/planning/**`, `timetable/**` | `PERMISSION_MODULES.ACADEMIC_COURSE_PLAN` | Timetable is currently manage-gated. Make read-only planning/timetable states usable for read-only actors. |
| 10 | Admission | `staff/academic/admission/**` | `PERMISSION_MODULES.ADMISSION` | Panels: read/manage rounds, verify applications, scores, enrollment, exam rooms, reports. Avoid calling read-only endpoints for score-only users unless backend allows it. |
| 11 | Activity | `staff/academic/activities/**`, `student/activities/**` | `PERMISSION_MODULES.ACTIVITY` for staff; user-type route for student self-service | Already module-gated on staff. Standardize dialogs/tables and own/all/member gates. |
| 12 | Supervision | `staff/academic/supervision/**` | `PERMISSION_MODULES.SUPERVISION` | Already best reference. Use it as the behavior model, then clean remaining UI inconsistencies. |
| 13 | Facility | `staff/facility/buildings/**` | `PERMISSION_MODULES.FACILITY` | Make read-only building list usable when actor has only read. |
| 14 | Organization work | `staff/work/**` | existing `workflowManage` guard plus `PERMISSION_MODULES.ORGANIZATION_WORK` where menu/access needs module checks | Keep "My Work" stable; management route remains guard-only. |
| 15 | Dashboard and self views | `staff/+page`, `staff/timetable`, `student/+page`, `student/timetable`, `parent/**` | user-type route gates and self-view APIs | Do not over-permission self routes; backend resolves current actor or parent-child link. |

## Files Created Or Modified

### Foundation

- Modify `frontend-school/src/lib/permissions/registry.ts`
  - Add missing `PERMISSION_MODULES` constants that already exist as backend registry modules.
  - Keep `PERMISSIONS` values aligned with `backend-school/src/permissions/registry.rs`.

- Modify `frontend-school/tests/static/api-global-contract.test.mjs`
  - Add static guards for module-level route gates on the rollout inventory.
  - Add a shadcn-svelte regression check for the `staff/manage` pilot.

- Modify `frontend-school/src/lib/auth/route-access.ts` only if a wave needs guard-only `permissionAny`.
  - Keep `_meta.menu.permission` as a single value because menu sync stores one `required_permission`.
  - Use `_meta.access.permissionAny` only for child/detail/action routes that can be entered through several capabilities without creating menu records.

### Staff Management Pilot

- Modify `frontend-school/src/routes/(app)/staff/manage/+page.ts`
  - Change menu route gate from `PERMISSIONS.STAFF_PROFILE_READ_SCHOOL` to `PERMISSION_MODULES.STAFF_PROFILE`.

- Modify `frontend-school/src/routes/(app)/staff/manage/+page.svelte`
  - Add permission capability booleans.
  - Hide create/edit/delete controls unless specific permissions exist.
  - Replace ad-hoc cards/tables/badges/alerts with local shadcn-svelte components.
  - Do not expose PII in the list.

- Modify `frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte`
  - Gate edit button with `STAFF_UPDATE_ALL`.
  - Gate `national_id` with `STAFF_PII_READ_OWN` or `STAFF_PII_READ_SCHOOL`.
  - Use `Card`, `Badge`, `Alert`, `Button`, and `Avatar`.

- Modify `frontend-school/src/routes/(app)/staff/manage/new/+page.svelte`
  - Ensure route has guard-only access requiring `STAFF_CREATE_ALL`.
  - Use shadcn-svelte form controls consistently.

- Modify `frontend-school/src/routes/(app)/staff/manage/[id]/edit/+page.svelte`
  - Ensure route has guard-only access requiring `STAFF_UPDATE_ALL`.
  - Use shadcn-svelte form controls consistently.

- Backend staff files are read-only for the first pilot unless a frontend-permitted path hits a backend 403:
  - `backend-school/src/modules/staff/handlers/staff.rs`
  - `backend-school/src/policies/staff_access_policy.rs`
  - `backend-school/src/modules/staff/services/staff_service.rs`

## Task 1: Foundation Permission Modules

**Files:**
- Modify: `frontend-school/src/lib/permissions/registry.ts`
- Test: `frontend-school/tests/static/api-global-contract.test.mjs`

- [ ] **Step 1: Add missing frontend module constants**

In `frontend-school/src/lib/permissions/registry.ts`, replace the `PERMISSION_MODULES` object with this complete object, preserving the existing export name:

```ts
export const PERMISSION_MODULES = {
	ACADEMIC_CLASSROOM: 'academic_classroom',
	ACADEMIC_COURSE_PLAN: 'academic_course_plan',
	ACADEMIC_CURRICULUM: 'academic_curriculum',
	ACADEMIC_ENROLLMENT: 'academic_enrollment',
	ACADEMIC_PROMOTION: 'academic_promotion',
	ACADEMIC_STRUCTURE: 'academic_structure',
	ACTIVITY: 'activity',
	ACHIEVEMENT: 'achievement',
	ADMISSION: 'admission',
	DASHBOARD: 'dashboard',
	FACILITY: 'facility',
	FEATURES: 'features',
	MENU: 'menu',
	ORGANIZATION_WORK: 'organization_work',
	ROLES: 'roles',
	SETTINGS: 'settings',
	STAFF: 'staff',
	STAFF_PII: 'staff_pii',
	STAFF_PROFILE: 'staff_profile',
	STUDENT: 'student',
	STUDENT_PII: 'student_pii',
	SUPERVISION: 'supervision',
	SYSTEM: 'system'
} as const;
```

- [ ] **Step 2: Run registry alignment test**

Run:

```bash
cd frontend-school
npm run test:static
```

Expected: existing static tests pass. If the test reports a frontend module missing from the backend registry, remove that frontend module and document why the backend has no runtime permission module for it.

- [ ] **Step 3: Commit foundation module constants**

```bash
git add frontend-school/src/lib/permissions/registry.ts
git commit -m "feat: expand frontend permission modules"
```

## Task 2: Static Guards For Rollout Rules

**Files:**
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`

- [ ] **Step 1: Add route gate static test**

Append this test near the existing route metadata tests in `frontend-school/tests/static/api-global-contract.test.mjs`:

```js
test('staff module workspace routes use module-level menu permission gates', async () => {
	const expectations = new Map([
		['frontend-school/src/routes/(app)/staff/manage/+page.ts', 'STAFF_PROFILE'],
		['frontend-school/src/routes/(app)/staff/menu/+page.ts', 'MENU'],
		['frontend-school/src/routes/(app)/staff/features/+page.ts', 'FEATURES'],
		['frontend-school/src/routes/(app)/staff/school-settings/+page.ts', 'SETTINGS'],
		['frontend-school/src/routes/(app)/staff/facility/buildings/+page.ts', 'FACILITY'],
		['frontend-school/src/routes/(app)/staff/academic/periods/+page.ts', 'ACADEMIC_STRUCTURE'],
		['frontend-school/src/routes/(app)/staff/academic/enrollments/+page.ts', 'ACADEMIC_ENROLLMENT'],
		['frontend-school/src/routes/(app)/staff/academic/admission/+page.ts', 'ADMISSION'],
		['frontend-school/src/routes/(app)/staff/academic/timetable/+page.ts', 'ACADEMIC_COURSE_PLAN'],
		['frontend-school/src/routes/(app)/staff/academic/structure/+page.ts', 'ACADEMIC_STRUCTURE'],
		['frontend-school/src/routes/(app)/staff/academic/classrooms/+page.ts', 'ACADEMIC_CLASSROOM'],
		['frontend-school/src/routes/(app)/staff/academic/study-plans/+page.ts', 'ACADEMIC_CURRICULUM'],
		['frontend-school/src/routes/(app)/staff/academic/planning/+page.ts', 'ACADEMIC_COURSE_PLAN']
	]);

	const violations = [];

	for (const [routeFile, moduleName] of expectations) {
		const source = stripComments(await readFile(path.join(repoRoot, routeFile), 'utf8'));
		const pattern = new RegExp(`permission:\\s*PERMISSION_MODULES\\.${moduleName}\\b`);
		if (!pattern.test(source)) {
			violations.push(`${routeFile}: expected PERMISSION_MODULES.${moduleName}`);
		}
	}

	assert.deepEqual(violations, []);
});
```

- [ ] **Step 2: Add staff/manage shadcn pilot static test**

Append this test after the route gate static test:

```js
test('staff manage pilot uses shadcn-svelte surfaces and permission gates', async () => {
	const source = stripComments(
		await readFile(
			path.join(repoRoot, 'frontend-school/src/routes/(app)/staff/manage/+page.svelte'),
			'utf8'
		)
	);

	for (const requiredImport of [
		"$lib/components/ui/button",
		"$lib/components/ui/input",
		"$lib/components/ui/dialog",
		"$lib/components/ui/table",
		"$lib/components/ui/card",
		"$lib/components/ui/badge",
		"$lib/components/ui/alert"
	]) {
		assert.match(source, new RegExp(requiredImport.replaceAll('/', '\\/')));
	}

	for (const requiredPermission of [
		'PERMISSIONS.STAFF_PROFILE_READ_OWN',
		'PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT',
		'PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE',
		'PERMISSIONS.STAFF_PROFILE_READ_SCHOOL',
		'PERMISSIONS.STAFF_CREATE_ALL',
		'PERMISSIONS.STAFF_UPDATE_ALL',
		'PERMISSIONS.STAFF_DELETE_ALL'
	]) {
		assert.match(source, new RegExp(requiredPermission.replace('.', '\\.')));
	}
});
```

- [ ] **Step 3: Run static tests and verify the new tests fail before implementation**

Run:

```bash
cd frontend-school
npm run test:static
```

Expected: FAIL. At least `staff/manage/+page.ts` should still reference `PERMISSIONS.STAFF_PROFILE_READ_SCHOOL`, and `staff/manage/+page.svelte` should not yet import every required shadcn surface.

- [ ] **Step 4: Commit failing static guards**

Commit only if the team accepts failing tests as a TDD checkpoint on the working branch. If not, keep these test changes staged locally and commit them with Task 3.

```bash
git add frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "test: guard module-level route gates"
```

## Task 3: Staff Manage Route Gate Pilot

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/manage/+page.ts`

- [ ] **Step 1: Change route metadata import**

Replace:

```ts
import { PERMISSIONS } from '$lib/permissions/registry';
```

with:

```ts
import { PERMISSION_MODULES } from '$lib/permissions/registry';
```

- [ ] **Step 2: Change route gate**

Replace:

```ts
permission: PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
```

with:

```ts
permission: PERMISSION_MODULES.STAFF_PROFILE
```

- [ ] **Step 3: Run static route metadata tests**

Run:

```bash
cd frontend-school
npm run test:static
```

Expected: the route gate violation for `staff/manage/+page.ts` is gone. The shadcn staff manage pilot test may still fail until Task 4.

## Task 4: Staff Manage List Capability Gates And shadcn Surfaces

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/manage/+page.svelte`

- [ ] **Step 1: Replace imports with capability and shadcn imports**

At the top of `staff/manage/+page.svelte`, use this import block:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import { listStaff, deleteStaff, type StaffListItem } from '$lib/api/staff';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Alert, AlertDescription, AlertTitle } from '$lib/components/ui/alert';
	import { Users, Plus, Search, Pencil, Trash2, Eye, AlertTriangle } from 'lucide-svelte';
```

- [ ] **Step 2: Add capability booleans**

After the existing state declarations, add:

```ts
	const canReadStaff = $derived(
		$can.hasAny(
			PERMISSIONS.STAFF_PROFILE_READ_OWN,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
			PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
		)
	);
	const canCreateStaff = $derived($can.has(PERMISSIONS.STAFF_CREATE_ALL));
	const canUpdateStaff = $derived($can.has(PERMISSIONS.STAFF_UPDATE_ALL));
	const canDeleteStaff = $derived($can.has(PERMISSIONS.STAFF_DELETE_ALL));
```

- [ ] **Step 3: Guard staff list API calls**

At the start of `loadStaff()`, before `try`, add:

```ts
		if (!canReadStaff) {
			staffList = [];
			total = 0;
			totalPages = 1;
			loading = false;
			error = '';
			return;
		}
```

- [ ] **Step 4: Guard delete action**

At the start of `openDeleteDialog(staff)`, add:

```ts
		if (!canDeleteStaff) return;
```

At the start of `confirmDelete()`, replace the first guard with:

```ts
		if (!staffToDelete || !canDeleteStaff) return;
```

- [ ] **Step 5: Hide create links unless allowed**

Wrap both `href="/staff/manage/new"` buttons with:

```svelte
{#if canCreateStaff}
	<Button href="/staff/manage/new" class="flex items-center gap-2">
		<Plus class="w-4 h-4" />
		เพิ่มบุคลากร
	</Button>
{/if}
```

- [ ] **Step 6: Replace search surface with `Card`**

Replace the search container:

```svelte
<div class="bg-card border border-border rounded-lg p-4">
```

with:

```svelte
<Card>
	<CardContent class="p-4">
```

and close it with:

```svelte
	</CardContent>
</Card>
```

- [ ] **Step 7: Replace loading and empty surfaces with `Card`**

Use this loading block:

```svelte
{:else if loading}
	<Card>
		<CardContent class="p-12 text-center">
			<div
				class="inline-block h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent"
			></div>
			<p class="mt-4 text-muted-foreground">กำลังโหลด...</p>
		</CardContent>
	</Card>
```

Use this no-read block before the normal empty state:

```svelte
{#if !canReadStaff}
	<Alert>
		<AlertTriangle class="h-4 w-4" />
		<AlertTitle>ไม่มีสิทธิ์ดูรายชื่อบุคลากร</AlertTitle>
		<AlertDescription>
			คุณยังสามารถใช้ปุ่มการทำงานที่ระบบอนุญาตไว้ได้ หากมีสิทธิ์เฉพาะด้านอื่น
		</AlertDescription>
	</Alert>
```

Use `Card`, `CardHeader`, `CardTitle`, `CardDescription`, and `CardContent` for the empty state instead of raw card-like divs.

- [ ] **Step 8: Replace list table with shadcn `Table`**

Replace the ad-hoc grid header/body with:

```svelte
<Card>
	<CardHeader>
		<CardTitle>รายชื่อบุคลากร</CardTitle>
		<CardDescription>แสดง {staffList.length} จาก {total} รายการ</CardDescription>
	</CardHeader>
	<CardContent class="p-0">
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead>ชื่อ-นามสกุล</TableHead>
					<TableHead>บทบาท</TableHead>
					<TableHead>สถานะ</TableHead>
					<TableHead class="text-right">จัดการ</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{#each staffList as staff (staff.id)}
					<TableRow>
						<TableCell>
							<p class="font-medium text-foreground">
								{staff.title}{staff.first_name}
								{staff.last_name}
							</p>
							<p class="text-xs text-muted-foreground">{staff.username}</p>
						</TableCell>
						<TableCell>
							<div class="flex flex-wrap gap-1">
								{#if staff.roles && staff.roles.length > 0}
									{#each staff.roles.slice(0, 2) as role (role)}
										<Badge variant="secondary">{role}</Badge>
									{/each}
									{#if staff.roles.length > 2}
										<Badge variant="outline">+{staff.roles.length - 2}</Badge>
									{/if}
								{:else}
									<span class="text-sm text-muted-foreground">-</span>
								{/if}
							</div>
						</TableCell>
						<TableCell>
							<Badge variant={staff.status === 'active' ? 'default' : 'secondary'}>
								{staff.status === 'active' ? 'ใช้งาน' : 'ไม่ใช้งาน'}
							</Badge>
						</TableCell>
						<TableCell>
							<div class="flex justify-end gap-2">
								<Button href="/staff/manage/{staff.id}" variant="ghost" size="icon-sm" aria-label="ดูข้อมูล">
									<Eye class="w-4 h-4" />
								</Button>
								{#if canUpdateStaff}
									<Button
										href="/staff/manage/{staff.id}/edit"
										variant="ghost"
										size="icon-sm"
										aria-label="แก้ไข"
									>
										<Pencil class="w-4 h-4" />
									</Button>
								{/if}
								{#if canDeleteStaff}
									<Button
										onclick={() => openDeleteDialog(staff)}
										variant="ghost"
										size="icon-sm"
										aria-label="ลบ"
									>
										<Trash2 class="h-4 w-4" />
									</Button>
								{/if}
							</div>
						</TableCell>
					</TableRow>
				{/each}
			</TableBody>
		</Table>
	</CardContent>
</Card>
```

- [ ] **Step 9: Run frontend checks**

Run:

```bash
cd frontend-school
npm run test:static
npm run check
```

Expected: static tests pass and Svelte check passes. If `Badge` variant names differ from the local component API, inspect `frontend-school/src/lib/components/ui/badge/badge.svelte` and use the variants it exports.

- [ ] **Step 10: Commit staff manage list pilot**

```bash
git add frontend-school/src/routes/\(app\)/staff/manage/+page.ts frontend-school/src/routes/\(app\)/staff/manage/+page.svelte frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "feat: pilot module permission UI on staff manage"
```

## Task 5: Staff Manage Detail And Form Gates

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/manage/new/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/manage/[id]/edit/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/manage/new/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/manage/[id]/edit/+page.svelte`

- [ ] **Step 1: Add guard-only route metadata for create page**

Create or update `frontend-school/src/routes/(app)/staff/manage/new/+page.ts`:

```ts
import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.STAFF_CREATE_ALL
	}
};

export const load = async () => {
	return {
		title: 'เพิ่มบุคลากรใหม่'
	};
};
```

- [ ] **Step 2: Add guard-only route metadata for edit page**

Create or update `frontend-school/src/routes/(app)/staff/manage/[id]/edit/+page.ts`:

```ts
import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.STAFF_UPDATE_ALL
	}
};

export const load = async () => {
	return {
		title: 'แก้ไขข้อมูลบุคลากร'
	};
};
```

- [ ] **Step 3: Gate detail edit action**

In `frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte`, import:

```ts
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
```

Add:

```ts
	const canUpdateStaff = $derived($can.has(PERMISSIONS.STAFF_UPDATE_ALL));
	const canReadStaffPii = $derived(
		$can.hasAny(PERMISSIONS.STAFF_PII_READ_OWN, PERMISSIONS.STAFF_PII_READ_SCHOOL)
	);
```

Wrap the edit button:

```svelte
{#if staff && canUpdateStaff}
	<Button href="/staff/manage/{staff.id}/edit" class="flex items-center gap-2">
		แก้ไขข้อมูล
	</Button>
{/if}
```

Wrap the national ID display:

```svelte
{#if staff.national_id && canReadStaffPii}
	<span class="text-foreground">บัตรปชช.: {staff.national_id}</span>
{/if}
```

- [ ] **Step 4: Replace detail surfaces with shadcn components**

Use `Card` for profile summary, education info, roles, organization units, and teaching/advisor sections. Use `Badge` for status, roles, positions, and organization tags. Use `Alert` for load errors. Use `Avatar` for the profile image or initials.

- [ ] **Step 5: Replace create/edit form surfaces with shadcn components**

For `new/+page.svelte` and `[id]/edit/+page.svelte`, use:

- `Card` around each form section.
- `Label` plus `Input`, `Textarea`, `Select`, `Checkbox`, or `Switch` for fields.
- `Button` for next/back/save/cancel.
- `Alert` for submit/load errors.
- `Badge` for selected roles or organization assignments.

- [ ] **Step 6: Run checks**

Run:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

Expected: all commands pass.

- [ ] **Step 7: Commit staff detail and form gates**

```bash
git add frontend-school/src/routes/\(app\)/staff/manage
git commit -m "feat: gate staff detail and form actions"
```

## Task 6: Backend Staff Permission Verification

**Files:**
- Read: `backend-school/src/modules/staff/handlers/staff.rs`
- Read: `backend-school/src/policies/staff_access_policy.rs`
- Modify only if verification proves a mismatch.

- [ ] **Step 1: Verify staff list policy already accepts scoped profile read**

Confirm `staff_access_policy::resolve_staff_profile_list_access()` includes:

```rust
resource_access_policy::resolve_user_resource_list_access(actor, STAFF_PROFILE_ACCESS)
```

Expected: actors with `staff_profile.read.own`, `staff_profile.read.organization_unit`, `staff_profile.read.organization_tree`, or `staff_profile.read.school` can resolve list access.

- [ ] **Step 2: Verify staff mutations stay exact**

Confirm these handler checks remain exact:

```rust
actor.require_permission(codes::STAFF_CREATE_ALL)?;
actor.require_permission(codes::STAFF_UPDATE_ALL)?;
actor.require_permission(codes::STAFF_DELETE_ALL)?;
```

Expected: frontend gates match backend authorization.

- [ ] **Step 3: Run backend checks**

Run:

```bash
cd backend-school
cargo test policies::staff_access_policy::tests --bin backend-school
cargo check
```

Expected: tests and check pass.

- [ ] **Step 4: Commit only if backend changed**

If no backend file changed, do not create a backend commit.

If backend files changed:

```bash
git add backend-school/src/modules/staff/handlers/staff.rs backend-school/src/policies/staff_access_policy.rs
git commit -m "fix: align staff permission policy with UI gates"
```

## Task 7: Module Wave Execution Checklist

Use this checklist for each remaining wave in the module inventory. The tables below name the concrete route gates and permission constants to use.

**Files:**
- Modify: the route files listed in the module inventory row.
- Modify: the page Svelte files listed in the module inventory row.
- Modify: matching frontend API files only when typed contracts need correction.
- Modify: matching backend handlers/policies only when a frontend-visible capability cannot call the backend endpoint it needs.

- [ ] **Step 1: Write or extend static route gate expectation**

Add the route file and expected module name to the `expectations` map from Task 2. The first rollout must include these exact pairs:

| Route file | Expected module |
| --- | --- |
| `frontend-school/src/routes/(app)/staff/manage/+page.ts` | `STAFF_PROFILE` |
| `frontend-school/src/routes/(app)/staff/menu/+page.ts` | `MENU` |
| `frontend-school/src/routes/(app)/staff/features/+page.ts` | `FEATURES` |
| `frontend-school/src/routes/(app)/staff/school-settings/+page.ts` | `SETTINGS` |
| `frontend-school/src/routes/(app)/staff/facility/buildings/+page.ts` | `FACILITY` |
| `frontend-school/src/routes/(app)/staff/academic/periods/+page.ts` | `ACADEMIC_STRUCTURE` |
| `frontend-school/src/routes/(app)/staff/academic/enrollments/+page.ts` | `ACADEMIC_ENROLLMENT` |
| `frontend-school/src/routes/(app)/staff/academic/admission/+page.ts` | `ADMISSION` |
| `frontend-school/src/routes/(app)/staff/academic/timetable/+page.ts` | `ACADEMIC_COURSE_PLAN` |
| `frontend-school/src/routes/(app)/staff/academic/structure/+page.ts` | `ACADEMIC_STRUCTURE` |
| `frontend-school/src/routes/(app)/staff/academic/classrooms/+page.ts` | `ACADEMIC_CLASSROOM` |
| `frontend-school/src/routes/(app)/staff/academic/study-plans/+page.ts` | `ACADEMIC_CURRICULUM` |
| `frontend-school/src/routes/(app)/staff/academic/planning/+page.ts` | `ACADEMIC_COURSE_PLAN` |

- [ ] **Step 2: Change menu route metadata to module gate**

For each route in this table, keep the existing `title`, `icon`, `group`, `workspace`, `order`, and `user_type`, and change only the permission import and permission value.

| Route file | Import | Permission line |
| --- | --- | --- |
| `staff/manage/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.STAFF_PROFILE` |
| `staff/menu/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.MENU` |
| `staff/features/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.FEATURES` |
| `staff/school-settings/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.SETTINGS` |
| `staff/facility/buildings/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.FACILITY` |
| `staff/academic/periods/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_STRUCTURE` |
| `staff/academic/enrollments/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_ENROLLMENT` |
| `staff/academic/admission/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ADMISSION` |
| `staff/academic/timetable/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_COURSE_PLAN` |
| `staff/academic/structure/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_STRUCTURE` |
| `staff/academic/classrooms/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_CLASSROOM` |
| `staff/academic/study-plans/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM` |
| `staff/academic/planning/+page.ts` | `import { PERMISSION_MODULES } from '$lib/permissions/registry';` | `permission: PERMISSION_MODULES.ACADEMIC_COURSE_PLAN` |

- [ ] **Step 3: Add guard-only metadata for child routes**

Use `_meta.access`, not `_meta.menu`, for these child routes. Keep any existing `load` function title.

| Route file | Access permission |
| --- | --- |
| `frontend-school/src/routes/(app)/staff/students/new/+page.ts` | `PERMISSIONS.STUDENT_CREATE_ALL` |
| `frontend-school/src/routes/(app)/staff/students/[id]/edit/+page.ts` | `PERMISSIONS.STUDENT_UPDATE_ALL` |
| `frontend-school/src/routes/(app)/staff/academic/admission/new/+page.ts` | `PERMISSIONS.ADMISSION_MANAGE_ALL` |
| `frontend-school/src/routes/(app)/staff/academic/admission/[id]/exam-rooms/+page.ts` | `PERMISSIONS.ADMISSION_MANAGE_ALL` |
| `frontend-school/src/routes/(app)/staff/academic/admission/[id]/scores/+page.ts` | `PERMISSIONS.ADMISSION_SCORES_ALL` |
| `frontend-school/src/routes/(app)/staff/academic/admission/[id]/enrollment/+page.ts` | `PERMISSIONS.ADMISSION_ENROLL_ALL` |
| `frontend-school/src/routes/(app)/staff/academic/timetable/templates/+page.ts` | `PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL` |
| `frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.ts` | `PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL` |
| `frontend-school/src/routes/(app)/staff/work/manage/+page.ts` | keep existing `workflowManage: true` |

Use this exact shape for permission-gated child routes:

```ts
import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.STUDENT_CREATE_ALL
	}
};
```

Replace `PERMISSIONS.STUDENT_CREATE_ALL` with the route's access permission from the table.

- [ ] **Step 4: Add page capability booleans**

At the top of each Svelte page that renders permission-controlled controls:

```ts
import { PERMISSIONS } from '$lib/permissions/registry';
import { can } from '$lib/stores/permissions';
```

Use these concrete capability groups for the rollout:

| Area | Capability booleans |
| --- | --- |
| Roles | `canReadRoles = $can.has(PERMISSIONS.ROLES_READ_ALL)`, `canCreateRoles = $can.has(PERMISSIONS.ROLES_CREATE_ALL)`, `canUpdateRoles = $can.has(PERMISSIONS.ROLES_UPDATE_ALL)`, `canDeleteRoles = $can.has(PERMISSIONS.ROLES_DELETE_ALL)`, `canAssignRoles = $can.has(PERMISSIONS.ROLES_ASSIGN_ALL)`, `canRemoveRoles = $can.has(PERMISSIONS.ROLES_REMOVE_ALL)` |
| Menu | `canReadMenu = $can.has(PERMISSIONS.MENU_READ_ALL)`, `canCreateMenu = $can.has(PERMISSIONS.MENU_CREATE_ALL)`, `canUpdateMenu = $can.has(PERMISSIONS.MENU_UPDATE_ALL)`, `canDeleteMenu = $can.has(PERMISSIONS.MENU_DELETE_ALL)` |
| Features | `canReadFeatures = $can.has(PERMISSIONS.FEATURES_READ_ALL)`, `canUpdateFeatures = $can.has(PERMISSIONS.FEATURES_UPDATE_ALL)` |
| Settings | `canReadSettings = $can.has(PERMISSIONS.SETTINGS_READ_ALL)`, `canUpdateSettings = $can.has(PERMISSIONS.SETTINGS_UPDATE_ALL)` |
| Students | `canReadStudents = $can.hasAny(PERMISSIONS.STUDENT_READ_ASSIGNED, PERMISSIONS.STUDENT_READ_SCHOOL)`, `canCreateStudents = $can.has(PERMISSIONS.STUDENT_CREATE_ALL)`, `canUpdateStudents = $can.has(PERMISSIONS.STUDENT_UPDATE_ALL)`, `canDeleteStudents = $can.has(PERMISSIONS.STUDENT_DELETE_ALL)`, `canReadStudentPii = $can.hasAny(PERMISSIONS.STUDENT_PII_READ_ASSIGNED, PERMISSIONS.STUDENT_PII_READ_SCHOOL)` |
| Achievements | `canReadAchievements = $can.hasAny(PERMISSIONS.ACHIEVEMENT_READ_OWN, PERMISSIONS.ACHIEVEMENT_READ_ALL)`, `canCreateAchievements = $can.hasAny(PERMISSIONS.ACHIEVEMENT_CREATE_OWN, PERMISSIONS.ACHIEVEMENT_CREATE_ALL)`, `canUpdateAchievements = $can.hasAny(PERMISSIONS.ACHIEVEMENT_UPDATE_OWN, PERMISSIONS.ACHIEVEMENT_UPDATE_ALL)`, `canDeleteAchievements = $can.hasAny(PERMISSIONS.ACHIEVEMENT_DELETE_OWN, PERMISSIONS.ACHIEVEMENT_DELETE_ALL)` |
| Academic curriculum | `canReadCurriculum = $can.hasAny(PERMISSIONS.ACADEMIC_CURRICULUM_READ_ALL, PERMISSIONS.ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE)`, `canManageCurriculumUnit = $can.has(PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT)`, `canManageCurriculumTree = $can.has(PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE)` |
| Academic structure | `canReadAcademicStructure = $can.has(PERMISSIONS.ACADEMIC_STRUCTURE_READ_ALL)`, `canManageAcademicStructure = $can.has(PERMISSIONS.ACADEMIC_STRUCTURE_MANAGE_ALL)` |
| Classrooms | `canReadClassrooms = $can.has(PERMISSIONS.ACADEMIC_CLASSROOM_READ_ALL)`, `canCreateClassrooms = $can.has(PERMISSIONS.ACADEMIC_CLASSROOM_CREATE_ALL)`, `canUpdateClassrooms = $can.has(PERMISSIONS.ACADEMIC_CLASSROOM_UPDATE_ALL)`, `canDeleteClassrooms = $can.has(PERMISSIONS.ACADEMIC_CLASSROOM_DELETE_ALL)` |
| Enrollment | `canReadEnrollment = $can.has(PERMISSIONS.ACADEMIC_ENROLLMENT_READ_ALL)`, `canUpdateEnrollment = $can.has(PERMISSIONS.ACADEMIC_ENROLLMENT_UPDATE_ALL)` |
| Course planning | `canReadCoursePlan = $can.has(PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL)`, `canManageCoursePlan = $can.has(PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL)` |
| Admission | `canReadAdmission = $can.has(PERMISSIONS.ADMISSION_READ_ALL)`, `canManageAdmission = $can.has(PERMISSIONS.ADMISSION_MANAGE_ALL)`, `canVerifyAdmission = $can.has(PERMISSIONS.ADMISSION_VERIFY_ALL)`, `canScoreAdmission = $can.has(PERMISSIONS.ADMISSION_SCORES_ALL)`, `canEnrollAdmission = $can.has(PERMISSIONS.ADMISSION_ENROLL_ALL)` |
| Activity | `canReadActivity = $can.has(PERMISSIONS.ACTIVITY_READ_ALL)`, `canManageActivity = $can.has(PERMISSIONS.ACTIVITY_MANAGE_ALL)`, `canManageOwnActivity = $can.has(PERMISSIONS.ACTIVITY_MANAGE_OWN)`, `canManageActivityMembers = $can.has(PERMISSIONS.ACTIVITY_MANAGE_MEMBERS_ALL)` |
| Supervision | Preserve the existing `canRequest`, `canManageSchool`, `canManageRequests`, `canEvaluate`, `canApprove`, and `canReport` booleans in `staff/academic/supervision/+page.svelte`. |
| Facility | `canReadFacility = $can.has(PERMISSIONS.FACILITY_READ_ALL)`, `canCreateFacility = $can.has(PERMISSIONS.FACILITY_CREATE_ALL)`, `canUpdateFacility = $can.has(PERMISSIONS.FACILITY_UPDATE_ALL)`, `canDeleteFacility = $can.has(PERMISSIONS.FACILITY_DELETE_ALL)` |
| Organization work | `canCreateWork = $can.has(PERMISSIONS.ORGANIZATION_WORK_CREATE_OWN)`, `canApproveUnitWork = $can.has(PERMISSIONS.ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT)` |

- [ ] **Step 5: Gate API calls**

Before a page calls an API, make sure the current actor has the matching capability. If no capability exists, show an `Alert` with a permission message and skip the API call.

- [ ] **Step 6: Replace ad-hoc controls with shadcn-svelte**

Use this mapping:

| Current pattern | Replacement |
| --- | --- |
| custom clickable div/button-like element | `Button` |
| card-like bordered panel | `Card` family |
| custom table/grid for tabular data | `Table` family |
| status pill or role pill | `Badge` |
| error or forbidden panel | `Alert` |
| modal | `Dialog` |
| tab bar | `Tabs` family |
| dropdown/select | `Select` or `Command` depending on search needs |
| binary setting | `Switch` or `Checkbox` |
| hover label for icon-only action | `Tooltip` |

- [ ] **Step 7: Verify static tests fail then pass**

Run after adding the expectation and before changing the route:

```bash
cd frontend-school
npm run test:static
```

Expected: FAIL for the current wave route gate.

Run after implementation:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

Expected: all pass.

- [ ] **Step 8: Run backend checks when backend files changed**

Run:

```bash
cd backend-school
cargo test --test static_architecture
cargo check
```

Expected: all pass.

- [ ] **Step 9: Commit the wave**

Use one commit per wave:

```bash
git status --short
git add -p
git commit -m "feat: standardize module permission UI wave"
```

Before committing, replace the generic commit subject with the concrete wave name, for example `feat: standardize admission permission UI`.

## Task 8: Wave 2 Roles And Organization

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/roles/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/roles/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/organization/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/organization/[id]/+page.svelte`
- Keep route gates: `frontend-school/src/routes/(app)/staff/roles/+page.ts`, `frontend-school/src/routes/(app)/staff/organization/+page.ts`

- [ ] Apply Task 7 with target module `ROLES`.
- [ ] Gate create/update/delete role controls with `ROLES_CREATE_ALL`, `ROLES_UPDATE_ALL`, `ROLES_DELETE_ALL`.
- [ ] Gate role assignment controls with `ROLES_ASSIGN_ALL` and role removal controls with `ROLES_REMOVE_ALL`.
- [ ] Keep organization work approval controls gated by `ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT`.
- [ ] Replace remaining ad-hoc tables, role pills, and dialogs with `Table`, `Badge`, `Dialog`, `Alert`, and `Card`.
- [ ] Run frontend checks and commit with `feat: standardize roles permission UI`.

## Task 9: Wave 3 Menu, Features, And Settings

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/menu/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/menu/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/features/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/features/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/school-settings/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/school-settings/+page.svelte`

- [ ] Change `staff/menu/+page.ts` to `PERMISSION_MODULES.MENU`.
- [ ] Change `staff/features/+page.ts` to `PERMISSION_MODULES.FEATURES`.
- [ ] Change `staff/school-settings/+page.ts` to `PERMISSION_MODULES.SETTINGS`.
- [ ] Gate menu read/create/update/delete controls with `MENU_READ_ALL`, `MENU_CREATE_ALL`, `MENU_UPDATE_ALL`, `MENU_DELETE_ALL`.
- [ ] Gate feature read/update controls with `FEATURES_READ_ALL`, `FEATURES_UPDATE_ALL`.
- [ ] Gate school settings read/update controls with `SETTINGS_READ_ALL`, `SETTINGS_UPDATE_ALL`.
- [ ] Use `Switch` for boolean toggles and `Alert` for disabled/forbidden states.
- [ ] Run frontend checks and backend checks if menu/system handlers changed.
- [ ] Commit with `feat: standardize system settings permission UI`.

## Task 10: Waves 4 Through 15

Use Task 7 for each wave below in order.

- [ ] Wave 4 Students: `PERMISSION_MODULES.STUDENT`, exact PII gates through `PERMISSION_MODULES.STUDENT_PII` and `PERMISSIONS.STUDENT_PII_READ_*`.
- [ ] Wave 5 Achievements: `PERMISSION_MODULES.ACHIEVEMENT`, own/all gates for create/update/delete/read.
- [ ] Wave 6 Academic curriculum: `PERMISSION_MODULES.ACADEMIC_CURRICULUM`, organization-unit/tree/all scope gates.
- [ ] Wave 7 Academic structure: `PERMISSION_MODULES.ACADEMIC_STRUCTURE`, read/manage gates.
- [ ] Wave 8 Classrooms and enrollment: `PERMISSION_MODULES.ACADEMIC_CLASSROOM` and `PERMISSION_MODULES.ACADEMIC_ENROLLMENT`.
- [ ] Wave 9 Course planning and timetable: `PERMISSION_MODULES.ACADEMIC_COURSE_PLAN`, read-only views for `ACADEMIC_COURSE_PLAN_READ_ALL`, mutation controls for `ACADEMIC_COURSE_PLAN_MANAGE_ALL`.
- [ ] Wave 10 Admission: `PERMISSION_MODULES.ADMISSION`, panels gated by `ADMISSION_READ_ALL`, `ADMISSION_MANAGE_ALL`, `ADMISSION_VERIFY_ALL`, `ADMISSION_SCORES_ALL`, `ADMISSION_ENROLL_ALL`.
- [ ] Wave 11 Activity: `PERMISSION_MODULES.ACTIVITY`, staff gates for `ACTIVITY_READ_ALL`, `ACTIVITY_MANAGE_ALL`, `ACTIVITY_MANAGE_OWN`, `ACTIVITY_MANAGE_MEMBERS_ALL`; keep student route user-type based.
- [ ] Wave 12 Supervision: `PERMISSION_MODULES.SUPERVISION`, preserve existing canRequest/canManage/canEvaluate/canApprove/canReport model.
- [ ] Wave 13 Facility: `PERMISSION_MODULES.FACILITY`, read/create/update/delete gates.
- [ ] Wave 14 Organization work: `PERMISSION_MODULES.ORGANIZATION_WORK` where module checks are needed; keep `workflowManage` for `/staff/work/manage`.
- [ ] Wave 15 Dashboard and self views: keep user-type/self-view gates; do not add admin permissions to self routes.

After each wave:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

If backend files changed:

```bash
cd backend-school
cargo test --test static_architecture
cargo check
```

## Final Verification

- [ ] Run all frontend static and type checks:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

- [ ] Run backend checks:

```bash
cd backend-school
cargo test --test static_architecture
cargo check
```

- [ ] Run diff hygiene checks from repo root:

```bash
git diff --check
git status --short
```

- [ ] Browser smoke checks:
  - Staff actor with only `staff_profile.read.organization_unit` sees `staff/manage`, scoped list rows, no create/edit/delete buttons.
  - Staff actor with `staff.create.all` and profile read sees create button.
  - Staff actor with `staff.update.all` sees edit buttons.
  - Staff actor with `staff.delete.all` sees delete buttons and confirmation dialog.
  - Staff actor without `staff_pii.read.*` does not see `national_id`.
  - Admin actor with wildcard sees every module route and every permitted action.
  - Read-only academic actor can enter module workspaces without mutation controls.
  - Score-only admission actor sees score workflow without unrelated management actions.

## Plan Self-Review

- Spec coverage: The plan covers module-level route/menu gates, page-level progressive capabilities, backend authorization boundaries, shadcn-svelte usage, `staff/manage` first, and rollout across all current permission modules.
- Placeholder scan: No unresolved placeholder markers are used. Module-specific route gates and capability constants are named directly in the rollout tables.
- Type consistency: The plan uses existing `PERMISSION_MODULES`, `PERMISSIONS`, `$can`, `_meta.menu`, `_meta.access`, and local shadcn-svelte exports already present in the repository.
