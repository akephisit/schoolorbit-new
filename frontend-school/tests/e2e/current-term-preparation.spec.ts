import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';
import type {
	PreviewTermPreparationInput,
	ApplyTermPreparationInput
} from '../../src/lib/api/academic-lifecycle';
import { mockYearManagement, yearUrl, id, fulfill, sourceYear } from './fixtures/year-management';
import { deferred, replaceDeliverySession } from './fixtures/current-delivery';
import { sessionState } from './fixtures/session-state';

type S = components['schemas'];
test.use({ serviceWorkers: 'block' });
const stamp = '2026-11-14T00:00:00Z';
const previewPath = '/api/academic/lifecycle/term-preparations/preview';
const applyPath = '/api/academic/lifecycle/term-preparations/apply';
const defaultTargetLabel = 'ปีการศึกษา 2570 · ภาคเรียนปลายทาง 5 · ลำดับ 1';
const trigger = (page: Page) =>
	page.getByRole('button', { name: 'เตรียมภาคเรียนถัดไป', exact: true });
const dialog = (page: Page) =>
	page.getByRole('dialog', { name: 'เตรียมข้อมูลสำหรับภาคเรียนถัดไป', exact: true });
const previewButton = (page: Page) =>
	dialog(page).getByRole('button', { name: 'ตรวจตัวอย่าง', exact: true });
const applyButton = (page: Page) =>
	dialog(page).getByRole('button', { name: 'สร้างรายการที่เตรียมที่เลือก', exact: true });
function year(
	number: number,
	value: number,
	status: S['AcademicYear']['status']
): S['AcademicYear'] {
	return {
		id: id(number),
		year: value,
		name: `ปีการศึกษา ${value}`,
		status,
		rowVersion: 1,
		isVisible: true,
		defaultSchoolDays: ['MON'],
		migrated: false,
		createdAt: stamp,
		updatedAt: stamp
	};
}
function term(
	number: number,
	yearId: string,
	status: S['AcademicTerm']['status'] = 'planning'
): S['AcademicTerm'] {
	return {
		id: id(number),
		academicYearId: yearId,
		bellScheduleId: id(80),
		sequence: 1,
		code: '1',
		name: `ภาคเรียนปลายทาง ${number}`,
		status,
		rowVersion: 1,
		isVisible: true,
		termType: 'regular',
		schoolDays: ['MON'],
		includedInYearResult: true,
		blocksYearClosure: true,
		migrated: false,
		createdAt: stamp,
		updatedAt: stamp
	};
}
function projection(
	input: PreviewTermPreparationInput,
	ready = true
): S['TermPreparationWorkspace'] {
	return {
		context: {
			sourceYearId: id(1),
			sourceTermId: input.sourceTermId,
			sourceLabel: 'ปี 2569 · ภาคเรียนที่ 1',
			sourceStatus: 'closed',
			targetYearId: id(2),
			targetTermId: input.targetTermId,
			targetLabel: 'ปี 2570 · ภาคเรียนปลายทาง',
			targetStatus: 'planning'
		},
		sourceChecksum: 'a'.repeat(64),
		canApply: ready,
		findings: [],
		modules: input.modules.map((module) => ({
			module,
			sourceCount: 1,
			proposedCount: 1,
			targetExistingCount: 0,
			status: 'ready',
			summary: 'ข้อมูลปัจจุบันที่เตรียมได้'
		})),
		mappingRequirements: input.modules.includes('timetable')
			? [
					{
						kind: 'teacher',
						sourceId: id(81),
						sourceLabel: 'ครูต้นทางที่บันทึกไว้',
						requiredBy: ['timetable'],
						selectedTargetId:
							input.mappings?.entities?.find(
								(item) => item.kind === 'teacher' && item.sourceId === id(81)
							)?.targetId ?? id(82),
						targetOptions: [
							{ id: id(82), label: 'ครูเป้าหมายเดิม' },
							{ id: id(83), label: 'ครูเป้าหมายที่เลือก' }
						]
					}
				]
			: [],
		dateRequirements: input.modules.includes('exams')
			? [
					{
						sourceDate: '2026-10-10',
						sourceLabel: 'วันสอบเดิม',
						requiredBy: ['exams'],
						selectedTargetDate:
							input.mappings?.dates?.find((item) => item.sourceDate === '2026-10-10')?.targetDate ??
							'2026-11-15'
					}
				]
			: []
	};
}
async function setup(page: Page, options: { permissions?: string[]; active?: boolean } = {}) {
	await mockYearManagement(page, { active: options.active });
	const reads: URL[] = [],
		previews: PreviewTermPreparationInput[] = [],
		applies: ApplyTermPreparationInput[] = [];
	page.on('request', (request) => {
		if (request.method() === 'GET' && new URL(request.url()).pathname.startsWith('/api/'))
			reads.push(new URL(request.url()));
	});
	let actor = id(99),
		session = sessionState();
	await page.route('**/api/auth/me', (route) =>
		fulfill(route, {
			id: actor,
			username: 'preparation-manager',
			firstName: 'วิชา',
			lastName: 'การ',
			displayName: 'วิชา การ',
			userType: 'staff',
			status: 'active',
			session,
			profileImageFileId: null,
			primaryRoleName: null,
			permissions: options.permissions ?? ['*']
		} satisfies S['CurrentUserResponse'])
	);
	const years = [
		year(90, 2568, 'planning'),
		year(1, 2569, options.active ? 'active' : 'closed'),
		year(2, 2570, 'planning'),
		year(91, 2571, 'closed'),
		year(92, 2572, 'archived')
	];
	const targetTerms = [
		term(5, id(2)),
		term(6, id(2), 'ready'),
		term(7, id(2), 'active'),
		term(93, id(90)),
		term(4, id(1)),
		term(94, id(91)),
		term(95, id(92))
	];
	await page.route('**/api/academic/context/options', (route) =>
		fulfill(route, {
			defaultAcademicYearId: id(1),
			defaultAcademicTermId: null,
			years: [
				{
					id: id(1),
					year: sourceYear.year,
					name: sourceYear.name,
					status: options.active ? 'active' : 'closed',
					isVisible: true
				}
			],
			terms: []
		} satisfies S['AcademicContextOptions'])
	);
	await page.route('**/api/academic/years', (route) => fulfill(route, years));
	await page.route('**/api/academic/terms?*', (route) =>
		fulfill(
			route,
			targetTerms.filter(
				(candidate) =>
					candidate.academicYearId ===
					new URL(route.request().url()).searchParams.get('academicYearId')
			)
		)
	);
	await page.route(`**${previewPath}`, (route) => {
		const body: PreviewTermPreparationInput = route.request().postDataJSON();
		previews.push(body);
		return fulfill(route, projection(body));
	});
	await page.route(`**${applyPath}`, (route) => {
		const body: ApplyTermPreparationInput = route.request().postDataJSON();
		applies.push(body);
		return fulfill(route, outcome(body));
	});
	return {
		reads,
		previews,
		applies,
		count: (path: string) => reads.filter((url) => url.pathname === path).length,
		setActor: (value: string) => {
			actor = value;
		},
		session: () => session,
		setSession: (value: typeof session) => {
			session = value;
		}
	};
}
function outcome(body: ApplyTermPreparationInput): S['TermPreparationOutcome'] {
	return {
		requestId: body.requestId,
		runId: id(85),
		sourceTermId: body.sourceTermId,
		targetTermId: body.targetTermId,
		createdAt: stamp,
		modules: body.modules.map((module) => ({ module, createdCount: 1, targetIds: [id(86)] }))
	};
}
async function openTarget(page: Page) {
	await trigger(page).click();
	await expect(dialog(page)).toBeVisible();
	const target = dialog(page).getByRole('combobox', {
		name: 'ภาคเรียนเป้าหมายที่กำลังวางแผน',
		exact: true
	});
	await expect(target).toBeEnabled();
	await expect(target).toContainText('เลือกภาคเรียน');
	await target.click();
	await page.getByRole('option', { name: defaultTargetLabel, exact: true }).click();
}
async function turn(page: Page) {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}

test('closed source preparation is reachable from the accepted year row and target lists remain lazy', async ({
	page
}) => {
	const api = await setup(page);
	await page.goto(yearUrl);
	await expect(trigger(page)).toBeEnabled();
	expect(api.count('/api/academic/years')).toBe(0);
	expect(api.count('/api/academic/terms')).toBe(0);
	expect(api.previews).toEqual([]);
	await trigger(page).click();
	await expect(dialog(page)).toContainText('ภาคเรียนที่ 1');
	const target = dialog(page).getByRole('combobox', {
		name: 'ภาคเรียนเป้าหมายที่กำลังวางแผน',
		exact: true
	});
	await expect(target).toBeEnabled();
	await expect(target).toContainText('เลือกภาคเรียน');
	await expect(previewButton(page)).toBeDisabled();
	await target.click();
	await expect(page.getByRole('option')).toHaveCount(1);
	await expect(page.getByRole('option')).toHaveText(defaultTargetLabel);
	expect(api.count('/api/academic/years')).toBe(1);
});

test('explicit mappings and date choices are re-previewed before one direct preparation receipt and one source reload', async ({
	page
}) => {
	const api = await setup(page);
	await page.goto(yearUrl);
	await openTarget(page);
	await dialog(page)
		.getByRole('checkbox', { name: /^ตารางสอน/ })
		.check();
	await dialog(page)
		.getByRole('checkbox', { name: /^ตารางสอบ/ })
		.check();
	await previewButton(page).click();
	await expect(dialog(page)).toContainText('1 รายการที่เตรียม');
	await dialog(page).getByRole('combobox').filter({ hasText: 'ครูเป้าหมายเดิม' }).click();
	await page.getByRole('option', { name: 'ครูเป้าหมายที่เลือก', exact: true }).click();
	await dialog(page)
		.getByRole('button', { name: 'วันที่ปลายทางของ วันสอบเดิม', exact: true })
		.click();
	await page
		.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: /^16$/ })
		.click();
	await expect(applyButton(page)).toBeDisabled();
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	expect(api.previews).toHaveLength(2);
	expect(api.previews[1]).toEqual({
		sourceTermId: id(3),
		targetTermId: id(5),
		modules: ['delivery', 'timetable', 'exams'],
		mappings: {
			entities: [{ kind: 'teacher', sourceId: id(81), targetId: id(83) }],
			dates: [{ sourceDate: '2026-10-10', targetDate: '2026-11-16' }]
		}
	});
	await applyButton(page).click();
	await expect(dialog(page)).toHaveCount(0);
	await expect.poll(() => api.count(`/api/academic/lifecycle/years/${id(1)}`)).toBe(2);
	expect(api.applies).toHaveLength(1);
	expect(api.applies[0]).toMatchObject({ ...api.previews[1], sourceChecksum: 'a'.repeat(64) });
	expect(api.count('/api/academic/context/options')).toBe(1);
	expect(api.reads.filter((url) => /result|aggregate|gpa/.test(url.pathname))).toEqual([]);
});

for (const permissions of [
	['academic_lifecycle.read.school', 'academic_context.read.school'],
	[
		'academic_lifecycle.read.school',
		'academic_lifecycle.manage.school',
		'academic_context.read.school'
	]
])
	test(`preparation cannot start without its exact manage and target-list read permissions: ${permissions.length}`, async ({
		page
	}) => {
		const api = await setup(page, { permissions });
		await page.goto(yearUrl);
		await expect(
			page.getByRole('heading', { name: 'จัดการปีการศึกษา', exact: true })
		).toBeVisible();
		await expect(trigger(page).and(page.locator(':enabled'))).toHaveCount(0);
		expect(api.count('/api/academic/years')).toBe(0);
		expect(api.count('/api/academic/terms')).toBe(0);
		expect(api.previews).toEqual([]);
		expect(api.applies).toEqual([]);
	});

test('an active source row never offers next-term preparation', async ({ page }) => {
	const api = await setup(page, { active: true });
	await page.goto(yearUrl);
	await expect(page.getByRole('button', { name: 'ปิดภาคเรียน', exact: true })).toBeVisible();
	await expect(trigger(page)).toHaveCount(0);
	expect(api.previews).toEqual([]);
});

test('closing and reopening fences a held preview from the previous dialog session', async ({
	page
}) => {
	const api = await setup(page),
		held = deferred();
	let first = true;
	let heldRequest: PreviewTermPreparationInput | null = null;
	await page.route(`**${previewPath}`, async (route) => {
		if (!first) return route.fallback();
		first = false;
		const body: PreviewTermPreparationInput = route.request().postDataJSON();
		heldRequest = body;
		await held.promise;
		return fulfill(route, { ...projection(body), sourceChecksum: 'b'.repeat(64) });
	});
	try {
		await page.goto(yearUrl);
		await openTarget(page);
		await previewButton(page).click();
		await expect
			.poll(() => heldRequest)
			.toMatchObject({ sourceTermId: id(3), targetTermId: id(5) });
		await expect(dialog(page)).toBeVisible();
		await expect(
			dialog(page).getByRole('button', { name: 'กำลังดำเนินการ...', exact: true })
		).toBeDisabled();
		await dialog(page).getByRole('button', { name: 'ปิด', exact: true }).click();
		await openTarget(page);
		await previewButton(page).click();
		await expect(applyButton(page)).toBeEnabled();
		held.release();
		await turn(page);
		await expect(applyButton(page)).toBeEnabled();
		await applyButton(page).click();
		await expect.poll(() => api.applies.length).toBe(1);
		expect(api.applies[0].sourceChecksum).toBe('a'.repeat(64));
	} finally {
		held.release();
	}
});

test('a stale preparation refuses apply until an explicit new preview', async ({ page }) => {
	const api = await setup(page);
	let first = true;
	await page.route(`**${applyPath}`, (route) => {
		if (!first) return route.fallback();
		first = false;
		return fulfill(route, 'ข้อมูลต้นทางเปลี่ยนแล้ว กรุณาตรวจตัวอย่างอีกครั้ง', 409);
	});
	await page.goto(yearUrl);
	await openTarget(page);
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	await applyButton(page).click();
	await expect(dialog(page).getByRole('alert')).toContainText('ข้อมูลต้นทางเปลี่ยนแล้ว');
	await expect(applyButton(page)).toBeDisabled();
	expect(api.applies).toEqual([]);
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	await applyButton(page).click();
	await expect(dialog(page)).toHaveCount(0);
	expect(api.applies).toHaveLength(1);
});

test('an unknown apply outcome retains the exact request identity for an explicit same-body retry', async ({
	page
}) => {
	await setup(page);
	const accepted: ApplyTermPreparationInput[] = [];
	await page.route(`**${applyPath}`, (route) => {
		const body: ApplyTermPreparationInput = route.request().postDataJSON();
		accepted.push(body);
		return accepted.length === 1 ? route.abort('failed') : fulfill(route, outcome(body));
	});
	await page.goto(yearUrl);
	await openTarget(page);
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	await applyButton(page).click();
	await expect(dialog(page).getByRole('alert')).toBeVisible();
	expect(accepted).toHaveLength(1);
	await expect(applyButton(page)).toBeEnabled();
	await applyButton(page).click();
	await expect(dialog(page)).toHaveCount(0);
	expect(accepted).toHaveLength(2);
	expect(accepted[1]).toEqual(accepted[0]);
});

for (const phase of ['preview', 'apply'] as const)
	test(`a held ${phase} from A cannot apply state or reload the source after same-route identity B`, async ({
		page
	}) => {
		const api = await setup(page),
			held = deferred();
		let request: Route | null = null;
		await page.route(`**${phase === 'preview' ? previewPath : applyPath}`, async (route) => {
			request = route;
			await held.promise;
			if (phase === 'preview') {
				const body: PreviewTermPreparationInput = route.request().postDataJSON();
				return fulfill(route, projection(body));
			}
			const body: ApplyTermPreparationInput = route.request().postDataJSON();
			return fulfill(route, outcome(body));
		});
		try {
			await page.goto(yearUrl);
			await openTarget(page);
			if (phase === 'apply') {
				await previewButton(page).click();
				await expect(applyButton(page)).toBeEnabled();
				await applyButton(page).click();
			} else await previewButton(page).click();
			await expect.poll(() => request !== null).toBe(true);
			api.setActor(id(98));
			await replaceDeliverySession(page, api);
			await expect(dialog(page)).toHaveCount(0);
			await expect.poll(() => api.count(`/api/academic/lifecycle/years/${id(1)}`)).toBe(2);
			await expect(trigger(page)).toBeEnabled();
			const baseline = api.count(`/api/academic/lifecycle/years/${id(1)}`);
			held.release();
			await turn(page);
			await expect(dialog(page)).toHaveCount(0);
			expect(api.count(`/api/academic/lifecycle/years/${id(1)}`)).toBe(baseline);
			await expect(trigger(page)).toBeEnabled();
		} finally {
			held.release();
		}
	});

for (const field of ['sourceTermId', 'targetTermId'] as const)
	test(`foreign preview ${field} is refused without an apply or implicit context change`, async ({
		page
	}) => {
		const api = await setup(page);
		await page.route(`**${previewPath}`, (route) => {
			const body: PreviewTermPreparationInput = route.request().postDataJSON();
			const value = projection(body);
			value.context[field] = id(6);
			return fulfill(route, value);
		});
		await page.goto(yearUrl);
		await openTarget(page);
		await previewButton(page).click();
		await expect(dialog(page).getByRole('alert')).toBeVisible();
		await expect(applyButton(page)).toBeDisabled();
		expect(api.applies).toEqual([]);
		expect(api.count('/api/academic/context/options')).toBe(1);
		await expect(page).toHaveURL(yearUrl);
	});

test('mapping changes invalidate a held preview instead of accepting its stale checksum', async ({
	page
}) => {
	const api = await setup(page),
		held = deferred();
	let calls = 0;
	await page.route(`**${previewPath}`, async (route) => {
		const body: PreviewTermPreparationInput = route.request().postDataJSON();
		if (++calls !== 2) return route.fallback();
		await held.promise;
		return fulfill(route, { ...projection(body), sourceChecksum: 'b'.repeat(64) });
	});
	try {
		await page.goto(yearUrl);
		await openTarget(page);
		await dialog(page)
			.getByRole('checkbox', { name: /^ตารางสอน/ })
			.check();
		await previewButton(page).click();
		await expect(applyButton(page)).toBeEnabled();
		await previewButton(page).click();
		await expect.poll(() => calls).toBe(2);
		await dialog(page).getByRole('combobox').filter({ hasText: 'ครูเป้าหมายเดิม' }).click();
		await page.getByRole('option', { name: 'ครูเป้าหมายที่เลือก', exact: true }).click();
		held.release();
		await turn(page);
		await expect(applyButton(page)).toBeDisabled();
		await previewButton(page).click();
		await expect(applyButton(page)).toBeEnabled();
		await applyButton(page).click();
		await expect(dialog(page)).toHaveCount(0);
		expect(api.applies[0].sourceChecksum).toBe('a'.repeat(64));
		expect(api.applies[0].mappings?.entities).toEqual([
			{ kind: 'teacher', sourceId: id(81), targetId: id(83) }
		]);
	} finally {
		held.release();
	}
});

test('module changes discard a previous ready preparation until their exact selection is reviewed', async ({
	page
}) => {
	const api = await setup(page);
	await page.goto(yearUrl);
	await openTarget(page);
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	await dialog(page)
		.getByRole('checkbox', { name: /^ตารางสอน/ })
		.check();
	await expect(applyButton(page)).toBeDisabled();
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	await applyButton(page).click();
	await expect(dialog(page)).toHaveCount(0);
	expect(api.applies[0].modules).toEqual(['delivery', 'timetable']);
	expect(api.previews).toHaveLength(2);
});

test('a target list belonging to another year is explicit and cannot enable preparation', async ({
	page
}) => {
	const api = await setup(page);
	await page.route('**/api/academic/terms?*', (route) => fulfill(route, [term(5, id(1))]));
	await page.goto(yearUrl);
	await trigger(page).click();
	await expect(dialog(page).getByRole('alert')).toContainText('ข้อมูลภาคเรียนไม่ตรงกับปีที่ขอ');
	await expect(previewButton(page)).toBeDisabled();
	expect(api.previews).toEqual([]);
	expect(api.applies).toEqual([]);
	expect(api.count('/api/academic/context/options')).toBe(1);
});

test('a held target-list read belongs to its dialog and identity rather than the same source IDs', async ({
	page
}) => {
	const api = await setup(page),
		held = deferred();
	let reads = 0;
	await page.route('**/api/academic/terms?*', async (route) => {
		const target = term(5, id(2));
		if (++reads === 1) {
			target.name = 'ภาคเรียนจากคำขอเก่าที่ห้ามใช้';
			await held.promise;
		}
		return fulfill(route, [target]);
	});
	try {
		await page.goto(yearUrl);
		await trigger(page).click();
		await expect.poll(() => reads).toBe(1);
		api.setActor(id(98));
		await replaceDeliverySession(page, api);
		await expect(dialog(page)).toHaveCount(0);
		await expect.poll(() => api.count(`/api/academic/lifecycle/years/${id(1)}`)).toBe(2);
		await trigger(page).click();
		const target = dialog(page).getByRole('combobox', {
			name: 'ภาคเรียนเป้าหมายที่กำลังวางแผน',
			exact: true
		});
		await expect(target).toBeEnabled();
		held.release();
		await turn(page);
		await expect(target).toContainText('เลือกภาคเรียน');
		await target.click();
		await expect(page.getByRole('option')).toHaveCount(1);
		await expect(page.getByRole('option')).toHaveText(defaultTargetLabel);
		await expect(page.getByText('ภาคเรียนจากคำขอเก่าที่ห้ามใช้', { exact: true })).toHaveCount(0);
		expect(reads).toBe(2);
		expect(api.previews).toEqual([]);
	} finally {
		held.release();
	}
});

test('identical future term and custom year names remain distinct and preview the explicitly chosen year', async ({
	page
}) => {
	const api = await setup(page);
	const first = { ...year(2, 2570, 'planning'), name: 'ปีเตรียมเปิด' };
	const second = { ...year(96, 2571, 'planning'), name: 'ปีเตรียมเปิด' };
	const requests: PreviewTermPreparationInput[] = [];
	await page.route('**/api/academic/years', (route) => fulfill(route, [first, second]));
	await page.route('**/api/academic/terms?*', (route) => {
		const ownerYear = new URL(route.request().url()).searchParams.get('academicYearId');
		const target = ownerYear === first.id ? term(5, first.id) : term(97, second.id);
		target.name = 'ภาคเรียนที่ 1';
		return fulfill(route, [target]);
	});
	await page.route(`**${previewPath}`, (route) => {
		const body: PreviewTermPreparationInput = route.request().postDataJSON();
		requests.push(body);
		const response = projection(body);
		response.context.targetYearId = second.id;
		response.context.targetLabel = 'ปี 2571 · ภาคเรียนที่ 1';
		return fulfill(route, response);
	});
	await page.goto(yearUrl);
	await trigger(page).click();
	const target = dialog(page).getByRole('combobox', {
		name: 'ภาคเรียนเป้าหมายที่กำลังวางแผน',
		exact: true
	});
	await expect(target).toBeEnabled();
	await expect(target).toHaveText('เลือกภาคเรียน');
	await target.click();
	const firstLabel = 'ปีเตรียมเปิด (ปีการศึกษา 2570) · ภาคเรียนที่ 1 · ลำดับ 1';
	const secondLabel = 'ปีเตรียมเปิด (ปีการศึกษา 2571) · ภาคเรียนที่ 1 · ลำดับ 1';
	await expect(page.getByRole('option', { name: firstLabel, exact: true })).toBeVisible();
	await page.getByRole('option', { name: secondLabel, exact: true }).click();
	await expect(target).toHaveText(secondLabel);
	await previewButton(page).click();
	await expect(applyButton(page)).toBeEnabled();
	await expect(dialog(page)).toContainText('ปี 2571 · ภาคเรียนที่ 1');
	expect(requests).toEqual([
		{
			sourceTermId: id(3),
			targetTermId: id(97),
			modules: ['delivery'],
			mappings: { entities: [], dates: [] }
		}
	]);
	expect(api.count('/api/academic/years')).toBe(1);
	expect(api.count('/api/academic/terms')).toBe(2);
	expect(api.applies).toEqual([]);
});
