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

function workspace(
	canManage: boolean,
	phaseCode: (typeof phaseCodes)[number],
	scoreValue: string | null = null
) {
	const index = phaseCodes.indexOf(phaseCode);
	return {
		learningGroupId: ids.group,
		learningOfferingId: ids.offering,
		assessmentPhaseId: subject().phases[index]!.id,
		phaseCode,
		phaseMaxScore: subject().phases[index]!.maxScore,
		phaseRowVersion: 1,
		scoreEntryEnabled: true,
		locked: false,
		canManage,
		canConfirm: canManage,
		items: [
			{
				id: index === 0 ? ids.item : `80000000-0000-4000-8000-00000000000${index + 1}`,
				name: 'ชีท 1',
				maxScore: subject().phases[index]!.maxScore,
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
		scores:
			scoreValue === null
				? []
				: [
						{
							scoreItemId:
								index === 0 ? ids.item : `80000000-0000-4000-8000-00000000000${index + 1}`,
							studentAcademicYearId: ids.student,
							value: scoreValue,
							rowVersion: 1
						}
					],
		sourceChecksum: 'source-1',
		rosterChecksum: 'roster-1',
		confirmation: null,
		confirmationIsCurrent: false
	};
}

async function mockGradebook(
	page: Page,
	canManage: boolean,
	closedPhase?: string,
	itemMaximum?: string,
	initialScores?: Array<string | null>,
	closedTerm = false
) {
	const controlRequests: string[] = [];
	const scoreBodies: unknown[] = [];
	const scorePaths: string[] = [];
	const workspacePaths: string[] = [];
	const itemPaths: string[] = [];
	const workspaces = phaseCodes.map((phase, index) =>
		workspace(canManage && !closedTerm && phase !== closedPhase, phase, initialScores?.[index])
	);
	if (itemMaximum) for (const row of workspaces) row.items[0]!.maxScore = itemMaximum;
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
				const context = contextOptions();
				await fulfill(
					route,
					closedTerm
						? {
								...context,
								activeAcademicTermId: null,
								terms: context.terms.map((term) => ({ ...term, status: 'closed' }))
							}
						: context
				);
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
				const phaseCode = phaseCodes.find((phase) => url.pathname.includes(`/phases/${phase}`))!;
				const current = workspaces.find((row) => row.phaseCode === phaseCode)!;
				if (request.method() === 'POST' && url.pathname.endsWith('/items')) {
					itemPaths.push(url.pathname);
					const input = request.postDataJSON() as {
						name: string;
						maxScore: string;
						displayOrder: number;
					};
					const item = {
						...input,
						id: '80000000-0000-4000-8000-000000000099',
						lifecycle: 'active',
						rowVersion: 1
					};
					current.items.push(item);
					await fulfill(route, item);
					return;
				}
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
					scorePaths.push(url.pathname);
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
				workspacePaths.push(url.pathname);
				await fulfill(route, current);
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
	return { controlRequests, scoreBodies, scorePaths, workspacePaths, itemPaths };
}

function gradebookUrl() {
	return `/staff/academic/gradebook?academicYearId=${ids.year}&academicTermId=${ids.term}`;
}

test('loaded scores trim fractional zeros without changing decimals, zero or blanks', async ({
	page
}) => {
	const observed = await mockGradebook(page, true, undefined, undefined, [
		'5.00',
		'5.50',
		'5.25',
		null
	]);
	await page.goto(gradebookUrl());
	const cell = (phase: string) =>
		page.getByRole('textbox', { name: `${phase} ชีท 1 เด็กชายทดสอบ ระบบ`, exact: true });
	await expect(cell('ก่อนกลางภาค')).toHaveValue('5');
	await expect(cell('กลางภาค')).toHaveValue('5.5');
	await expect(cell('หลังกลางภาค')).toHaveValue('5.25');
	await expect(cell('ปลายภาค')).toHaveValue('');
	await page.reload();
	await expect(cell('ก่อนกลางภาค')).toHaveValue('5');
	await page.getByRole('button', { name: 'เลือกทุกช่อง' }).click();
	await cell('ก่อนกลางภาค').press('Tab');
	await cell('ปลายภาค').fill('0');
	await cell('ปลายภาค').press('Tab');
	await expect.poll(() => observed.scoreBodies.length).toBe(1);
	expect(observed.scoreBodies[0]).toMatchObject({
		cells: [{ scoreItemId: '80000000-0000-4000-8000-000000000004', value: '0' }]
	});
	await page.setViewportSize({ width: 390, height: 844 });
	await page
		.getByRole('button', { name: 'เปิดกรอก ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true })
		.click();
	await expect(page.getByRole('dialog').getByRole('textbox')).toHaveValue('5');
});

for (const [key, targetPhase] of [
	['Enter', 'กลางภาค'],
	['Shift+Enter', 'ปลายภาค']
]) {
	test(`${key} selects the destination score so typing replaces it`, async ({ page }) => {
		await mockGradebook(page, true, undefined, undefined, ['5.00', '12.00', '15.00', '18.00']);
		await page.goto(gradebookUrl());
		await page.getByRole('button', { name: 'เลือกทุกช่อง' }).click();
		const before = page.getByRole('textbox', {
			name: 'ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ',
			exact: true
		});
		const target = page.getByRole('textbox', {
			name: `${targetPhase} ชีท 1 เด็กชายทดสอบ ระบบ`,
			exact: true
		});
		await before.press(key);
		await expect(target).toBeFocused();
		await page.keyboard.type('7');
		await expect(target).toHaveValue('7');
	});
}

for (const key of ['Tab', 'Enter']) {
	test(`${key} moves focus while saving is pending without losing subsequent scores`, async ({
		page
	}) => {
		const observed = await mockGradebook(page, true);
		let releaseSave!: () => void;
		const saveGate = new Promise<void>((resolve) => {
			releaseSave = resolve;
		});
		let savingStarted = false;
		await page.route(
			(url) => url.pathname.endsWith('/scores'),
			async (route) => {
				savingStarted = true;
				await saveGate;
				await route.fallback();
			}
		);
		await page.goto(gradebookUrl());
		await page.getByRole('button', { name: 'เลือกทุกช่อง' }).click();
		const cell = (phase: string) =>
			page.getByRole('textbox', {
				name: `${phase} ชีท 1 เด็กชายทดสอบ ระบบ`,
				exact: true
			});
		try {
			await cell('ก่อนกลางภาค').fill('5');
			await cell('ก่อนกลางภาค').press(key);
			await expect.poll(() => savingStarted).toBe(true);
			await expect(cell('กลางภาค')).toBeFocused();
			await cell('กลางภาค').fill('7');
			await cell('กลางภาค').press(key);
			await expect(cell('หลังกลางภาค')).toBeFocused();
			await cell('หลังกลางภาค').press(`Shift+${key}`);
			await expect(cell('กลางภาค')).toBeFocused();
		} finally {
			releaseSave();
		}
		await expect.poll(() => observed.scoreBodies.length).toBe(2);
		expect(observed.scoreBodies[0]).toMatchObject({
			cells: [{ scoreItemId: ids.item, value: '5' }]
		});
		expect(observed.scoreBodies[1]).toMatchObject({
			cells: [{ scoreItemId: '80000000-0000-4000-8000-000000000002', value: '7' }]
		});
		await expect(cell('ก่อนกลางภาค')).toHaveValue('5');
		await expect(cell('กลางภาค')).toHaveValue('7');
		await expect(cell('กลางภาค')).toBeFocused();
	});
}

test('teacher selects a score column and autosaves an explicit zero', async ({ page }) => {
	const observed = await mockGradebook(page, true);
	await page.goto(gradebookUrl());

	await expect(page.getByRole('heading', { name: 'คะแนนทั้งภาคเรียน' })).toBeVisible();
	await page.getByRole('checkbox').first().click();
	await page
		.getByRole('textbox', { name: 'ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true })
		.fill('0');
	await page
		.getByRole('textbox', { name: 'ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true })
		.blur();
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
	await page
		.getByRole('button', { name: 'เปิดกรอก ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true })
		.click();
	await expect(page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' })).toBeVisible();
	await page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' }).click();
	await expect(page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' })).toBeHidden();
});

test('read-only teacher never requests manager controls', async ({ page }) => {
	const observed = await mockGradebook(page, false);
	await page.goto(gradebookUrl());

	await expect(page.getByRole('heading', { name: 'คะแนนทั้งภาคเรียน' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ตั้งค่าการกรอก', exact: true })).toHaveCount(0);
	expect(observed.controlRequests).toEqual([]);
});

test('closed term keeps scores readable without a school-manager editing bypass', async ({
	page
}) => {
	const observed = await mockGradebook(
		page,
		true,
		undefined,
		undefined,
		['0', '5.5', null, '10'],
		true
	);
	await page.goto(gradebookUrl());
	await expect(page.getByRole('heading', { name: 'คะแนนทั้งภาคเรียน' })).toBeVisible();
	await expect.poll(() => observed.workspacePaths.length).toBe(4);
	for (const [phase, value] of [
		['ก่อนกลางภาค', '0'],
		['กลางภาค', '5.5'],
		['หลังกลางภาค', ''],
		['ปลายภาค', '10']
	]) {
		const cell = page.getByRole('textbox', {
			name: `${phase} ชีท 1 เด็กชายทดสอบ ระบบ`,
			exact: true
		});
		await expect(cell).toBeDisabled();
		await expect(cell).toHaveValue(value!);
	}
	await expect(page.getByRole('button', { name: /^แก้ .*ชีท 1$/ })).toHaveCount(0);
	expect(observed.scoreBodies).toEqual([]);
	expect(observed.itemPaths).toEqual([]);
});

test('one table saves different phases to their own endpoint and clearing stays blank', async ({
	page
}) => {
	const observed = await mockGradebook(page, true);
	await page.goto(gradebookUrl());
	await page.getByRole('button', { name: 'เลือกทุกช่อง' }).click();
	await expect(page.locator('table')).toHaveCount(1);
	await expect(page.locator('th[scope="colgroup"]')).toHaveCount(4);
	const before = page.getByRole('textbox', {
		name: 'ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ',
		exact: true
	});
	const final = page.getByRole('textbox', { name: 'ปลายภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true });
	await before.fill('12');
	await final.fill('23');
	await final.blur();
	await expect.poll(() => observed.scorePaths.length).toBe(2);
	expect(observed.scorePaths).toEqual([
		`/api/academic/gradebook/groups/${ids.group}/phases/before_midterm/scores`,
		`/api/academic/gradebook/groups/${ids.group}/phases/final/scores`
	]);
	await expect(page.locator('tbody tr').first().locator('td').last()).toHaveText('35');
	await final.fill('');
	await final.blur();
	await expect.poll(() => observed.scoreBodies.length).toBe(3);
	expect(observed.scoreBodies[2]).toMatchObject({
		cells: [{ operation: 'clear', scoreItemId: '80000000-0000-4000-8000-000000000004' }]
	});
	await expect(final).toHaveValue('');
	expect(observed.workspacePaths).toHaveLength(4);
});

test('select all and keyboard movement skip a closed phase', async ({ page }) => {
	const observed = await mockGradebook(page, true, 'midterm');
	await page.goto(gradebookUrl());
	await page.getByRole('button', { name: 'เลือกทุกช่อง' }).click();
	await expect(
		page.getByRole('textbox', { name: 'กลางภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true })
	).toBeDisabled();
	const before = page.getByRole('textbox', {
		name: 'ก่อนกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ',
		exact: true
	});
	await before.fill('5');
	await before.press('Tab');
	await expect(
		page.getByRole('textbox', { name: 'หลังกลางภาค ชีท 1 เด็กชายทดสอบ ระบบ', exact: true })
	).toBeFocused();
	await expect.poll(() => observed.scoreBodies.length).toBe(1);
});

test('adding an item uses the chosen phase and refreshes only that phase', async ({ page }) => {
	const observed = await mockGradebook(page, true, undefined, '20');
	await page.goto(gradebookUrl());
	await page.getByRole('button', { name: 'เพิ่มรายการคะแนนปลายภาค', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อรายการ').fill('งานเพิ่มเติม');
	await dialog.getByLabel('คะแนนเต็ม', { exact: true }).fill('10.01');
	await dialog.getByRole('button', { name: 'เพิ่มรายการ', exact: true }).click();
	await expect(
		dialog.getByText('คะแนนเต็มของรายการนี้ต้องไม่เกิน 10 คะแนน', { exact: true })
	).toBeVisible();
	expect(observed.itemPaths).toHaveLength(0);
	await dialog.getByLabel('คะแนนเต็ม', { exact: true }).fill('10');
	await dialog.getByRole('button', { name: 'เพิ่มรายการ', exact: true }).click();
	await expect(
		page.getByRole('checkbox', { name: 'เลือก ปลายภาค งานเพิ่มเติม', exact: true })
	).toBeChecked();
	expect(observed.itemPaths).toEqual([
		`/api/academic/gradebook/groups/${ids.group}/phases/final/items`
	]);
	expect(observed.workspacePaths).toHaveLength(5);
	expect(observed.workspacePaths[4]).toContain('/phases/final');
	await expect(
		page.getByRole('button', { name: 'เพิ่มรายการคะแนนปลายภาค', exact: true })
	).toBeDisabled();
});

test('a full phase disables additions but still allows editing without increasing its allocation', async ({
	page
}) => {
	await mockGradebook(page, true);
	await page.goto(gradebookUrl());
	await expect(
		page.getByRole('button', { name: 'เพิ่มรายการคะแนนก่อนกลางภาค', exact: true })
	).toBeDisabled();
	await page.getByRole('button', { name: 'แก้ ก่อนกลางภาค ชีท 1', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('คะแนนเต็ม', { exact: true }).fill('20.01');
	await dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(
		dialog.getByText('คะแนนเต็มของรายการนี้ต้องไม่เกิน 20 คะแนน', { exact: true })
	).toBeVisible();
});

test('entry settings open and close in a dialog instead of taking ledger space', async ({
	page
}) => {
	await mockGradebook(page, true);
	await page.goto(gradebookUrl());
	await expect(page.getByRole('switch')).toHaveCount(0);
	await page.getByRole('button', { name: 'ตั้งค่าการกรอก', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('switch')).toHaveCount(4);
	await dialog.getByRole('button', { name: 'ปิด', exact: true }).click();
	await expect(dialog).toHaveCount(0);
});
