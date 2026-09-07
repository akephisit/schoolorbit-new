import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const ids = {
	year: '10000000-0000-4000-8000-000000000001',
	term: '20000000-0000-4000-8000-000000000001',
	teacher: '30000000-0000-4000-8000-000000000001',
	subject: '40000000-0000-4000-8000-000000000001',
	offering: '50000000-0000-4000-8000-000000000001',
	group: '60000000-0000-4000-8000-000000000001',
	phase: '70000000-0000-4000-8000-000000000001',
	item: '80000000-0000-4000-8000-000000000001',
	student: '90000000-0000-4000-8000-000000000001',
	membership: '91000000-0000-4000-8000-000000000001'
};

const phaseCodes = ['before_midterm', 'midterm', 'after_midterm', 'final'] as const;

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(
			status < 400 ? { success: true, data } : { success: false, error: String(data) }
		)
	});
}

function contextOptions() {
	return {
		activeAcademicYearId: ids.year,
		activeAcademicTermId: ids.term,
		years: [
			{
				id: ids.year,
				name: 'ปีการศึกษา 2569',
				year: 2569,
				status: 'active',
				startDate: '2026-05-01',
				endDate: '2027-03-31'
			}
		],
		terms: [
			{
				id: ids.term,
				academicYearId: ids.year,
				name: 'ภาคเรียนที่ 1',
				code: '1',
				sequence: 1,
				termType: 'regular',
				status: 'active',
				startDate: '2026-05-01',
				endDate: '2026-10-31',
				includedInYearResult: true,
				blocksYearClosure: true
			}
		]
	};
}

function subject() {
	return {
		subjectId: ids.subject,
		learningGroupId: ids.group,
		learningOfferingId: ids.offering,
		code: 'ค21101',
		name: 'คณิตศาสตร์พื้นฐาน',
		groupName: 'ม.1/1',
		assigned: true,
		phases: phaseCodes.map((phaseCode, index) => ({
			id: index === 0 ? ids.phase : `70000000-0000-4000-8000-00000000000${index + 1}`,
			phaseCode,
			maxScore: ['20', '20', '30', '30'][index],
			rowVersion: 1
		}))
	};
}

function workspace(canManage: boolean) {
	return {
		learningGroupId: ids.group,
		learningOfferingId: ids.offering,
		assessmentPhaseId: ids.phase,
		phaseCode: 'before_midterm',
		phaseMaxScore: '20',
		phaseRowVersion: 1,
		scoreEntryEnabled: true,
		locked: false,
		canManage,
		canConfirm: canManage,
		items: [
			{
				id: ids.item,
				name: 'ชีท 1',
				maxScore: '20',
				displayOrder: 1,
				lifecycle: 'active',
				rowVersion: 1
			}
		],
		students: [
			{
				membershipId: ids.membership,
				studentAcademicYearId: ids.student,
				displayName: 'เด็กชายทดสอบ ระบบ',
				rowVersion: 1
			}
		],
		scores: [],
		sourceChecksum: 'source-1',
		rosterChecksum: 'roster-1',
		confirmation: null,
		confirmationIsCurrent: false
	};
}

async function mockGradebook(page: Page, canManage: boolean) {
	const controlRequests: string[] = [];
	const scoreBodies: unknown[] = [];
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.teacher,
					username: 'teacher',
					firstName: 'ครู',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-09-01T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: canManage
						? ['academic_gradebook.read.school', 'academic_gradebook.manage.school']
						: ['academic_gradebook.read.assigned']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, contextOptions());
				return;
			}
			if (url.pathname === '/api/academic/gradebook/subjects') {
				await fulfill(route, [subject()]);
				return;
			}
			if (url.pathname === '/api/academic/learner-evaluations/subjects') {
				await fulfill(route, []);
				return;
			}
			if (
				url.pathname === '/api/academic/gradebook/controls' ||
				url.pathname === '/api/academic/learner-evaluations/controls'
			) {
				controlRequests.push(url.pathname);
				await fulfill(
					route,
					url.pathname.endsWith('gradebook/controls')
						? phaseCodes.map((phaseCode, index) => ({
								id: `92000000-0000-4000-8000-00000000000${index + 1}`,
								academicYearId: ids.year,
								academicTermId: ids.term,
								phaseCode,
								scoreEntryEnabled: true,
								rowVersion: 1
							}))
						: []
				);
				return;
			}
			if (url.pathname.includes(`/api/academic/gradebook/groups/${ids.group}/phases/`)) {
				if (request.method() === 'PUT' && url.pathname.endsWith('/scores')) {
					const body = request.postDataJSON() as {
						cells: Array<{
							operation: 'set' | 'clear';
							scoreItemId: string;
							studentAcademicYearId: string;
							value?: string;
						}>;
					};
					scoreBodies.push(body);
					await fulfill(route, {
						cells: body.cells.map((cell) => ({
							scoreItemId: cell.scoreItemId,
							studentAcademicYearId: cell.studentAcademicYearId,
							value: cell.operation === 'set' ? cell.value : null,
							rowVersion: cell.operation === 'set' ? 2 : null
						})),
						workspaceRevision: 'source-2'
					});
					return;
				}
				await fulfill(route, workspace(canManage));
				return;
			}
			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}
			if (url.pathname === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			if (url.pathname === '/api/me/work-items/counts') {
				await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
				return;
			}
			if (url.pathname === '/api/notifications') {
				await fulfill(route, { items: [], unread_count: 0 });
				return;
			}
			if (url.pathname === '/api/school/settings') {
				await fulfill(route, 'forbidden', 403);
				return;
			}
			await fulfill(route, {});
		}
	);
	return { controlRequests, scoreBodies };
}

function gradebookUrl() {
	return `/staff/academic/gradebook?academicYearId=${ids.year}&academicTermId=${ids.term}`;
}

test('teacher selects a score column and autosaves an explicit zero', async ({ page }) => {
	const observed = await mockGradebook(page, true);
	await page.goto(gradebookUrl());

	await expect(page.getByRole('heading', { name: 'รายการคะแนนในช่วงนี้' })).toBeVisible();
	await page.getByRole('checkbox').first().click();
	await page.getByLabel('ชีท 1 เด็กชายทดสอบ ระบบ').fill('0');
	await page.getByLabel('ชีท 1 เด็กชายทดสอบ ระบบ').blur();
	await expect.poll(() => observed.scoreBodies.length).toBe(1);
	expect(observed.scoreBodies[0]).toMatchObject({
		cells: [
			{
				operation: 'set',
				scoreItemId: ids.item,
				studentAcademicYearId: ids.student,
				value: '0'
			}
		]
	});
});

test('mobile editor has an explicit close action', async ({ page }) => {
	await mockGradebook(page, true);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto(gradebookUrl());

	await page.getByRole('button', { name: 'เลือกทุกช่อง' }).click();
	await page.getByRole('button', { name: 'กรอก' }).click();
	await expect(page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' })).toBeVisible();
	await page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' }).click();
	await expect(page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' })).toBeHidden();
});

test('read-only teacher never requests manager controls', async ({ page }) => {
	const observed = await mockGradebook(page, false);
	await page.goto(gradebookUrl());

	await expect(page.getByRole('heading', { name: 'รายการคะแนนในช่วงนี้' })).toBeVisible();
	expect(observed.controlRequests).toEqual([]);
});
