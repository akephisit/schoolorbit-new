import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

const id = (n: number) => `44000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const year = id(1);
const nextYear = id(2);
const firstCycle = id(3);
const secondCycle = id(4);
const templateId = id(5);
const path = (section: string, academicYearId = year, cycleId = '') =>
	`/staff/academic/supervision/${section}?academicYearId=${academicYearId}${cycleId ? `&cycleId=${cycleId}` : ''}`;

function gate() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => (release = resolve));
	return { promise, release };
}

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(
	page: Page,
	options: {
		permissions?: string[];
		hold?: 'cycles' | 'teacher-status' | 'detail' | 'patch';
		fail?: 'cycles' | 'teacher-status' | 'detail';
		failAt?: number;
	} = {}
) {
	const reads: string[] = [];
	const writes: string[] = [];
	const held = gate();
	const counts = new Map<string, number>();
	let cycleStatus = 'open';
	let templateTitle = 'แบบประเมินทดสอบ';
	const detail = () => ({
		id: templateId,
		title: templateTitle,
		description: 'รายละเอียดแบบประเมิน',
		status: 'active',
		ratingMin: 1,
		ratingMax: 5,
		createdAt: '2026-09-01T00:00:00Z',
		updatedAt: '2026-09-01T00:00:00Z',
		sections: [
			{
				id: id(6),
				templateId,
				title: 'หมวดกิจกรรม',
				sortOrder: 1,
				items: [
					{
						id: id(7),
						sectionId: id(6),
						label: 'หัวข้อ rubric ที่โหลดเมื่อเปิด',
						itemType: 'rating',
						required: true,
						sortOrder: 1
					}
				]
			}
		],
		steps: []
	});
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const resource = url.pathname;
			const method = route.request().method();
			if (method === 'GET') {
				reads.push(resource);
				counts.set(resource, (counts.get(resource) ?? 0) + 1);
			} else writes.push(`${method} ${resource}`);
			if (resource === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-supervision',
					firstName: 'นิเทศ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: options.permissions ?? [
						'supervision.manage.school',
						'supervision.read.school'
					]
				});
			if (resource === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: year,
					activeAcademicTermId: null,
					terms: [],
					years: [year, nextYear].map((value, index) => ({
						id: value,
						year: 2569 + index,
						name: String(2569 + index),
						status: 'open',
						startDate: '2026-05-01',
						plannedEndDate: '2027-04-30',
						closedOn: null
					}))
				});
			if (resource === '/api/menu/user') return reply(route, { groups: [] });
			const cycles = () =>
				[firstCycle, secondCycle].map((value, index) => ({
					id: value,
					academicYearId: url.searchParams.get('academicYearId') ?? year,
					academicTermId: null,
					templateId,
					title: index
						? 'รอบที่สอง'
						: url.searchParams.get('academicYearId') === nextYear
							? 'รอบปีใหม่'
							: 'รอบแรก',
					startsAt: '2026-09-01T00:00:00Z',
					endsAt: '2026-12-01T00:00:00Z',
					status: cycleStatus,
					createdAt: '2026-09-01T00:00:00Z',
					updatedAt: '2026-09-01T00:00:00Z',
					targets: []
				}));
			const kind =
				resource === '/api/supervision/cycles'
					? 'cycles'
					: resource.endsWith('/teacher-status')
						? 'teacher-status'
						: resource === `/api/supervision/templates/${templateId}`
							? 'detail'
							: '';
			if (method === 'GET' && kind) {
				if (
					options.hold === kind &&
					(kind !== 'cycles' || url.searchParams.get('academicYearId') === year) &&
					(kind !== 'teacher-status' || resource.includes(secondCycle))
				)
					await held.promise;
				if (options.fail === kind && counts.get(resource) === (options.failAt ?? 1))
					return reply(route, 'region ไม่พร้อม', 503);
			}
			if (resource === '/api/supervision/cycles' && method === 'GET')
				return reply(route, { items: cycles() });
			if (resource === '/api/supervision/templates/summaries')
				return reply(route, {
					items: [
						{
							id: templateId,
							title: templateTitle,
							status: 'active',
							ratingMin: 1,
							ratingMax: 5,
							sectionCount: 1,
							itemCount: 1
						}
					]
				});
			if (resource === `/api/supervision/templates/${templateId}` && method === 'GET')
				return reply(route, detail());
			if (resource.endsWith('/teacher-status'))
				return reply(route, {
					items: [
						{
							teacherId: id(21),
							teacherDisplayName: resource.includes(secondCycle) ? 'ครู รอบสอง' : 'ครู รอบแรก',
							organizationUnitNames: [],
							evaluatorNames: [],
							nextStepLabel: 'จองคาบนิเทศ',
							averageRating: null
						}
					]
				});
			if (resource === `/api/supervision/cycles/${firstCycle}` && method === 'PATCH') {
				if (options.hold === 'patch') await held.promise;
				cycleStatus = route.request().postDataJSON().status;
				return reply(route, { ...cycles()[0], title: 'รอบแรก', status: cycleStatus });
			}
			if (resource === `/api/supervision/templates/${templateId}` && method === 'PATCH') {
				templateTitle = route.request().postDataJSON().title;
				return reply(route, detail());
			}
			if (resource.startsWith('/api/supervision/'))
				return reply(route, 'unmocked supervision read', 404);
			return reply(route, {});
		}
	);
	return {
		reads,
		writes,
		release: held.release,
		count: (resource: string) => counts.get(resource) ?? 0
	};
}

async function navigate(page: Page, href: string) {
	await page.evaluate((destination) => {
		const link = document.createElement('a');
		link.href = destination;
		link.textContent = 'test navigation';
		link.dataset.sveltekitPreloadData = 'off';
		link.style.cssText = 'position:fixed;right:10px;bottom:10px;z-index:9999;background:white';
		document.body.append(link);
	}, href);
	await page.getByRole('link', { name: 'test navigation' }).click();
}

test('explicit overview renders teacher status while cycle options are pending', async ({
	page
}) => {
	const api = await mock(page, {
		hold: 'cycles',
		permissions: ['supervision.manage.organization_unit']
	});
	await page.goto(path('overview', year, firstCycle));
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	await expect(page.getByRole('link', { name: 'คำขอจอง', exact: true })).toHaveAttribute(
		'href',
		path('requests')
	);
	await expect(page.getByTestId('supervision-cycle-selector').getByRole('status')).toBeVisible();
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(1);
	expect(api.reads.some((value) => value.endsWith('/progress'))).toBe(false);
	api.release();
	await expect(
		page
			.getByTestId('supervision-cycle-selector')
			.getByRole('button', { name: 'รอบแรก', exact: true })
	).toBeVisible();
});

test('default overview waits only for cycles to select the first ID', async ({ page }) => {
	const api = await mock(page, { hold: 'cycles' });
	await page.goto(path('overview'));
	await expect(page.getByTestId('supervision-teacher-status').getByRole('status')).toBeVisible();
	expect(api.reads.filter((value) => value.endsWith('/teacher-status'))).toHaveLength(0);
	api.release();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('overview retries status without rereading cycles', async ({ page }) => {
	const api = await mock(page, {
		fail: 'teacher-status',
		permissions: ['supervision.read.school']
	});
	await page.goto(path('overview', year, firstCycle));
	const region = page.getByTestId('supervision-teacher-status');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(2);
});

test('overview cycle selection and Back supersede pending status without broad reads', async ({
	page
}) => {
	const api = await mock(page, { hold: 'teacher-status' });
	await page.goto(path('overview', year, firstCycle));
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	await page
		.getByTestId('supervision-cycle-selector')
		.getByRole('button', { name: 'รอบแรก', exact: true })
		.click();
	await page.getByRole('option', { name: /รอบที่สอง/ }).click();
	await expect(page).toHaveURL(new RegExp(`cycleId=${secondCycle}`));
	await expect(page.getByText('ครู รอบแรก')).toHaveCount(0);
	await page.goBack();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	api.release();
	await expect(page.getByText('ครู รอบสอง')).toHaveCount(0);
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('cycles starts with its own skeleton, keeps template choices lazy, and patches status', async ({
	page
}) => {
	const api = await mock(page, { hold: 'cycles' });
	await page.goto(path('cycles'));
	await expect(page.getByTestId('supervision-cycles').getByRole('status')).toBeVisible();
	await expect(page.getByText('ยังไม่มีรอบนิเทศ')).toHaveCount(0);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
	api.release();
	await expect(page.getByTestId('supervision-cycles-ready')).toBeVisible();
	await page.getByRole('button', { name: 'สร้างรอบนิเทศ', exact: true }).first().click();
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'แบบประเมินทดสอบ', exact: true })
	).toBeVisible();
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
	await page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page
		.getByRole('row', { name: /รอบแรก/ })
		.getByRole('button', { name: 'ปิดรอบ' })
		.click();
	await expect(
		page.getByRole('row', { name: /รอบแรก/ }).getByRole('button', { name: 'เปิดอีกครั้ง' })
	).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('cycles retains data through a failed refresh and retries only its list', async ({ page }) => {
	const api = await mock(page, { fail: 'cycles', failAt: 2 });
	await page.goto(path('cycles'));
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('supervision-cycles').getByText('region ไม่พร้อม')).toBeVisible();
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	await page.getByTestId('supervision-cycles').getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count('/api/supervision/cycles')).toBe(3);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
});

test('cycles ignores a late old-year list', async ({ page }) => {
	const api = await mock(page, { hold: 'cycles' });
	await page.goto(path('cycles'));
	await expect(page.getByTestId('supervision-cycles').getByRole('status')).toBeVisible();
	await navigate(page, path('cycles', nextYear));
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('รอบแรก', { exact: true })).toHaveCount(0);
});

test('templates loads summaries, then retrieves the selected rubric only on preview', async ({
	page
}) => {
	const api = await mock(page, { hold: 'detail' });
	await page.goto(path('templates'));
	await expect(page.getByTestId('supervision-templates-ready')).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	await page.getByRole('button', { name: 'ดูตัวอย่าง', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	api.release();
	await expect(page.getByRole('dialog').getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(1);
});

test('template preview has a focused retry', async ({ page }) => {
	const api = await mock(page, { fail: 'detail' });
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'ดูตัวอย่าง', exact: true }).click();
	await expect(page.getByRole('dialog').getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByRole('dialog').getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('template edit patches the returned summary without rereading any list', async ({ page }) => {
	const api = await mock(page);
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByPlaceholder('ชื่อแบบประเมิน')).toHaveValue('แบบประเมินทดสอบ');
	await dialog.getByPlaceholder('ชื่อแบบประเมิน').fill('แบบประเมินที่แก้แล้ว');
	await dialog.getByRole('button', { name: 'บันทึกแบบประเมิน', exact: true }).click();
	await expect(
		page.getByTestId('supervision-templates-ready').getByText('แบบประเมินที่แก้แล้ว')
	).toBeVisible();
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
	expect(api.count('/api/supervision/cycles')).toBe(0);
});

test('denied management access does not preload a protected collection', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.request.own'] });
	await page.goto(path('cycles'));
	await expect(page).toHaveURL(/\/403/);
	expect(api.reads.filter((value) => value.startsWith('/api/supervision/'))).toHaveLength(0);
});

test('a closed template editor stays closed after its delayed detail arrives', async ({ page }) => {
	const api = await mock(page, { hold: 'detail' });
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	api.release();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'สร้างแบบประเมิน', exact: true }).first().click();
	await expect(page.getByRole('dialog').getByPlaceholder('ชื่อแบบประเมิน')).toHaveValue(
		'แบบนิเทศการจัดการเรียนรู้'
	);
});

test('default overview retry recovers the failed cycle dependency before status', async ({
	page
}) => {
	const api = await mock(page, { fail: 'cycles' });
	await page.goto(path('overview'));
	const region = page.getByTestId('supervision-teacher-status');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('ครู รอบแรก')).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(2);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(1);
});

test('a pending cycle mutation does not block or patch the next academic year', async ({
	page
}) => {
	const api = await mock(page, { hold: 'patch' });
	await page.goto(path('cycles'));
	await page
		.getByRole('row', { name: /รอบแรก/ })
		.getByRole('button', { name: 'ปิดรอบ' })
		.click();
	await navigate(page, path('cycles', nextYear));
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('row', { name: /รอบปีใหม่/ }).getByRole('button', { name: 'ปิดรอบ' })
	).toBeEnabled();
	api.release();
	await expect(page.getByText('รอบแรก', { exact: true })).toHaveCount(0);
});
