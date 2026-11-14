import type { ExamScheduleWorkspace, UpdateExamRoundInput } from '../../src/lib/api/examSchedule';
import { sessionState } from './fixtures/session-state';
import { revalidatePage } from './fixtures/page-revalidation';
import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const yearId = '10000000-0000-4000-8000-000000000011';
const termId = '20000000-0000-4000-8000-000000000011';
const firstRoundId = '30000000-0000-4000-8000-000000000011';
const secondRoundId = '30000000-0000-4000-8000-000000000012';
const detailUrl = (roundId = firstRoundId) =>
	`/staff/academic/exam-schedules/${roundId}?academicYearId=${yearId}&academicTermId=${termId}`;
const listUrl = `/staff/academic/exam-schedules?academicYearId=${yearId}&academicTermId=${termId}`;

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

function workspace(roundId = firstRoundId, name = 'รอบแรก'): ExamScheduleWorkspace {
	return {
		round: {
			id: roundId,
			academicYearId: yearId,
			academicTermId: termId,
			name,
			description: null,
			examKind: 'midterm',
			rowVersion: 1,
			createdAt: '2026-05-01T00:00:00Z',
			updatedAt: '2026-05-01T00:00:00Z'
		},
		days: [],
		unscheduledItems: [],
		scheduledSessions: [],
		paperReceiptItems: [],
		readiness: { complete: false, findings: [{ code: 'missing_exam_day', count: 1 }] }
	};
}

function workspaceWithDay(): ExamScheduleWorkspace {
	return {
		...workspace(),
		days: [
			{
				id: '40000000-0000-4000-8000-000000000011',
				examRoundId: firstRoundId,
				examDate: '2026-06-01',
				label: null,
				startTime: '08:30:00',
				endTime: '16:00:00',
				gradeLevelIds: [],
				blockedWindows: [],
				roomAssignments: []
			}
		]
	};
}

async function installShell(page: Page, permissions = ['*']) {
	await page.route('**/api/**', async (route) => {
		const pathname = new URL(route.request().url()).pathname;
		if (pathname === '/api/auth/me') {
			await fulfill(route, {
				id: '90000000-0000-4000-8000-000000000011',
				username: 'exam-manager',
				firstName: 'ทดสอบ',
				lastName: 'ตารางสอบ',
				userType: 'staff',
				status: 'ACTIVE',
				session: sessionState(),
				permissions
			});
			return;
		}
		if (pathname === '/api/academic/context/options') {
			await fulfill(route, {
				defaultAcademicYearId: yearId,
				defaultAcademicTermId: termId,
				years: [
					{
						isVisible: true,
						id: yearId,
						name: 'ปีการศึกษา 2569',
						year: 2569,
						status: 'active'
					}
				],
				terms: [
					{
						isVisible: true,
						id: termId,
						academicYearId: yearId,
						name: 'ภาคเรียนที่ 1',
						code: '1',
						sequence: 1,
						termType: 'regular',
						status: 'active',
						includedInYearResult: true,
						blocksYearClosure: true
					}
				]
			});
			return;
		}
		if (pathname === '/api/menu/user') {
			await fulfill(route, { groups: [] });
			return;
		}
		await fulfill(route, []);
	});
}

test('workspace paints while grade options wait and room options stay lazy', async ({ page }) => {
	await installShell(page);
	const pendingWorkspace: Route[] = [];
	const pendingGrades: Route[] = [];
	const roomReads: string[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) => {
		pendingWorkspace.push(route);
	});
	await page.route('**/api/lookup/grade-levels?**', (route) => {
		pendingGrades.push(route);
	});
	await page.route('**/api/lookup/homerooms?**', (route) => {
		roomReads.push('homerooms');
		return fulfill(route, []);
	});
	await page.route('**/api/lookup/rooms?**', (route) => {
		roomReads.push('rooms');
		return fulfill(route, []);
	});
	await page.goto(detailUrl());
	await expect.poll(() => pendingWorkspace.length).toBe(1);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pendingWorkspace[0], workspace());
	await expect.poll(() => pendingGrades.length).toBe(1);
	await expect(
		page.getByRole('button', { name: 'มีรายการที่ควรตรวจ 1', exact: true })
	).toBeVisible();
	expect(roomReads).toEqual([]);
	await fulfill(pendingGrades[0], []);
	await page.getByRole('tab', { name: 'ห้องสอบ' }).click();
	await expect.poll(() => roomReads.sort()).toEqual(['homerooms', 'rooms']);
});

test('list tap preloads the same contextual detail destination used by navigation', async ({
	page
}) => {
	await installShell(page);
	let detailReads = 0;
	await page.route('**/api/academic/exam-schedules?**', (route) =>
		fulfill(route, [workspace().round])
	);
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) => {
		detailReads += 1;
		return fulfill(route, workspace());
	});
	await page.goto(listUrl);
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	const row = page.getByRole('row').filter({ hasText: 'รอบแรก' });
	await row.hover();
	expect(detailReads).toBe(0);
	await row.dispatchEvent('pointerdown');
	await expect.poll(() => detailReads).toBe(1);
	await row.getByRole('button', { name: 'เปิด' }).click();
	await expect(page).toHaveURL(
		new RegExp(`/${firstRoundId}\\?academicYearId=${yearId}&academicTermId=${termId}`)
	);
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	expect(detailReads).toBe(1);
});

test('round navigation clears old workspace before the next result and keeps scoped retry', async ({
	page
}) => {
	await installShell(page);
	const pending: Route[] = [];
	await page.route(
		'**/api/academic/exam-schedules/30000000-0000-4000-8000-0000000000*',
		(route) => {
			pending.push(route);
		}
	);
	await page.goto(detailUrl());
	await expect.poll(() => pending.length).toBe(1);
	await fulfill(pending[0], workspace());
	await expect(
		page.getByRole('button', { name: 'มีรายการที่ควรตรวจ 1', exact: true })
	).toBeVisible();
	await page.evaluate((destination) => {
		const link = document.createElement('a');
		link.href = destination;
		link.id = 'test-exam-detail-navigation';
		link.textContent = 'เปิดรอบที่สอง';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, detailUrl(secondRoundId));
	await page.locator('#test-exam-detail-navigation').click();
	await expect.poll(() => pending.length).toBe(2);
	await expect(page.getByText('รอบแรก', { exact: true })).toHaveCount(0);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await pending[1].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'รอบที่สองยังไม่พร้อม' })
	});
	await expect(page.getByText('รอบที่สองยังไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect.poll(() => pending.length).toBe(3);
	await fulfill(pending[2], workspace(secondRoundId, 'รอบที่สอง'));
	await expect(page.getByText('รอบที่สอง', { exact: true }).first()).toBeVisible();
});

test('grade and room option failures have separate retries without replacing the workspace', async ({
	page
}) => {
	await installShell(page);
	const grades: Route[] = [];
	const homerooms: Route[] = [];
	const rooms: Route[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspace())
	);
	await page.route('**/api/lookup/grade-levels?**', (route) => grades.push(route));
	await page.route('**/api/lookup/homerooms?**', (route) => homerooms.push(route));
	await page.route('**/api/lookup/rooms?**', (route) => rooms.push(route));
	await page.goto(detailUrl());
	await expect.poll(() => grades.length).toBe(1);
	await grades[0].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'ระดับชั้นชั่วคราวไม่พร้อม' })
	});
	await expect(
		page.getByRole('button', { name: 'มีรายการที่ควรตรวจ 1', exact: true })
	).toBeVisible();
	await expect(page.getByText('ระดับชั้นชั่วคราวไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect.poll(() => grades.length).toBe(2);
	await fulfill(grades[1], []);
	await expect(page.getByTestId('exam-grade-levels-ready')).toBeVisible();
	await page.getByRole('tab', { name: 'ห้องสอบ' }).click();
	await expect.poll(() => homerooms.length).toBe(1);
	await expect.poll(() => rooms.length).toBe(1);
	await expect(page.getByRole('tab', { name: 'ห้องสอบ' })).toHaveAttribute('data-state', 'active');
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(homerooms[0], []);
	await rooms[0].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'ห้องสอบชั่วคราวไม่พร้อม' })
	});
	await expect(page.getByText('ห้องสอบชั่วคราวไม่พร้อม')).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'มีรายการที่ควรตรวจ 1', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect.poll(() => homerooms.length).toBe(2);
	await expect.poll(() => rooms.length).toBe(2);
	await Promise.all([fulfill(homerooms[1], []), fulfill(rooms[1], [])]);
	await expect(page.getByText('ห้องสอบและที่นั่ง')).toBeVisible();
});

test('manual workspace refresh retains the current round and reports its own failure', async ({
	page
}) => {
	await installShell(page);
	const requests: Route[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		requests.push(route)
	);
	await page.goto(detailUrl());
	await expect.poll(() => requests.length).toBe(1);
	await fulfill(requests[0], workspace());
	const ready = page.getByTestId('exam-detail-ready');
	await expect(ready).toBeVisible();
	await revalidatePage(page);
	await expect.poll(() => requests.length).toBe(2);
	await expect(ready).toHaveAttribute('aria-busy', 'true');
	await expect(
		page.getByRole('button', { name: 'มีรายการที่ควรตรวจ 1', exact: true })
	).toBeVisible();
	await requests[1].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'รีเฟรชรอบสอบไม่สำเร็จ' })
	});
	const refreshError = ready.getByRole('alert').filter({ hasText: 'รีเฟรชรอบสอบไม่สำเร็จ' });
	await expect(refreshError).toBeVisible();
	await ready.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect.poll(() => requests.length).toBe(3);
	await fulfill(requests[2], workspace(firstRoundId, 'รอบที่อัปเดต'));
	await expect(page.getByText('รอบที่อัปเดต', { exact: true }).first()).toBeVisible();
	await expect(refreshError).toHaveCount(0);
});

test('a late direct round-kind save cannot replace a different round after navigation', async ({
	page
}) => {
	await installShell(page);
	const pending: Route[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) => {
		if (route.request().method() === 'PATCH') {
			pending.push(route);
			return;
		}
		return fulfill(route, workspace());
	});
	await page.route(`**/api/academic/exam-schedules/${secondRoundId}`, (route) =>
		fulfill(route, workspace(secondRoundId, 'รอบที่สอง'))
	);
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	await page
		.getByRole('combobox')
		.filter({ hasText: /กลางภาค/ })
		.click();
	await page.getByRole('option', { name: 'ปลายภาค', exact: true }).click();
	await expect.poll(() => pending.length).toBe(1);
	expect(pending[0].request().postDataJSON()).toEqual({ rowVersion: 1, examKind: 'final' });
	await page.evaluate((destination) => {
		const link = document.createElement('a');
		link.href = destination;
		link.id = 'test-exam-kind-navigation';
		link.textContent = 'เปิดรอบที่สอง';
		link.dataset.sveltekitPreloadData = 'off';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, detailUrl(secondRoundId));
	await page.locator('#test-exam-kind-navigation').click();
	await expect(page.getByRole('heading', { name: 'รอบที่สอง', exact: true })).toBeVisible();
	await fulfill(pending[0], { ...workspace().round, rowVersion: 2, examKind: 'final' });
	await expect(page.getByRole('heading', { name: 'รอบที่สอง', exact: true })).toBeVisible();
	await expect(page.getByRole('combobox').filter({ hasText: /กลางภาค/ })).toBeVisible();
	await expect(page.getByText('รอบแรก', { exact: true })).toHaveCount(0);
});

test('a direct kind save sends its accepted revision and retains the accepted round while current sources refresh', async ({
	page
}) => {
	await installShell(page);
	const reads: Route[] = [],
		writes: UpdateExamRoundInput[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) => {
		if (route.request().method() === 'PATCH') {
			writes.push(route.request().postDataJSON());
			return fulfill(route, { ...workspace().round, rowVersion: 2, examKind: 'final' });
		}
		reads.push(route);
	});
	await page.goto(detailUrl());
	await expect.poll(() => reads.length).toBe(1);
	await fulfill(reads[0], workspace());
	const ready = page.getByTestId('exam-detail-ready');
	await expect(ready).toBeVisible();
	await page
		.getByRole('combobox')
		.filter({ hasText: /กลางภาค/ })
		.click();
	await page.getByRole('option', { name: 'ปลายภาค', exact: true }).click();
	await expect.poll(() => reads.length).toBe(2);
	await expect(ready).toBeVisible();
	await expect(ready).toHaveAttribute('aria-busy', 'true');
	await expect(
		page.locator('[data-slot="badge"]').filter({ hasText: 'ปลายภาค' }).first()
	).toBeVisible();
	expect(writes).toEqual([{ rowVersion: 1, examKind: 'final' }]);
	const fresh = workspace();
	fresh.round = { ...fresh.round, rowVersion: 2, examKind: 'final' };
	await fulfill(reads[1], fresh);
	await expect(ready).toHaveAttribute('aria-busy', 'false');
	await expect(page.getByRole('combobox').filter({ hasText: /ปลายภาค/ })).toBeEnabled();
	expect(reads).toHaveLength(2);
});

test('invigilator staff options stay tab-lazy and recover from a failed read', async ({ page }) => {
	await installShell(page);
	const staffRequests: Route[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspaceWithDay())
	);
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}/invigilators`, (route) =>
		fulfill(route, { roundId: firstRoundId, assignments: [], staffWorkloads: [] })
	);
	await page.route(
		`**/api/academic/exam-schedules/${firstRoundId}/invigilator-staff-options?**`,
		(route) => {
			staffRequests.push(route);
		}
	);
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	expect(staffRequests).toHaveLength(0);
	await page.getByRole('tab', { name: 'กรรมการ' }).click();
	await expect.poll(() => staffRequests.length).toBe(1);
	await staffRequests[0].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'รายชื่อครูยังไม่พร้อม' })
	});
	await expect(page.getByRole('button', { name: 'ลองโหลดครูอีกครั้ง' })).toBeVisible();
	await expect(page.getByText('ไม่พบรายชื่อครู')).toHaveCount(0);
	await page.getByRole('button', { name: 'ลองโหลดครูอีกครั้ง' }).click();
	await expect.poll(() => staffRequests.length).toBe(2);
	await fulfill(staffRequests[1], []);
	await expect(page.getByRole('button', { name: 'ลองโหลดครูอีกครั้ง' })).toHaveCount(0);
});

test('invigilator tab shows a focused skeleton before its workspace arrives', async ({ page }) => {
	await installShell(page);
	const invigilators: Route[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspace())
	);
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}/invigilators`, (route) => {
		invigilators.push(route);
	});
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	await page.getByRole('tab', { name: 'กรรมการ' }).click();
	await expect.poll(() => invigilators.length).toBe(1);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(invigilators[0], { roundId: firstRoundId, assignments: [], staffWorkloads: [] });
	await expect(
		page.locator('[role="tabpanel"][data-state="active"]').getByRole('heading', {
			name: 'ยังไม่มีวันสอบ'
		})
	).toBeVisible();
});

test('staff list shows a skeleton while its independent options request is pending', async ({
	page
}) => {
	await installShell(page);
	const staffRequests: Route[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspaceWithDay())
	);
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}/invigilators`, (route) =>
		fulfill(route, { roundId: firstRoundId, assignments: [], staffWorkloads: [] })
	);
	await page.route(
		`**/api/academic/exam-schedules/${firstRoundId}/invigilator-staff-options?**`,
		(route) => staffRequests.push(route)
	);
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	await page.getByRole('tab', { name: 'กรรมการ' }).click();
	await expect.poll(() => staffRequests.length).toBe(1);
	await expect(page.getByText('จัดกรรมการคุมสอบ')).toBeVisible();
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(staffRequests[0], []);
	await expect(page.getByText('ไม่พบรายชื่อครู')).toBeVisible();
});

test('read-only rooms tab does not request management options', async ({ page }) => {
	await installShell(page, ['academic_exam_schedule.read.school']);
	const managementReads: string[] = [];
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspace())
	);
	await page.route('**/api/lookup/homerooms?**', (route) => {
		managementReads.push('homerooms');
		return fulfill(route, []);
	});
	await page.route('**/api/lookup/rooms?**', (route) => {
		managementReads.push('rooms');
		return fulfill(route, []);
	});
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	await page.getByRole('tab', { name: 'ห้องสอบ' }).click();
	await expect(page.getByText('ห้องสอบและที่นั่ง')).toBeVisible();
	expect(managementReads).toEqual([]);
});

test('current room editing requests its management options only when the room tab opens', async ({
	page
}) => {
	await installShell(page);
	const managementReads: string[] = [];
	const currentWorkspace = workspaceWithDay();
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, currentWorkspace)
	);
	await page.route('**/api/lookup/homerooms?**', (route) => {
		managementReads.push('homerooms');
		return fulfill(route, []);
	});
	await page.route('**/api/lookup/rooms?**', (route) => {
		managementReads.push('rooms');
		return fulfill(route, []);
	});
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	expect(managementReads).toEqual([]);
	await page.getByRole('tab', { name: 'ห้องสอบ' }).click();
	await expect(page.getByText('ห้องสอบและที่นั่ง')).toBeVisible();
	await expect.poll(() => managementReads.length).toBe(2);
	expect(managementReads.sort()).toEqual(['homerooms', 'rooms']);
});

test('export-only data waits until the user clicks export', async ({ page }) => {
	await installShell(page);
	let exportInvigilatorReads = 0;
	let exportHomeroomReads = 0;
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspace())
	);
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}/invigilators`, (route) => {
		exportInvigilatorReads += 1;
		return fulfill(route, { roundId: firstRoundId, assignments: [], staffWorkloads: [] });
	});
	await page.route('**/api/lookup/homerooms?**', (route) => {
		exportHomeroomReads += 1;
		return fulfill(route, []);
	});
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	expect(exportInvigilatorReads).toBe(0);
	expect(exportHomeroomReads).toBe(0);
	await page.getByRole('button', { name: 'ส่งออก' }).click();
	await expect.poll(() => exportInvigilatorReads).toBe(1);
	await expect.poll(() => exportHomeroomReads).toBe(1);
});

test('read-only export loads homeroom labels only after the export action', async ({ page }) => {
	await installShell(page, ['academic_exam_schedule.read.school']);
	let homeroomReads = 0;
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
		fulfill(route, workspace())
	);
	await page.route(`**/api/academic/exam-schedules/${firstRoundId}/invigilators`, (route) =>
		fulfill(route, { roundId: firstRoundId, assignments: [], staffWorkloads: [] })
	);
	await page.route('**/api/lookup/homerooms?**', (route) => {
		homeroomReads += 1;
		return fulfill(route, []);
	});
	await page.goto(detailUrl());
	await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
	expect(homeroomReads).toBe(0);
	await page.getByRole('button', { name: 'ส่งออก' }).click();
	await expect.poll(() => homeroomReads).toBe(1);
});

for (const viewport of [
	{ name: 'mobile', width: 390, height: 844 },
	{ name: 'desktop', width: 1280, height: 900 }
]) {
	for (const colorScheme of ['light', 'dark'] as const) {
		test(`room seats use student numbers without generation controls: ${viewport.name} ${colorScheme}`, async ({
			page
		}) => {
			await page.setViewportSize(viewport);
			await page.emulateMedia({ colorScheme });
			await installShell(page);
			const assignment = {
				id: '50000000-0000-4000-8000-000000000011',
				examDayId: '40000000-0000-4000-8000-000000000011',
				homeroomId: '60000000-0000-4000-8000-000000000011',
				homeroomName: 'ม.1/1',
				roomId: '70000000-0000-4000-8000-000000000011',
				roomName: '316',
				buildingName: 'อาคารเรียน',
				roomCapacity: 40,
				capacityOverride: null,
				invigilators: [],
				seatsGenerated: false
			};
			const data = workspaceWithDay();
			await page.route(`**/api/academic/exam-schedules/${firstRoundId}`, (route) =>
				fulfill(route, {
					...data,
					days: data.days.map((day) => ({ ...day, roomAssignments: [assignment] }))
				})
			);
			await page.route('**/api/lookup/homerooms?**', (route) =>
				fulfill(route, [{ id: assignment.homeroomId, name: 'ม.1/1' }])
			);
			await page.route('**/api/lookup/rooms?**', (route) =>
				fulfill(route, [
					{ id: assignment.roomId, name_th: '316', building_name: 'อาคารเรียน', capacity: 40 }
				])
			);
			const writes: string[] = [];
			const pendingSaves: Route[] = [];
			await page.route('**/api/academic/exam-schedules/days/*/room-assignments', (route) => {
				writes.push(new URL(route.request().url()).pathname);
				pendingSaves.push(route);
			});
			await page.route('**/api/academic/exam-schedules/room-assignments/*/seats', async (route) => {
				writes.push('manual-seats');
				await fulfill(route, []);
			});
			await page.goto(detailUrl());
			await expect(page.getByTestId('exam-detail-ready')).toBeVisible();
			await page.getByRole('tab', { name: 'ห้องสอบ', exact: true }).click();
			await expect(
				page.getByText('เลขที่นั่งสอบใช้เลขที่นักเรียนในห้องเรียนโดยอัตโนมัติ')
			).toBeVisible();
			await expect(page.getByRole('button', { name: 'สร้างเลขที่นั่ง' })).toHaveCount(0);
			const row = page.getByRole('row').filter({ hasText: 'ม.1/1' });
			await expect(row).toBeVisible();
			await page.screenshot({
				path: `test-results/exam-auto-seats-${viewport.name}-${colorScheme}.png`,
				fullPage: true
			});
			await row.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			await page.getByRole('button', { name: 'บันทึกห้องสอบ', exact: true }).click();
			await expect.poll(() => pendingSaves.length).toBe(1);
			await expect(page.getByRole('button', { name: 'กำลังบันทึก...' })).toBeDisabled();
			await fulfill(pendingSaves[0], { ...assignment, seatsGenerated: true });
			await expect(page.getByRole('dialog')).toHaveCount(0);
			expect(writes).toHaveLength(1);
			expect(writes).not.toContain('manual-seats');
			await expect(row).toBeVisible();
		});
	}
}
