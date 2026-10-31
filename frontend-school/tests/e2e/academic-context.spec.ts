import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';

declare global {
	interface Window {
		__academicReplaceCount?: number;
	}
}

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	activeYear: '10000000-0000-4000-8000-000000000001',
	planningYear: '10000000-0000-4000-8000-000000000002',
	closedYear: '10000000-0000-4000-8000-000000000003',
	activeTerm: '20000000-0000-4000-8000-000000000001',
	readyTerm: '20000000-0000-4000-8000-000000000002',
	planningTerm: '20000000-0000-4000-8000-000000000003'
};

const contextOptions = {
	defaultAcademicYearId: ids.activeYear,
	defaultAcademicTermId: ids.activeTerm,
	years: [
		{
			isVisible: true,
			id: ids.activeYear,
			name: 'ปีการศึกษา 2570',
			year: 2570,
			status: 'active'
		},
		{
			isVisible: true,
			id: ids.planningYear,
			name: 'ปีการศึกษา 2571',
			year: 2571,
			status: 'planning'
		},
		{
			isVisible: true,
			id: ids.closedYear,
			name: 'ปีการศึกษา 2569',
			year: 2569,
			status: 'closed'
		}
	],
	terms: [
		{
			isVisible: true,
			id: ids.activeTerm,
			academicYearId: ids.activeYear,
			name: 'ภาคเรียนที่ 1',
			code: '1',
			sequence: 1,
			termType: 'regular',
			status: 'active',
			includedInYearResult: true,
			blocksYearClosure: true
		},
		{
			isVisible: true,
			id: ids.readyTerm,
			academicYearId: ids.activeYear,
			name: 'ภาคเรียนที่ 2',
			code: '2',
			sequence: 2,
			termType: 'regular',
			status: 'ready',
			includedInYearResult: true,
			blocksYearClosure: true
		},
		{
			isVisible: true,
			id: ids.planningTerm,
			academicYearId: ids.planningYear,
			name: 'ภาคฤดูร้อน',
			code: 'summer',
			sequence: 1,
			termType: 'summer',
			status: 'planning',
			includedInYearResult: false,
			blocksYearClosure: true
		}
	]
};

function fulfillJson(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mockAcademicContextApis(
	page: Page,
	options: {
		contextFailure?: boolean;
		contextGate?: Promise<void>;
		navigationMenu?: boolean;
	} = {}
) {
	const contextRequests: Array<{ method: string; pathname: string }> = [];

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());

			if (url.pathname.startsWith('/api/academic/context')) {
				contextRequests.push({ method: request.method(), pathname: url.pathname });
			}

			if (url.pathname === '/api/auth/me') {
				await fulfillJson(route, {
					id: '30000000-0000-4000-8000-000000000001',
					username: 'academic-admin',
					firstName: 'วิชาการ',
					displayName: 'วิชาการ ทดสอบ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					createdAt: '2026-08-24T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['*', 'academic_context.read.school']
				});
				return;
			}

			if (url.pathname === '/api/academic/context/options') {
				await options.contextGate;
				if (options.contextFailure) {
					await fulfillJson(route, 'ไม่สามารถโหลดบริบทการศึกษาได้', 503);
				} else {
					await fulfillJson(route, contextOptions);
				}
				return;
			}

			if (url.pathname === '/api/academic/delivery-versions') {
				await fulfillJson(route, []);
				return;
			}
			if (
				[
					'/api/supervision/cycles',
					'/api/supervision/templates/summaries',
					'/api/supervision/observations'
				].includes(url.pathname)
			) {
				await fulfillJson(route, { items: [] });
				return;
			}

			if (url.pathname === '/api/academic/delivery/homerooms') {
				await fulfillJson(route, {
					academicYearId: url.searchParams.get('academicYearId'),
					academicTermId: url.searchParams.get('academicTermId'),
					timetableVersionId: null,
					timetableVersionStatus: null,
					timetableVersionEffectiveFrom: null,
					homerooms: [],
					unlinked: []
				});
				return;
			}

			if (url.pathname === '/api/academic/term-change-sets') {
				await fulfillJson(route, []);
				return;
			}

			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}

			if (url.pathname === '/api/school/settings') {
				await fulfillJson(route, 'forbidden', 403);
				return;
			}

			if (url.pathname === '/api/menu/user') {
				await fulfillJson(route, {
					groups: options.navigationMenu
						? [
								{
									code: 'context-navigation',
									name: 'บริบททดสอบ',
									icon: 'Circle',
									workspaceCode: 'home',
									workspaceName: 'งานของฉัน',
									workspaceIcon: 'Home',
									workspaceOrder: 1,
									displayOrder: 1,
									items: [
										{
											id: 'home',
											name: 'หน้าหลักทดสอบ',
											path: '/staff',
											icon: 'Home',
											displayOrder: 1,
											requiredPermission: null,
											userType: 'staff'
										},
										{
											id: 'delivery',
											name: 'รายการเปิดสอนทดสอบ',
											path: '/staff/academic/delivery',
											icon: 'BookOpen',
											displayOrder: 2,
											requiredPermission: null,
											userType: 'staff'
										},
										{
											id: 'supervision',
											name: 'นิเทศทดสอบ',
											path: '/staff/academic/supervision',
											icon: 'Eye',
											displayOrder: 3,
											requiredPermission: null,
											userType: 'staff'
										}
									]
								}
							]
						: []
				});
				return;
			}

			if (url.pathname === '/api/me/work-items/counts') {
				await fulfillJson(route, {
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
				await fulfillJson(route, { items: [], unread_count: 0 });
				return;
			}

			if (url.pathname === '/api/school/public') {
				await fulfillJson(route, { schoolName: 'โรงเรียนทดสอบ' });
				return;
			}

			await fulfillJson(route, {});
		}
	);

	return contextRequests;
}

async function openDelivery(page: Page, query = '') {
	await page.goto(`/staff/academic/delivery${query}`);
	await expect(page.getByTestId('academic-context-switcher')).toBeVisible();
}

test('missing required IDs use explicit school defaults with replaceState and no mutation', async ({
	page
}) => {
	await page.addInitScript(() => {
		const originalReplaceState = history.replaceState.bind(history);
		Object.defineProperty(window, '__academicReplaceCount', {
			value: 0,
			writable: true
		});
		history.replaceState = (data, unused, url) => {
			if (String(url).includes('academicYearId=')) {
				window.__academicReplaceCount = (window.__academicReplaceCount ?? 0) + 1;
			}
			return originalReplaceState(data, unused, url);
		};
	});
	const requests = await mockAcademicContextApis(page);
	await openDelivery(page, '?view=grid#offerings');

	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.activeYear}`));
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.activeTerm}`));
	await expect(page).toHaveURL(/view=grid/);
	await expect(page.getByLabel('เลือกปีการศึกษา', { exact: true })).toContainText(
		'ปีการศึกษา 2570'
	);
	await expect(page.getByLabel('เลือกภาคเรียน', { exact: true })).toContainText('ภาคเรียนที่ 1');
	expect(await page.evaluate(() => window.__academicReplaceCount ?? 0)).toBeGreaterThan(0);
	expect(requests).toEqual([{ method: 'GET', pathname: '/api/academic/context/options' }]);
});

test('missing direct-link context stays neutral while fallback repair is pending', async ({
	page
}) => {
	let releaseContext: () => void = () => {};
	const contextGate = new Promise<void>((resolve) => {
		releaseContext = resolve;
	});
	await mockAcademicContextApis(page, { contextGate });
	await page.goto('/staff/academic/delivery');
	await expect(page.getByTestId('academic-context-switcher')).toBeVisible();

	try {
		await expect(page.getByText('เลือกปีการศึกษาและภาคเรียนก่อน')).toHaveCount(0);
	} finally {
		releaseContext();
	}

	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.activeYear}`));
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.activeTerm}`));
});

test('year changes remove an incompatible term and term-optional routes offer all year', async ({
	page
}) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);

	await page.getByLabel('เลือกปีการศึกษา', { exact: true }).click();
	await page.getByRole('option', { name: /ปีการศึกษา 2571/ }).click();
	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.planningYear}`));
	await expect(page).not.toHaveURL(/academicTermId=/);

	await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
	await page.getByRole('option', { name: /ภาคฤดูร้อน/ }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.planningTerm}`));

	await page.goto(
		`/staff/academic/supervision?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}&view=calendar`
	);
	await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
	await page.getByRole('option', { name: 'ทั้งปี', exact: true }).click();
	await expect(page.getByLabel('เลือกภาคเรียน', { exact: true })).toContainText('ทั้งปี');
	await expect(page).not.toHaveURL(/academicTermId=/);
	await expect(page).toHaveURL(/view=calendar/);
});

test('normal history restores selected terms', async ({ page }) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);

	await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
	await page.getByRole('option', { name: /ภาคเรียนที่ 2/ }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.readyTerm}`));

	await page.goBack();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.activeTerm}`));
	await expect(page.getByLabel('เลือกภาคเรียน', { exact: true })).toContainText('ภาคเรียนที่ 1');

	await page.goForward();
	await expect(page.getByLabel('เลือกภาคเรียน', { exact: true })).toContainText('ภาคเรียนที่ 2');
});

test('dirty sources require confirmation and cancelling preserves the context', async ({
	page
}) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);
	await page.evaluate(async () => {
		const modulePath =
			performance
				.getEntriesByType('resource')
				.map((entry) => entry.name)
				.find((name) => new URL(name).pathname === '/src/lib/academic-context/store.ts') ??
			'/src/lib/academic-context/store.ts';
		const { registerAcademicContextDirtySource } = await import(/* @vite-ignore */ modulePath);
		registerAcademicContextDirtySource('playwright-draft', () => true);
	});

	await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
	await page.getByRole('option', { name: /ภาคเรียนที่ 2/ }).click();
	await expect(page.getByRole('alertdialog')).toBeVisible();
	await page.getByRole('button', { name: 'อยู่หน้านี้ต่อ' }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.activeTerm}`));
	await expect(page.getByRole('alertdialog')).toHaveCount(0);

	await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
	await page.getByRole('option', { name: /ภาคเรียนที่ 2/ }).click();
	await page.getByRole('button', { name: 'เปลี่ยนบริบท' }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.readyTerm}`));
});

test('mobile trigger summarizes the context and exposes both controls', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);

	const trigger = page.getByTestId('academic-context-mobile-trigger');
	await expect(trigger).toContainText('2570');
	await expect(trigger).toContainText('ภาคเรียนที่ 1');
	await trigger.click();
	await expect(page.getByRole('dialog', { name: 'เลือกบริบทการศึกษา' })).toBeVisible();
	await expect(page.getByLabel('เลือกปีการศึกษา (มือถือ)')).toBeVisible();
	await expect(page.getByLabel('เลือกภาคเรียน (มือถือ)')).toBeVisible();
});

for (const explicit of [
	{ name: 'malformed', year: 'not-a-year', term: 'not-a-term' },
	{
		name: 'hidden',
		year: '10000000-0000-4000-8000-000000000004',
		term: '20000000-0000-4000-8000-000000000004'
	},
	{
		name: 'unavailable year',
		year: '10000000-0000-4000-8000-000000000005',
		term: '20000000-0000-4000-8000-000000000005'
	},
	{ name: 'unavailable term', year: ids.activeYear, term: '20000000-0000-4000-8000-000000000005' }
]) {
	test(`Sidebar${explicit.year === ids.activeYear ? '' : ' and home'} navigation preserve ${explicit.name} explicit context without querying school defaults`, async ({
		page
	}) => {
		const contextReads: URL[] = [];
		page.on('request', (request) => {
			const url = new URL(request.url());
			if (
				url.pathname.startsWith('/api/academic/delivery') ||
				url.pathname.startsWith('/api/supervision/')
			)
				contextReads.push(url);
		});
		await mockAcademicContextApis(page, { navigationMenu: true });
		const query = `?academicYearId=${explicit.year}&academicTermId=${explicit.term}`;
		await openDelivery(page, query);
		const sidebar = page.locator('aside');
		const supervision = sidebar.getByRole('link', { name: 'นิเทศทดสอบ', exact: true });
		await expect(supervision).toHaveAttribute('href', `/staff/academic/supervision${query}`);
		await supervision.hover();
		await supervision.click();
		await expect(page).toHaveURL(`/staff/academic/supervision${query}`);
		if (explicit.year !== ids.activeYear) {
			// The home route accepts only a year, so it legitimately drops the term.
			const yearQuery = `?academicYearId=${explicit.year}`;
			await sidebar.getByRole('link', { name: 'หน้าหลักทดสอบ', exact: true }).click();
			await expect(page).toHaveURL(`/staff${yearQuery}`);
			const service = page
				.getByTestId('staff-services')
				.getByRole('link', { name: 'รายการเปิดสอนทดสอบ', exact: true });
			await expect(service).toHaveAttribute('href', `/staff/academic/delivery${yearQuery}`);
			await service.hover();
			await service.click();
			await expect(page).toHaveURL(`/staff/academic/delivery${yearQuery}`);
		}
		expect(contextReads.length).toBeGreaterThan(0);
		for (const url of contextReads) {
			if (url.searchParams.has('academicYearId'))
				expect(url.searchParams.get('academicYearId')).toBe(explicit.year);
			if (url.searchParams.has('academicTermId'))
				expect(url.searchParams.get('academicTermId')).toBe(explicit.term);
		}
	});
}

test('status labels are visible and option failure is actionable without guessing defaults', async ({
	page
}) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);
	await page.getByLabel('เลือกปีการศึกษา', { exact: true }).click();
	for (const label of ['กำลังวางแผน', 'กำลังใช้งาน', 'ปิดแล้ว']) {
		await expect(page.getByText(label, { exact: true }).first()).toBeVisible();
	}
	await page.keyboard.press('Escape');
	await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
	await expect(page.getByText('พร้อมใช้งาน', { exact: true }).first()).toBeVisible();

	const failedPage = await page.context().newPage();
	await mockAcademicContextApis(failedPage, { contextFailure: true });
	await failedPage.goto('/staff/academic/delivery');
	await expect(failedPage.getByText('โหลดปีการศึกษาไม่สำเร็จ')).toBeVisible();
	await expect(failedPage.getByRole('button', { name: 'ลองโหลดอีกครั้ง' })).toBeVisible();
	await expect(failedPage).not.toHaveURL(/academicYearId=|academicTermId=/);
	await failedPage.close();
});
