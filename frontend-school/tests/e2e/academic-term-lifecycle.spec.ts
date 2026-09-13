import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';

test.use({ serviceWorkers: 'block' });
type Workspace = components['schemas']['TermLifecycleWorkspace'];
type ActivationWorkspace = components['schemas']['TermActivationWorkspace'];
const year = '10000000-0000-4000-8000-000000000001';
const term = '20000000-0000-4000-8000-000000000001';
const nextYear = '10000000-0000-4000-8000-000000000002';
const nextTerm = '20000000-0000-4000-8000-000000000002';
const actor = '30000000-0000-4000-8000-000000000001';
const url = `/staff/academic/term-lifecycle?academicYearId=${year}&academicTermId=${term}`;

function workspace(reader = false): Workspace {
	return {
		context: {
			academicYearId: year,
			academicTermId: term,
			yearName: 'ปีการศึกษา 2569',
			termName: 'ภาคเรียนที่ 1',
			yearStatus: 'active',
			termStatus: 'active',
			yearRowVersion: 1,
			termRowVersion: 1,
			yearStartDate: '2026-05-01',
			yearEndDate: '2027-04-30',
			termStartDate: '2026-05-16',
			plannedEndDate: null,
			closedOn: null,
			sequence: 1,
			bellScheduleId: '40000000-0000-4000-8000-000000000001',
			includedInYearResult: true,
			blocksYearClosure: true
		},
		coverage: { students: [], ready: false, sourceChecksum: 'a'.repeat(64) },
		findings: [
			{
				code: 'results.incomplete',
				severity: 'blocking',
				count: 2,
				message: 'ผลสรุปนักเรียนยังไม่ครบ 2 คน',
				resolutionUrl: '/staff/academic/results'
			}
		],
		availableActions: reader ? [] : ['begin_closing'],
		sourceChecksum: 'b'.repeat(64)
	};
}

function activationWorkspace(opensYear = true): ActivationWorkspace {
	const context = workspace().context;
	return {
		context: {
			...context,
			yearStatus: opensYear ? 'planning' : 'active',
			termStatus: 'ready'
		},
		opensYear,
		predecessor: null,
		policy: {
			rowVersion: 1,
			requireHomeroomPlacements: true,
			requirePublishedOfferings: false,
			requirePublishedTimetable: false
		},
		plannedStudents: opensYear ? 1 : 0,
		eligiblePlacements: opensYear ? 1 : 0,
		findings: [],
		canActivate: true,
		sourceChecksum: 'd'.repeat(64)
	};
}

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(
	page: Page,
	reader = false,
	conflict = false,
	initial = workspace(reader),
	activation = activationWorkspace()
) {
	let current = initial;
	let openingPolicy: components['schemas']['OpeningPolicy'] = {
		...activation.policy
	};
	const writes: Array<components['schemas']['TermTransitionRequest']> = [];
	const openingPolicyWrites: Array<components['schemas']['UpdateOpeningPolicyInput']> = [];
	const preparationPreviews: Array<components['schemas']['PreviewTermPreparationInput']> = [];
	const preparationWrites: Array<components['schemas']['ApplyTermPreparationInput']> = [];
	let contextReads = 0;
	let activationReads = 0;
	let openingPolicyReads = 0;
	await page.route(
		(request) => request.pathname.startsWith('/api/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			if (path === '/api/auth/me') {
				const user: components['schemas']['CurrentUserResponse'] = {
					id: actor,
					username: 'E2E-LIFECYCLE-reader',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: reader
						? ['academic_lifecycle.read.school', 'academic_context.read.school']
						: ['*']
				};
				return fulfill(route, user);
			}
			if (path === '/api/academic/context/options') {
				contextReads++;
				const options: components['schemas']['AcademicContextOptions'] = {
					activeAcademicYearId: year,
					activeAcademicTermId: term,
					years: [
						{
							id: year,
							year: 2569,
							name: 'ปีการศึกษา 2569',
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-04-30'
						},
						{
							id: nextYear,
							year: 2570,
							name: 'ปีการศึกษา 2570',
							status: 'planning',
							startDate: '2027-05-01',
							endDate: '2028-04-30'
						}
					],
					terms: [
						{
							id: term,
							academicYearId: year,
							code: '1',
							name: 'ภาคเรียนที่ 1',
							sequence: 1,
							termType: 'regular',
							status: current.context.termStatus,
							startDate: '2026-05-16',
							plannedEndDate: null,
							closedOn: null,
							includedInYearResult: true,
							blocksYearClosure: true
						},
						{
							id: nextTerm,
							academicYearId: nextYear,
							code: '1',
							name: 'ภาคเรียนที่ 1',
							sequence: 1,
							termType: 'regular',
							status: 'planning',
							startDate: '2027-05-16',
							plannedEndDate: '2027-10-31',
							closedOn: null,
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				};
				return fulfill(route, options);
			}
			if (path === '/api/academic/lifecycle/term-preparations/preview') {
				const request: components['schemas']['PreviewTermPreparationInput'] = route
					.request()
					.postDataJSON();
				preparationPreviews.push(request);
				const result: components['schemas']['TermPreparationWorkspace'] = {
					context: {
						sourceTermId: term,
						sourceYearId: year,
						sourceLabel: 'ปีการศึกษา 2569 · ภาคเรียนที่ 1',
						sourceStatus: 'closed',
						targetTermId: nextTerm,
						targetYearId: nextYear,
						targetLabel: 'ปีการศึกษา 2570 · ภาคเรียนที่ 1',
						targetStatus: 'planning',
						targetStartDate: '2027-05-16',
						targetEndDate: '2027-10-31'
					},
					modules: request.modules.map((module) => ({
						module,
						sourceCount: 2,
						targetExistingCount: 0,
						draftCount: 2,
						status: 'ready',
						summary: 'สร้างแบบร่าง 2 รายการ'
					})),
					mappingRequirements: [],
					dateRequirements: [],
					findings: [],
					canApply: true,
					sourceChecksum: 'e'.repeat(64)
				};
				return fulfill(route, result);
			}
			if (path === '/api/academic/lifecycle/term-preparations/apply') {
				const request: components['schemas']['ApplyTermPreparationInput'] = route
					.request()
					.postDataJSON();
				preparationWrites.push(request);
				return fulfill(route, {
					runId: '50000000-0000-4000-8000-000000000001',
					requestId: request.requestId,
					sourceTermId: term,
					targetTermId: nextTerm,
					modules: request.modules.map((module) => ({
						module,
						createdCount: 2,
						targetIds: []
					})),
					createdAt: '2026-09-13T10:00:00Z'
				} satisfies components['schemas']['TermPreparationOutcome']);
			}
			if (path === `/api/academic/lifecycle/terms/${term}`) return fulfill(route, current);
			if (path === `/api/academic/lifecycle/terms/${term}/activation`) {
				activationReads++;
				return fulfill(route, activation);
			}
			if (path === '/api/academic/lifecycle/opening-policy') {
				if (route.request().method() === 'GET') {
					openingPolicyReads++;
					return fulfill(route, openingPolicy);
				}
				const request: components['schemas']['UpdateOpeningPolicyInput'] = route
					.request()
					.postDataJSON();
				openingPolicyWrites.push(request);
				openingPolicy = { ...request, rowVersion: request.rowVersion + 1 };
				return fulfill(route, openingPolicy);
			}
			if (path === `/api/academic/lifecycle/terms/${term}/transitions`) {
				const request: components['schemas']['TermTransitionRequest'] = route
					.request()
					.postDataJSON();
				writes.push(request);
				if (conflict)
					return fulfill(route, 'ข้อมูลภาคเรียนหรือความพร้อมเปลี่ยนแล้ว กรุณาตรวจสอบใหม่', 409);
				const statuses: Record<
					components['schemas']['TermTransitionAction'],
					components['schemas']['AcademicTermStatus']
				> = {
					mark_ready: 'ready',
					begin_closing: 'closing',
					cancel_closing: 'active',
					close: 'closed',
					reopen: 'closing',
					cancel: 'cancelled',
					activate: 'active'
				};
				current = {
					...current,
					context: {
						...current.context,
						termStatus: statuses[request.action],
						closedOn: request.closedOn ?? null,
						termRowVersion: 2,
						yearRowVersion: 2
					},
					availableActions: request.action === 'begin_closing' ? ['close', 'cancel_closing'] : [],
					sourceChecksum: 'c'.repeat(64)
				};
				return fulfill(route, {
					requestId: writes.at(-1)!.requestId,
					action: request.action,
					context: current.context,
					completedAt: '2026-09-11T10:00:00Z'
				} satisfies components['schemas']['TermTransitionOutcome']);
			}
			if (path === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === '/api/school/public')
				return fulfill(route, { schoolName: 'โรงเรียนทดสอบ', logoFileId: null });
			if (path === '/api/menu/user') return fulfill(route, { groups: [] });
			if (path === '/api/me/work-items/counts')
				return fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			if (path === '/api/notifications') return fulfill(route, { items: [], unread_count: 0 });
			return fulfill(route, 'ไม่เปิดใช้ในชุดทดสอบนี้', 403);
		}
	);
	return {
		writes,
		openingPolicyWrites,
		preparationPreviews,
		preparationWrites,
		contextReads: () => contextReads,
		activationReads: () => activationReads,
		openingPolicyReads: () => openingPolicyReads
	};
}

test('reader sees readiness without transition controls or mutation requests', async ({ page }) => {
	const observed = await mock(page, true);
	await page.goto(url);
	await expect(
		page.getByRole('heading', { name: 'ปิดและเปลี่ยนภาคเรียน', exact: true })
	).toBeVisible();
	await expect(page.getByText('ผลสรุปนักเรียนยังไม่ครบ 2 คน')).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'เริ่มขั้นตอนปิดภาคเรียน', exact: true })
	).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เกณฑ์เปิดภาคเรียน', exact: true })).toHaveCount(0);
	expect(observed.writes).toHaveLength(0);
});

test('manager edits optional opening gates in a lazy versioned dialog', async ({ page }) => {
	const observed = await mock(page);
	await page.goto(url);
	await page.getByRole('button', { name: 'เกณฑ์เปิดภาคเรียน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('heading', { name: 'เกณฑ์เสริมก่อนเปิดภาคเรียน' })).toBeVisible();
	await expect.poll(observed.openingPolicyReads).toBe(1);
	await dialog.getByRole('switch', { name: 'ต้องมีรายการเปิดสอนที่เผยแพร่' }).click();
	await dialog.getByRole('button', { name: 'บันทึกเกณฑ์', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(observed.openingPolicyWrites).toEqual([
		{
			rowVersion: 1,
			requireHomeroomPlacements: true,
			requirePublishedOfferings: true,
			requirePublishedTimetable: false
		}
	]);
});

test('begin closing requires confirmation and refreshes authoritative context without allowing incomplete closure', async ({
	page
}) => {
	const observed = await mock(page);
	await page.goto(url);
	await page.getByRole('button', { name: 'เริ่มขั้นตอนปิดภาคเรียน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	expect(observed.writes).toHaveLength(0);
	await dialog.getByRole('button', { name: 'ยืนยันเริ่มขั้นตอนปิดภาคเรียน', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect.poll(() => observed.writes.length).toBe(1);
	expect(observed.writes[0]).toMatchObject({
		academicYearId: year,
		action: 'begin_closing',
		expectedYearVersion: 1,
		expectedTermVersion: 1,
		readinessChecksum: 'b'.repeat(64),
		acknowledgedWarningCodes: [],
		closedOn: null,
		reason: null
	});
	await expect.poll(observed.contextReads).toBeGreaterThan(1);
	await expect(page.getByRole('button', { name: 'ปิดภาคเรียน', exact: true })).toBeDisabled();
});

test('a stale transition keeps the dialog open and requires a fresh readiness review', async ({
	page
}) => {
	const observed = await mock(page, false, true);
	await page.goto(url);
	await page.getByRole('button', { name: 'เริ่มขั้นตอนปิดภาคเรียน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'ยืนยันเริ่มขั้นตอนปิดภาคเรียน', exact: true }).click();
	await expect(
		dialog.getByText('ข้อมูลภาคเรียนหรือความพร้อมเปลี่ยนแล้ว กรุณาตรวจสอบใหม่')
	).toBeVisible();
	await expect(
		dialog.getByRole('button', { name: 'ยืนยันเริ่มขั้นตอนปิดภาคเรียน', exact: true })
	).toBeDisabled();
	await expect(dialog.getByRole('button', { name: 'โหลดความพร้อมล่าสุด' })).toBeVisible();
	expect(observed.writes).toHaveLength(1);
});

test('final closure requires an actual date and acknowledgement of the current warnings', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-09-30T05:00:00Z'));
	const initial = workspace();
	initial.context.termStatus = 'closing';
	initial.coverage.ready = true;
	initial.availableActions = ['close'];
	initial.findings = [
		{
			code: 'exams.warning',
			severity: 'warning',
			count: 1,
			message: 'มีรอบสอบร่างที่ยังไม่เผยแพร่',
			resolutionUrl: null
		}
	];
	const observed = await mock(page, false, false, initial);
	await page.goto(url);
	await page.getByRole('button', { name: 'ปิดภาคเรียน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันปิดภาคเรียน', exact: true });
	await expect(confirm).toBeDisabled();
	await dialog.getByRole('button', { name: 'วันที่ปิดภาคเรียนจริง', exact: true }).click();
	await page.locator('[data-value="2026-09-30"]').getByRole('button').click();
	await expect(confirm).toBeDisabled();
	await dialog.getByRole('checkbox').check();
	await expect(confirm).toBeEnabled();
	expect(observed.writes).toHaveLength(0);
	await confirm.click();
	await expect(dialog).toBeHidden();
	expect(observed.writes[0]).toMatchObject({
		action: 'close',
		closedOn: '2026-09-30',
		acknowledgedWarningCodes: ['exams.warning'],
		readinessChecksum: 'b'.repeat(64)
	});
});

test('reopening requires a reason and activation never happens just by viewing a ready term', async ({
	page
}) => {
	const initial = workspace();
	initial.context.termStatus = 'closed';
	initial.context.closedOn = '2026-09-30';
	initial.availableActions = ['reopen'];
	const observed = await mock(page, false, false, initial);
	await page.goto(url);
	await page.getByRole('button', { name: 'เปิดกลับเพื่อตรวจสอบ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันเปิดกลับเพื่อตรวจสอบ' });
	await expect(confirm).toBeDisabled();
	await dialog.getByLabel('เหตุผลเปิดกลับ').fill('ตรวจผลแก้ไขจากฝ่ายวิชาการ');
	await confirm.click();
	await expect(dialog).toBeHidden();
	expect(observed.writes[0]).toMatchObject({
		action: 'reopen',
		closedOn: null,
		reason: 'ตรวจผลแก้ไขจากฝ่ายวิชาการ'
	});
});

test('ready term activates only after explicit confirmation', async ({ page }) => {
	const initial = workspace();
	initial.context.yearStatus = 'planning';
	initial.context.termStatus = 'ready';
	initial.availableActions = ['activate'];
	const observed = await mock(page, false, false, initial, activationWorkspace(true));
	await page.goto(url);
	await page.getByRole('button', { name: 'เริ่มใช้ภาคเรียนนี้', exact: true }).click();
	expect(observed.writes).toHaveLength(0);
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('เปิดปีการศึกษาและภาคเรียนพร้อมกัน')).toBeVisible();
	await expect(dialog.getByText('นักเรียนที่วางแผนไว้ 1 คน')).toBeVisible();
	await expect.poll(observed.activationReads).toBe(1);
	await dialog.getByRole('button', { name: 'ยืนยันเริ่มใช้ภาคเรียนนี้' }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	expect(observed.writes[0]).toMatchObject({
		action: 'activate',
		readinessChecksum: 'd'.repeat(64),
		closedOn: null,
		reason: null
	});
});

test('reader can inspect opening gates without seeing an activation mutation control', async ({
	page
}) => {
	const initial = workspace(true);
	initial.context.yearStatus = 'planning';
	initial.context.termStatus = 'ready';
	const activation = activationWorkspace(true);
	activation.canActivate = false;
	activation.findings = [
		{
			code: 'opening.homeroom_placement_missing',
			severity: 'blocking',
			count: 1,
			message: 'มีนักเรียนที่ยังไม่มีห้องประจำชั้น',
			resolutionUrl: null
		}
	];
	const observed = await mock(page, true, false, initial, activation);
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจความพร้อมเปิดใช้', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('มีนักเรียนที่ยังไม่มีห้องประจำชั้น')).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'ยืนยันเริ่มใช้ภาคเรียนนี้' })).toHaveCount(0);
	expect(observed.writes).toHaveLength(0);
});

test('closed term prepares selected future modules only after preview and explicit apply', async ({
	page
}) => {
	const initial = workspace();
	initial.context.termStatus = 'closed';
	initial.context.closedOn = '2026-10-31';
	initial.availableActions = ['reopen'];
	const observed = await mock(page, false, false, initial);
	await page.goto(url);
	await page.getByRole('button', { name: 'เตรียมภาคเรียนถัดไป', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByRole('heading', { name: 'เตรียมข้อมูลสำหรับภาคเรียนถัดไป' })
	).toBeVisible();
	expect(observed.preparationWrites).toHaveLength(0);
	await dialog.getByRole('button', { name: 'ตรวจตัวอย่าง', exact: true }).click();
	await expect(dialog.getByText('ปีการศึกษา 2570 · ภาคเรียนที่ 1')).toBeVisible();
	await expect.poll(() => observed.preparationPreviews.length).toBe(1);
	expect(observed.preparationWrites).toHaveLength(0);
	await dialog.getByRole('button', { name: 'สร้างแบบร่างที่เลือก', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(observed.preparationWrites).toHaveLength(1);
	expect(observed.preparationWrites[0]).toMatchObject({
		sourceTermId: term,
		targetTermId: nextTerm,
		modules: ['delivery'],
		sourceChecksum: 'e'.repeat(64),
		mappings: { entities: [], dates: [] }
	});
	expect(observed.preparationWrites[0].requestId).toMatch(/^[0-9a-f-]{36}$/);
});
