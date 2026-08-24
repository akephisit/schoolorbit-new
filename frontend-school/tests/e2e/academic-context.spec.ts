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
	activeAcademicYearId: ids.activeYear,
	activeAcademicTermId: ids.activeTerm,
	years: [
		{
			id: ids.activeYear,
			name: 'ปีการศึกษา 2570',
			year: 2570,
			status: 'active',
			startDate: '2027-05-01',
			endDate: '2028-03-31'
		},
		{
			id: ids.planningYear,
			name: 'ปีการศึกษา 2571',
			year: 2571,
			status: 'planning',
			startDate: '2028-05-01',
			endDate: '2029-04-30'
		},
		{
			id: ids.closedYear,
			name: 'ปีการศึกษา 2569',
			year: 2569,
			status: 'closed',
			startDate: '2026-05-01',
			endDate: '2027-03-31'
		}
	],
	terms: [
		{
			id: ids.activeTerm,
			academicYearId: ids.activeYear,
			name: 'ภาคเรียนที่ 1',
			code: '1',
			sequence: 1,
			termType: 'regular',
			status: 'active',
			startDate: '2027-05-01',
			endDate: '2027-10-31',
			includedInYearResult: true,
			blocksYearClosure: true
		},
		{
			id: ids.readyTerm,
			academicYearId: ids.activeYear,
			name: 'ภาคเรียนที่ 2',
			code: '2',
			sequence: 2,
			termType: 'regular',
			status: 'ready',
			startDate: '2027-11-01',
			endDate: '2028-03-31',
			includedInYearResult: true,
			blocksYearClosure: true
		},
		{
			id: ids.planningTerm,
			academicYearId: ids.planningYear,
			name: 'ภาคฤดูร้อน',
			code: 'summer',
			sequence: 1,
			termType: 'summer',
			status: 'planning',
			startDate: '2029-03-01',
			endDate: '2029-04-15',
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

async function mockAcademicContextApis(page: Page, options: { contextFailure?: boolean } = {}) {
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
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
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
				if (options.contextFailure) {
					await fulfillJson(route, 'ไม่สามารถโหลดบริบทการศึกษาได้', 503);
				} else {
					await fulfillJson(route, contextOptions);
				}
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
				await fulfillJson(route, { groups: [] });
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

test('missing required IDs use active defaults with replaceState and no mutation', async ({
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
	await expect(page.getByText('กำลังใช้งาน', { exact: true }).first()).toBeVisible();
	expect(await page.evaluate(() => window.__academicReplaceCount ?? 0)).toBeGreaterThan(0);
	expect(requests).toEqual([{ method: 'GET', pathname: '/api/academic/context/options' }]);
});

test('year changes remove an incompatible term and term-optional routes offer all year', async ({
	page
}) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);

	await page.getByLabel('เลือกปีการศึกษา').click();
	await page.getByRole('option', { name: /ปีการศึกษา 2571/ }).click();
	await expect(page).toHaveURL(new RegExp(`academicYearId=${ids.planningYear}`));
	await expect(page).not.toHaveURL(/academicTermId=/);

	await page.getByLabel('เลือกภาคเรียน').click();
	await page.getByRole('option', { name: /ภาคฤดูร้อน/ }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.planningTerm}`));

	await page.goto(
		`/staff/academic/supervision?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}&view=calendar`
	);
	await page.getByLabel('เลือกภาคเรียน').click();
	await page.getByRole('option', { name: 'ทั้งปี', exact: true }).click();
	await expect(page.getByLabel('เลือกภาคเรียน')).toContainText('ทั้งปี');
	await expect(page).not.toHaveURL(/academicTermId=/);
	await expect(page).toHaveURL(/view=calendar/);
});

test('normal history restores selected terms', async ({ page }) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);

	await page.getByLabel('เลือกภาคเรียน').click();
	await page.getByRole('option', { name: /ภาคเรียนที่ 2/ }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.readyTerm}`));

	await page.goBack();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.activeTerm}`));
	await expect(page.getByLabel('เลือกภาคเรียน')).toContainText('ภาคเรียนที่ 1');

	await page.goForward();
	await expect(page.getByLabel('เลือกภาคเรียน')).toContainText('ภาคเรียนที่ 2');
});

test('dirty sources require confirmation and cancelling preserves the context', async ({
	page
}) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);
	await page.evaluate(async () => {
		const modulePath = '/src/lib/academic-context/store.ts';
		const { registerAcademicContextDirtySource } = await import(/* @vite-ignore */ modulePath);
		registerAcademicContextDirtySource('playwright-draft', () => true);
	});

	await page.getByLabel('เลือกภาคเรียน').click();
	await page.getByRole('option', { name: /ภาคเรียนที่ 2/ }).click();
	await expect(page.getByRole('alertdialog')).toBeVisible();
	await page.getByRole('button', { name: 'อยู่หน้านี้ต่อ' }).click();
	await expect(page).toHaveURL(new RegExp(`academicTermId=${ids.activeTerm}`));
	await expect(page.getByRole('alertdialog')).toHaveCount(0);

	await page.getByLabel('เลือกภาคเรียน').click();
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

test('status labels are visible and option failure is actionable without guessing defaults', async ({
	page
}) => {
	await mockAcademicContextApis(page);
	await openDelivery(page, `?academicYearId=${ids.activeYear}&academicTermId=${ids.activeTerm}`);
	await page.getByLabel('เลือกปีการศึกษา').click();
	for (const label of ['กำลังวางแผน', 'กำลังใช้งาน', 'ปิดแล้ว']) {
		await expect(page.getByText(label, { exact: true }).first()).toBeVisible();
	}
	await page.keyboard.press('Escape');
	await page.getByLabel('เลือกภาคเรียน').click();
	await expect(page.getByText('พร้อมใช้งาน', { exact: true }).first()).toBeVisible();

	const failedPage = await page.context().newPage();
	await mockAcademicContextApis(failedPage, { contextFailure: true });
	await failedPage.goto('/staff/academic/delivery');
	await expect(failedPage.getByText('โหลดบริบทการศึกษาไม่สำเร็จ')).toBeVisible();
	await expect(failedPage.getByRole('button', { name: 'ลองโหลดอีกครั้ง' })).toBeVisible();
	await expect(failedPage).not.toHaveURL(/academicYearId=|academicTermId=/);
	await failedPage.close();
});
