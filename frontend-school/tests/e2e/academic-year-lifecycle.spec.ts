import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';

test.use({ serviceWorkers: 'block' });
type Workspace = components['schemas']['YearLifecycleWorkspace'];
type Request = components['schemas']['YearTransitionRequest'];
const year = '10000000-0000-4000-8000-000000000001';
const term = '20000000-0000-4000-8000-000000000001';
const actor = '30000000-0000-4000-8000-000000000001';
const url = `/staff/academic/year-lifecycle?academicYearId=${year}`;

function workspace(ready = true): Workspace {
	return {
		context: {
			academicYearId: year,
			year: 2569,
			name: 'ปีการศึกษา 2569',
			startDate: '2026-05-01',
			endDate: '2027-04-30',
			status: 'closing',
			rowVersion: 4
		},
		terms: [
			{
				academicTermId: term,
				name: 'ภาคเรียนที่ 2',
				sequence: 2,
				status: 'closed',
				includedInYearResult: true,
				blocksYearClosure: true,
				rowVersion: 2
			}
		],
		coverage: {
			students: [
				{
					studentAcademicYearId: '50000000-0000-4000-8000-000000000001',
					revisionId: ready ? '60000000-0000-4000-8000-000000000001' : null,
					revision: ready ? 1 : null,
					isCurrent: ready,
					holdReason: null
				}
			],
			ready,
			sourceChecksum: 'a'.repeat(64)
		},
		findings: ready
			? [
					{
						code: 'year.optional_terms',
						severity: 'warning',
						count: 1,
						message: 'มีภาคฤดูร้อนที่ยังไม่เริ่มใช้งาน',
						resolutionUrl: null
					}
				]
			: [
					{
						code: 'year.annual_missing',
						severity: 'blocking',
						count: 1,
						message: 'ผลรายปียังไม่ยืนยัน',
						resolutionUrl: null
					}
				],
		availableActions: ['cancel_closing', 'close'],
		canClose: ready,
		sourceChecksum: 'b'.repeat(64)
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
	{
		reader = false,
		ready = true,
		conflict = false,
		closed = false,
		blocked = false,
		uncertain = false
	} = {}
) {
	let current = workspace(ready);
	if (closed)
		current = {
			...current,
			context: { ...current.context, status: 'closed' },
			availableActions: []
		};
	if (reader) current.availableActions = [];
	const writes: Request[] = [];
	const reopeningWrites: components['schemas']['YearReopeningRequest'][] = [];
	let recoveryReads = 0;
	let reads = 0;
	await page.route(
		(request) => request.pathname.startsWith('/api/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			if (path === '/api/auth/me')
				return fulfill(route, {
					id: actor,
					username: 'E2E-LIFECYCLE-year',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: reader
						? ['academic_lifecycle.read.school', 'academic_context.read.school']
						: ['*']
				} satisfies components['schemas']['CurrentUserResponse']);
			if (path === '/api/academic/context/options') {
				reads++;
				return fulfill(route, {
					activeAcademicYearId: current.context.status === 'closed' ? null : year,
					activeAcademicTermId: null,
					years: [
						{
							id: year,
							year: 2569,
							name: 'ปีการศึกษา 2569',
							status: current.context.status,
							startDate: '2026-05-01',
							endDate: '2027-04-30'
						}
					],
					terms: [
						{
							id: term,
							academicYearId: year,
							sequence: 2,
							code: '2',
							name: 'ภาคเรียนที่ 2',
							termType: 'regular',
							status: 'closed',
							startDate: '2026-11-01',
							plannedEndDate: null,
							closedOn: '2027-03-20',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				} satisfies components['schemas']['AcademicContextOptions']);
			}
			if (path === `/api/academic/lifecycle/years/${year}`) return fulfill(route, current);
			if (path === `/api/academic/lifecycle/years/${year}/reopening`) {
				if (route.request().method() === 'GET') {
					recoveryReads++;
					return fulfill(route, {
						context: current.context,
						canReopen: !blocked,
						sourceChecksum: 'd'.repeat(64),
						findings: blocked
							? [
									{
										code: 'year.reopen_promotion',
										severity: 'blocking',
										count: 1,
										message: 'มีรายการเลื่อนชั้นที่ดำเนินการแล้ว',
										resolutionUrl: null
									}
								]
							: []
					});
				}
				const request = route.request().postDataJSON();
				reopeningWrites.push(request);
				if (conflict && reopeningWrites.length === 1) {
					current = { ...current, context: { ...current.context, rowVersion: 6 } };
					return fulfill(route, 'เงื่อนไขการเปิดกลับเปลี่ยนแล้ว', 409);
				}
				if (uncertain && reopeningWrites.length === 1) return route.abort('failed');
				current = {
					...current,
					context: {
						...current.context,
						status: 'closing',
						rowVersion: current.context.rowVersion + 1
					},
					availableActions: []
				};
				return fulfill(route, {
					requestId: request.requestId,
					context: current.context,
					completedAt: '2026-09-11T10:00:00Z'
				});
			}
			if (path === `/api/academic/lifecycle/years/${year}/transitions`) {
				const request: Request = route.request().postDataJSON();
				writes.push(request);
				if (conflict)
					return fulfill(route, 'ข้อมูลปีการศึกษาหรือความพร้อมเปลี่ยนแล้ว กรุณาตรวจสอบใหม่', 409);
				current = {
					...current,
					context: {
						...current.context,
						status:
							request.action === 'close'
								? 'closed'
								: request.action === 'cancel_closing'
									? 'active'
									: 'closing',
						rowVersion: 5
					},
					availableActions: [],
					sourceChecksum: 'c'.repeat(64)
				};
				return fulfill(route, {
					requestId: request.requestId,
					action: request.action,
					context: current.context,
					completedAt: '2026-09-11T10:00:00Z'
				} satisfies components['schemas']['YearTransitionOutcome']);
			}
			if (path === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === '/api/school/public')
				return fulfill(route, { schoolName: 'โรงเรียนทดสอบ', logoFileId: null });
			if (path === '/api/menu/user') return fulfill(route, { groups: [] });
			if (path === '/api/notifications') return fulfill(route, { items: [], unread_count: 0 });
			if (path === '/api/me/work-items/counts')
				return fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			return fulfill(route, 'ไม่เปิดใช้ในชุดทดสอบนี้', 403);
		}
	);
	return { writes, reopeningWrites, recoveryReads: () => recoveryReads, contextReads: () => reads };
}

test('closed year reader lazily inspects recovery without reopen authority', async ({ page }) => {
	const observed = await mock(page, { reader: true, closed: true });
	await page.goto(url);
	const open = page.getByRole('button', { name: 'ตรวจการเปิดปีเก่ากลับ', exact: true });
	await expect(open).toBeVisible();
	expect(observed.recoveryReads()).toBe(0);
	await open.click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByText('ไม่เปลี่ยนคะแนน ผลที่ล็อก ห้องเรียน หรือช่วงเวลาที่เปิดให้กรอกคะแนน')
	).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'ยืนยันเปิดกลับเพื่อตรวจทาน' })).toHaveCount(0);
	expect(observed.recoveryReads()).toBe(1);
	expect(observed.reopeningWrites).toHaveLength(0);
});

test('year reopening requires reason and returns to closing without changing term', async ({
	page
}, testInfo) => {
	const observed = await mock(page, { closed: true });
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจการเปิดปีเก่ากลับ' }).click();
	const dialog = page.getByRole('dialog');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันเปิดกลับเพื่อตรวจทาน' });
	await expect(confirm).toBeDisabled();
	await dialog.getByLabel('เหตุผลที่เปิดปีเก่ากลับ').fill('ตรวจทานการตั้งค่าปี');
	await page.screenshot({
		path: testInfo.outputPath('year-reopening-desktop.png'),
		animations: 'disabled'
	});
	await confirm.click();
	await expect(dialog).toBeHidden();
	expect(observed.reopeningWrites).toHaveLength(1);
	expect(observed.reopeningWrites[0]).toMatchObject({
		expectedYearVersion: 4,
		sourceChecksum: 'd'.repeat(64),
		reason: 'ตรวจทานการตั้งค่าปี'
	});
	await expect(
		page.getByLabel('สถานะปีการศึกษา').getByText('กำลังปิด', { exact: true })
	).toBeVisible();
	await expect(page.getByRole('cell', { name: 'ปิดแล้ว', exact: true })).toBeVisible();
	await expect.poll(() => observed.contextReads()).toBeGreaterThan(1);
});

test('executed promotion blocks reopening for administrator', async ({ page }) => {
	const observed = await mock(page, { closed: true, blocked: true });
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจการเปิดปีเก่ากลับ' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('มีรายการเลื่อนชั้นที่ดำเนินการแล้ว')).toBeVisible();
	await dialog.getByLabel('เหตุผลที่เปิดปีเก่ากลับ').fill('ตรวจทาน');
	await expect(dialog.getByRole('button', { name: 'ยืนยันเปิดกลับเพื่อตรวจทาน' })).toBeDisabled();
	expect(observed.reopeningWrites).toHaveLength(0);
});

test('reopening conflict retains reason and requires an explicit fresh preview', async ({
	page
}) => {
	const observed = await mock(page, { closed: true, conflict: true });
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจการเปิดปีเก่ากลับ' }).click();
	const dialog = page.getByRole('dialog');
	const reason = dialog.getByLabel('เหตุผลที่เปิดปีเก่ากลับ');
	await reason.fill('ตรวจทานข้อมูลเดิม');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันเปิดกลับเพื่อตรวจทาน' });
	await confirm.click();
	await expect(confirm).toBeDisabled();
	await expect(reason).toHaveValue('ตรวจทานข้อมูลเดิม');
	await dialog.getByRole('button', { name: 'ตรวจเงื่อนไขล่าสุด' }).click();
	await expect(confirm).toBeEnabled();
	await confirm.click();
	await expect(dialog).toBeHidden();
	expect(observed.reopeningWrites).toHaveLength(2);
	expect(observed.reopeningWrites[1].expectedYearVersion).toBe(6);
	expect(observed.reopeningWrites[1].requestId).not.toBe(observed.reopeningWrites[0].requestId);
});

test('uncertain reopening retry keeps the same request identifier', async ({ page }) => {
	const observed = await mock(page, { closed: true, uncertain: true });
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจการเปิดปีเก่ากลับ' }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('เหตุผลที่เปิดปีเก่ากลับ').fill('ตรวจทานข้อมูลเดิม');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันเปิดกลับเพื่อตรวจทาน' });
	await confirm.click();
	await expect(dialog.getByRole('alert')).toBeVisible();
	await confirm.click();
	await expect(dialog).toBeHidden();
	expect(observed.reopeningWrites).toHaveLength(2);
	expect(observed.reopeningWrites[1]).toEqual(observed.reopeningWrites[0]);
});

test('mobile reopening retains visible dismissal controls', async ({ page }, testInfo) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mock(page, { closed: true });
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจการเปิดปีเก่ากลับ' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'Close', exact: true })).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'กลับไปตรวจสอบ', exact: true })).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath('year-reopening-mobile.png'),
		animations: 'disabled'
	});
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(dialog).toBeHidden();
});

test('year reader sees annual blockers without mutation controls', async ({ page }) => {
	const observed = await mock(page, { reader: true, ready: false });
	await page.goto(url);
	await expect(page.getByRole('heading', { name: 'ปิดปีการศึกษา', exact: true })).toBeVisible();
	await expect(page.getByText('ผลรายปียังไม่ยืนยัน')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true })).toHaveCount(0);
	expect(observed.writes).toHaveLength(0);
});

test('incomplete annual coverage disables close even for administrator', async ({ page }) => {
	await mock(page, { ready: false });
	await page.goto(url);
	await expect(page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true })).toBeDisabled();
});

test('year closure acknowledges warnings and refreshes authoritative context', async ({ page }) => {
	const observed = await mock(page);
	await page.goto(url);
	await page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true }).click();
	const dialog = page.getByRole('dialog');
	const confirm = dialog.getByRole('button', { name: 'ยืนยันปิดปีการศึกษา', exact: true });
	await expect(confirm).toBeDisabled();
	await dialog.getByRole('checkbox').check();
	await confirm.click();
	await expect(dialog).toBeHidden();
	expect(observed.writes).toHaveLength(1);
	expect(observed.writes[0]).toMatchObject({
		action: 'close',
		expectedYearVersion: 4,
		readinessChecksum: 'b'.repeat(64),
		acknowledgedWarningCodes: ['year.optional_terms']
	});
	expect(observed.writes[0].requestId).toMatch(/^[0-9a-f-]{36}$/);
	await expect.poll(() => observed.contextReads()).toBeGreaterThan(1);
});

test('stale year readiness cannot blindly retry', async ({ page }) => {
	const observed = await mock(page, { conflict: true });
	await page.goto(url);
	await page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('checkbox').check();
	const confirm = dialog.getByRole('button', { name: 'ยืนยันปิดปีการศึกษา', exact: true });
	await confirm.click();
	await expect(confirm).toBeDisabled();
	await expect(
		dialog.getByText('ข้อมูลปีการศึกษาหรือความพร้อมเปลี่ยนแล้ว กรุณาตรวจสอบใหม่')
	).toBeVisible();
	expect(observed.writes).toHaveLength(1);
});

test('mobile year confirmation has a visible close control', async ({ page }, testInfo) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mock(page);
	await page.goto(url);
	await page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'Close', exact: true })).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath('year-mobile-confirmation.png'),
		animations: 'disabled'
	});
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(dialog).toBeHidden();
});
