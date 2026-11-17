import { test, expect, type Page } from '@playwright/test';
import {
	mockBudget,
	budgetYearSettings,
	budgetYearId,
	budgetRequestId,
	budgetFundId
} from './fixtures/budget';
import { id } from './fixtures/staff-home-route-data';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';

test.use({ serviceWorkers: 'block' });
const workspaceNavigation = (page: Page) =>
	page.getByRole('navigation', { name: 'งานงบประมาณ', exact: true });
async function choose(page: Page, label: string, option: string) {
	await page.getByRole('combobox', { name: label, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}

for (const width of [390, 1440]) {
	test(`local navigation retains fiscal context through pointer and keyboard at ${width}px`, async ({
		page
	}) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockBudget(page, { withRequest: true });
		await page.goto(
			`/staff/budget/projects?fiscalYearId=${budgetYearId}&status=approved&offset=20`
		);
		const navigation = workspaceNavigation(page);
		await expect(navigation.locator('[aria-current="page"]')).toHaveText('โครงการ');
		for (const link of await navigation.getByRole('link').all()) {
			const destination = new URL((await link.getAttribute('href'))!, page.url());
			expect([...destination.searchParams.keys()]).toEqual(['fiscalYearId']);
			expect(destination.searchParams.get('fiscalYearId')).toBe(budgetYearId);
		}
		await navigation.getByRole('link', { name: 'คำขอใช้เงิน', exact: true }).click();
		await expect(page).toHaveURL(`/staff/budget/requests?fiscalYearId=${budgetYearId}`);
		await navigation.getByRole('link', { name: 'รายงาน', exact: true }).focus();
		await page.keyboard.press('Enter');
		await expect(page.getByTestId('budget-pivot')).toContainText('9,928.00');
		await expect(navigation.locator('[aria-current="page"]')).toHaveText('รายงาน');
		const visible = await navigation.evaluate((element) => {
			const current = element.querySelector('[aria-current="page"]')!.getBoundingClientRect();
			const bounds = element.getBoundingClientRect();
			return current.left >= bounds.left && current.right <= bounds.right;
		});
		expect(visible).toBe(true);
		await page.goBack();
		await expect(page).toHaveURL(`/staff/budget/requests?fiscalYearId=${budgetYearId}`);
		expect(api.writes).toEqual([]);
	});
}

for (const [role, permissions, path, links] of [
	['reader', [PERMISSIONS.BUDGET_READ_SCHOOL], '/staff/budget/projects', 5],
	['manager', [PERMISSIONS.BUDGET_MANAGE_SCHOOL], '/staff/budget/settings', 1],
	['finance', [PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL], '/staff/budget/settings', 1]
] as const) {
	test(`${role} sees only authorized local destinations`, async ({ page }) => {
		const api = await mockBudget(page, { permissions: [...permissions] });
		await page.goto(path);
		const navigation = workspaceNavigation(page);
		await expect(navigation.getByRole('link')).toHaveCount(links);
		await expect(navigation.getByRole('link', { name: 'ตั้งค่า', exact: true })).toHaveCount(
			role === 'reader' ? 0 : 1
		);
		await expect(navigation.locator('[aria-current="page"]')).toHaveCount(1);
		expect(api.reads.some((read) => read.includes('/lookup/'))).toBe(false);
		if (role !== 'reader') expect(api.reads.some((read) => read.includes('/projects'))).toBe(false);
		expect(api.writes).toEqual([]);
	});
}

test('module-only access shows missing-read guidance without an empty navigation surface', async ({
	page
}) => {
	const api = await mockBudget(page, { permissions: [PERMISSIONS.BUDGET_CREATE_OWN] });
	await page.goto('/staff/budget/projects');
	await expect(page.getByText('ต้องมีสิทธิ์ดูงบประมาณ', { exact: true })).toBeVisible();
	await expect(workspaceNavigation(page)).toHaveCount(0);
	expect(api.reads).toEqual([]);
	expect(api.writes).toEqual([]);
});

for (const [kind, region] of [
	['projects', 'budget-projects'],
	['requests', 'budget-requests'],
	['adjustments', 'budget-adjustments']
] as const) {
	test(`${kind} filters apply to server URL, reset paging and survive history`, async ({
		page
	}) => {
		const api = await mockBudget(page, { withRequest: true });
		await page.route(
			(url) =>
				url.pathname === `/api/budget/${kind}` && url.searchParams.get('status') === 'rejected',
			(route) =>
				route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({
						success: true,
						data: { items: [], total: 0, offset: 0, limit: 20 }
					})
				})
		);
		const requests: URL[] = [];
		page.on('request', (request) => {
			const url = new URL(request.url());
			if (url.pathname === `/api/budget/${kind}`) requests.push(url);
		});
		await page.goto(`/staff/budget/${kind}?offset=20`);
		await choose(page, 'ปีงบประมาณ', '2570');
		await expect(page).toHaveURL(`/staff/budget/${kind}?fiscalYearId=${budgetYearId}&limit=20`);
		await choose(page, 'สถานะ', 'ไม่อนุมัติ');
		await expect(page.getByTestId(region)).toContainText('ไม่พบ');
		await expect(
			page.getByRole('navigation', { name: 'หน้ารายการงบประมาณ', exact: true })
		).toContainText('0 รายการ');
		expect(requests.at(-1)!.searchParams.get('fiscalYearId')).toBe(budgetYearId);
		expect(requests.at(-1)!.searchParams.get('status')).toBe('rejected');
		expect(requests.at(-1)!.searchParams.get('offset')).toBe('0');
		await page.goBack();
		await expect(page.getByRole('combobox', { name: 'สถานะ', exact: true })).toContainText(
			'ทุกสถานะ'
		);
		await expect(page.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true })).toContainText(
			'2570'
		);
		if (kind === 'projects') {
			await page.getByLabel('ค้นหาโครงการ', { exact: true }).fill('คณิตศาสตร์');
			await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
			await expect(page).toHaveURL((url) => url.searchParams.get('search') === 'คณิตศาสตร์');
			expect(requests.at(-1)!.searchParams.get('search')).toBe('คณิตศาสตร์');
		} else await expect(page.getByRole('textbox')).toHaveCount(0);
		await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).click();
		await expect(page).toHaveURL(`/staff/budget/${kind}?limit=20`);
		await expect(page.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true })).toContainText(
			'ทุกปีงบประมาณ'
		);
		expect(api.writes).toEqual([]);
	});

	test(`${kind} results remain available while year catalogs fail and retry independently`, async ({
		page
	}) => {
		const api = await mockBudget(page, { withRequest: true });
		let catalogReads = 0;
		await page.route('**/api/budget/catalogs*', (route) => {
			catalogReads++;
			return catalogReads === 1
				? route.fulfill({
						status: 503,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'ปีงบประมาณไม่พร้อมชั่วคราว' })
					})
				: route.fallback();
		});
		await page.goto(`/staff/budget/${kind}`);
		await expect(page.getByTestId(region)).toBeVisible();
		await expect(
			page.getByRole('navigation', { name: 'หน้ารายการงบประมาณ', exact: true })
		).toBeVisible();
		await expect(page.getByText('ปีงบประมาณไม่พร้อมชั่วคราว', { exact: true })).toBeVisible();
		const listReads = api.reads.filter((read) => read.startsWith(`/api/budget/${kind}?`)).length;
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(page.getByText('ปีงบประมาณไม่พร้อมชั่วคราว', { exact: true })).toHaveCount(0);
		expect(api.reads.filter((read) => read.startsWith(`/api/budget/${kind}?`)).length).toBe(
			listReads
		);
		await choose(page, 'ปีงบประมาณ', '2570');
		expect(catalogReads).toBeGreaterThanOrEqual(2);
		// The retry itself must not refetch results; the explicit selection owns the next list read.
		await expect
			.poll(() => api.reads.filter((read) => read.startsWith(`/api/budget/${kind}?`)).length)
			.toBe(listReads + 1);
		expect(api.reads.some((read) => read.includes('/settings') || read.includes('/lookup/'))).toBe(
			false
		);
		expect(api.writes).toEqual([]);
	});
}

for (const kind of ['projects', 'adjustments'] as const) {
	for (const state of ['open', 'closed', 'missing'] as const) {
		test(`${kind} creation retains the selected ${state} fiscal year without silently changing it`, async ({
			page
		}) => {
			const next = budgetYearSettings();
			next.fiscalYear = {
				...next.fiscalYear,
				id: id(998),
				name: '2571',
				isClosed: state === 'closed',
				startDate: '2027-10-01',
				endDate: '2028-09-30'
			};
			const selectedId = state === 'missing' ? id(997) : next.fiscalYear.id;
			const api = await mockBudget(page, { yearSettings: [budgetYearSettings(), next] });
			await page.goto(`/staff/budget/${kind}?fiscalYearId=${selectedId}`);
			await page
				.getByRole('button', {
					name: kind === 'projects' ? 'สร้างโครงการ' : 'สร้างรายการปรับงบ',
					exact: true
				})
				.click();
			const dialog = page.getByRole('dialog');
			const selected = dialog.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true });
			if (state === 'missing')
				await expect(
					dialog.getByText('ไม่พบปีงบประมาณที่เลือก กรุณาเลือกปีงบประมาณอีกครั้ง', { exact: true })
				).toBeVisible();
			else await expect(selected).toContainText('2571');
			await expect(selected).not.toContainText('2570');
			if (state === 'closed')
				await expect(dialog.getByRole('status')).toContainText('ปีงบประมาณที่เลือกปิดแล้ว');
			await page.keyboard.press('Escape');
			expect(api.writes).toEqual([]);
		});
	}
}

test('missing adjustment year keeps the year index usable and recovers allocations after a scoped failure', async ({
	page
}) => {
	const missingYearId = id(997);
	const api = await mockBudget(page);
	let failedAllocations = 0;
	await page.route(
		(url) =>
			url.pathname === '/api/budget/dashboard' &&
			url.searchParams.get('fiscalYearId') === missingYearId,
		(route) => {
			failedAllocations++;
			return route.fulfill({
				status: 503,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'อ่านวงเงินของปีที่เลือกไม่สำเร็จ' })
			});
		}
	);
	await page.goto(`/staff/budget/adjustments?fiscalYearId=${missingYearId}`);
	await page.getByRole('button', { name: 'สร้างรายการปรับงบ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByText('ไม่พบปีงบประมาณที่เลือก กรุณาเลือกปีงบประมาณอีกครั้ง', { exact: true })
	).toBeVisible();
	await expect(dialog.getByText('โหลดตัวเลือกงบประมาณไม่สำเร็จ', { exact: true })).toBeVisible();
	const selectedYear = dialog.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true });
	await expect(selectedYear).not.toContainText('2570');
	await dialog.getByLabel('จำนวนเงิน (บาท) *', { exact: true }).fill('12.34');
	await dialog.getByLabel('เหตุผล *', { exact: true }).fill('เก็บเหตุผลระหว่างแก้ไขปี');
	await selectedYear.click();
	const validYear = page.getByRole('option', { name: '2570', exact: true });
	await expect(validYear).toBeVisible();
	await validYear.click();
	await expect(selectedYear).toContainText('2570');
	await expect(dialog.getByText('โหลดตัวเลือกงบประมาณไม่สำเร็จ', { exact: true })).toHaveCount(0);
	await expect(
		dialog.getByText('ไม่พบปีงบประมาณที่เลือก กรุณาเลือกปีงบประมาณอีกครั้ง', { exact: true })
	).toHaveCount(0);
	await expect(dialog.getByLabel('จำนวนเงิน (บาท) *', { exact: true })).toHaveValue('12.34');
	await expect(dialog.getByLabel('เหตุผล *', { exact: true })).toHaveValue(
		'เก็บเหตุผลระหว่างแก้ไขปี'
	);
	await dialog.getByRole('combobox', { name: 'งบประมาณปลายทาง', exact: true }).click();
	await expect(page.getByRole('option').filter({ hasText: 'กิจกรรมที่หนึ่ง' })).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(dialog.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true })).toBeEnabled();
	expect(failedAllocations).toBe(1);
	expect(api.reads.some((read) => read === '/api/budget/catalogs')).toBe(true);
	expect(
		api.reads.some((read) => read === `/api/budget/dashboard?fiscalYearId=${budgetYearId}`)
	).toBe(true);
	expect(api.writes).toEqual([]);
});

test('request navigation reuses explicit return-year context without reading the project', async ({
	page
}) => {
	const api = await mockBudget(page, {
		withRequest: true,
		permissions: [PERMISSIONS.BUDGET_READ_SCHOOL]
	});
	const returnTo = `/staff/budget/requests?fiscalYearId=${budgetYearId}&status=approved`;
	await page.goto(
		`/staff/budget/requests/${budgetRequestId}?returnTo=${encodeURIComponent(returnTo)}`
	);
	await expect(page.getByTestId('budget-request-detail')).toContainText('คำขอ');
	await expect(
		workspaceNavigation(page).getByRole('link', { name: 'รายงาน', exact: true })
	).toHaveAttribute('href', `/staff/budget/reports?fiscalYearId=${budgetYearId}`);
	expect(api.reads.some((read) => read.startsWith('/api/budget/projects'))).toBe(false);
	expect(api.writes).toEqual([]);
});

for (const role of ['manager', 'finance'] as const) {
	test(`Enter in ${role} actual-receipt input only saves receipts and preserves authority`, async ({
		page
	}) => {
		const settings = budgetYearSettings();
		settings.authorizedFunding = [
			{
				fundId: budgetFundId,
				carryoverSatang: '10000',
				annualFundingSatang: '90000',
				reserveSatang: '1000'
			}
		];
		const api = await mockBudget(page, {
			settings,
			permissions:
				role === 'manager'
					? [PERMISSIONS.BUDGET_MANAGE_SCHOOL, PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL]
					: [PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL]
		});
		await page.goto('/staff/budget/settings');
		if (role === 'manager')
			await page.getByRole('tab', { name: 'รับเงินจริง', exact: true }).click();
		await expect(page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true })).toHaveCount(
			0
		);
		const input = page.getByLabel('เงินอุดหนุน (บาท)', { exact: true });
		await input.fill('123.45');
		await input.press('Enter');
		await expect(page.getByText('บันทึกยอดรับเงินจริงแล้ว', { exact: true })).toBeVisible();
		expect(api.writes).toHaveLength(1);
		expect(api.writes[0].path).toBe(`/api/budget/fiscal-years/${budgetYearId}/funding`);
		expect(api.writes[0].payload).not.toHaveProperty('authorizedFunding');
		expect(api.yearSettings.get(budgetYearId)!.authorizedFunding).toEqual(
			settings.authorizedFunding
		);
		expect(
			api.yearSettings
				.get(budgetYearId)!
				.sourceBalances.find((fund) => fund.fundId === budgetFundId)!.receivedSatang
		).toBe('12345');
		expect(api.reads.some((read) => read.includes('/projects') || read.includes('/lookup/'))).toBe(
			false
		);
	});
}
