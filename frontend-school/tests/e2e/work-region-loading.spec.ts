import { test, expect } from '@playwright/test';
import { mockWork, itemsPath, countsPath, windowsPath } from './fixtures/work-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
for (const kind of ['items', 'counts']) {
	test(`${kind} can be slow while the independent work region renders`, async ({ page }) => {
		const api = await mockWork(page, { hold: kind });
		await page.goto('/staff/work');
		await expect(page.getByTestId(kind === 'items' ? 'work-counts' : 'work-items')).toContainText(
			kind === 'items' ? 'เปิดอยู่' : 'งานแรก'
		);
		await expect.poll(() => api.count(itemsPath)).toBe(1);
		expect(api.count(countsPath)).toBe(1);
		api.release();
		await expect(page.getByTestId('work-items')).toContainText('งานแรก');
	});
	test(`${kind} retries without rereading its sibling`, async ({ page }) => {
		const api = await mockWork(page, { fail: kind });
		await page.goto('/staff/work');
		const region = page.getByTestId(kind === 'items' ? 'work-items' : 'work-counts');
		await expect(region).toContainText(`region ${kind} ไม่พร้อม`);
		await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(region).not.toContainText(`region ${kind} ไม่พร้อม`);
		expect(api.count(itemsPath)).toBe(kind === 'items' ? 2 : 1);
		expect(api.count(countsPath)).toBe(kind === 'counts' ? 2 : 1);
	});
}
test('active work route reconciles a realtime signal and retains usable items through failure', async ({
	page
}) => {
	const api = await mockWork(page, { event: 'work_items_changed', fail: 'items', failAt: 2 });
	await page.goto('/staff/work');
	await expect(page.getByTestId('work-items')).toContainText('งานแรก');
	api.releaseEvent();
	await expect(page.getByTestId('work-items')).toContainText('region items ไม่พร้อม');
	await expect(page.getByTestId('work-items')).toContainText('งานแรก');
	await page.getByTestId('work-items').getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByTestId('work-items')).toContainText('งานปรับปรุง');
	expect(api.count(countsPath)).toBe(2);
});
test('another page updates shared counts without fetching a hidden work list', async ({ page }) => {
	const api = await mockWork(page, { event: 'work_items_changed' });
	await page.goto('/staff/profile');
	await expect(page.locator('#nickname')).toHaveValue('ชื่อเล่นเดิม');
	api.releaseEvent();
	await expect.poll(() => api.count(countsPath)).toBe(2);
	expect(api.count(itemsPath)).toBe(0);
});
test('late work response cannot refill the next page', async ({ page }) => {
	const api = await mockWork(page, { hold: 'items' });
	await page.goto('/staff/work');
	await expect(page.getByTestId('work-items').getByRole('status')).toBeVisible();
	await navigate(page, '/staff/profile');
	api.release();
	await expect(page.getByTestId('work-items')).toHaveCount(0);
	await expect(page.getByText('งานแรก', { exact: true })).toHaveCount(0);
});
test('windows start in their own skeleton and optional forms perform no reads', async ({
	page
}) => {
	const api = await mockWork(page, { hold: 'windows' });
	await page.goto('/staff/work/manage');
	await expect(page.getByTestId('work-windows').getByRole('status')).toBeVisible();
	expect(api.count('/api/lookup/staff')).toBe(0);
	expect(api.count('/api/lookup/organization-units')).toBe(0);
	api.release();
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
});
test('window refresh retains data on failure and retries without options', async ({ page }) => {
	const api = await mockWork(page, { fail: 'windows', failAt: 2 });
	await page.goto('/staff/work/manage');
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('work-windows')).toContainText('region windows ไม่พร้อม');
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
	await page.getByTestId('work-windows').getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	expect(api.count('/api/lookup/staff')).toBe(0);
	expect(api.count('/api/lookup/organization-units')).toBe(0);
});
for (const kind of ['staff', 'units'])
	test(`opened ${kind} options retry only their workflow`, async ({ page }) => {
		const api = await mockWork(page, { fail: kind });
		await page.goto('/staff/work/manage');
		await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
		await page
			.getByRole('button', {
				name: kind === 'staff' ? 'เปิดฟอร์มมอบหมายงาน' : 'เปิดฟอร์มสร้างรอบงาน',
				exact: true
			})
			.click();
		const region = page.getByTestId(kind === 'staff' ? 'work-staff-options' : 'work-unit-options');
		await expect(region).toContainText(`region ${kind} ไม่พร้อม`);
		await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(region).not.toContainText(`region ${kind} ไม่พร้อม`);
		expect(api.count(windowsPath)).toBe(1);
		expect(
			api.count(kind === 'staff' ? '/api/lookup/organization-units' : '/api/lookup/staff')
		).toBe(0);
	});
test('typed window creation and status patch the primary list without refetch', async ({
	page
}) => {
	const api = await mockWork(page);
	await page.goto('/staff/work/manage');
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
	await page.getByRole('button', { name: 'เปิด', exact: true }).first().click();
	await expect(
		page.getByTestId('work-windows').getByText('เปิดอยู่', { exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'เปิดฟอร์มสร้างรอบงาน', exact: true }).click();
	await page.getByLabel('ชื่อรอบงาน', { exact: true }).fill('รอบงานใหม่');
	await page.getByLabel('รหัสงาน', { exact: true }).fill('new-code');
	await page.getByRole('button', { name: 'สร้างรอบงาน', exact: true }).click();
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานใหม่');
	expect(api.count(windowsPath)).toBe(1);
});
test('assignment UUID success clears its draft without refetching windows', async ({ page }) => {
	const api = await mockWork(page);
	await page.goto('/staff/work/manage');
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
	await page.getByRole('button', { name: 'เปิดฟอร์มมอบหมายงาน', exact: true }).click();
	await page.getByLabel('ชื่องาน', { exact: true }).fill('งานใหม่');
	await page.getByRole('button', { name: 'ครู/บุคลากร', exact: true }).click();
	await page.getByRole('option', { name: 'ผู้รับงาน', exact: true }).click();
	await page.getByRole('button', { name: 'มอบหมายงาน', exact: true }).click();
	await expect(page.getByLabel('ชื่องาน', { exact: true })).toHaveValue('');
	expect(api.count(windowsPath)).toBe(1);
	expect(api.writes[0].path).toBe('/api/work-items');
});
for (const event of ['work_items_changed', 'workflow_window_changed'])
	test(`manage route responds only to relevant ${event} signal`, async ({ page }) => {
		const api = await mockWork(page, { event });
		await page.goto('/staff/work/manage');
		await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
		api.releaseEvent();
		await expect.poll(() => api.count(countsPath)).toBe(2);
		await expect
			.poll(() => api.count(windowsPath))
			.toBe(event === 'workflow_window_changed' ? 2 : 1);
		expect(api.count(itemsPath)).toBe(0);
	});
test('manager access denial never requests windows or options', async ({ page }) => {
	const api = await mockWork(page, { permissions: [] });
	await page.goto('/staff/work/manage');
	await expect(page).toHaveURL(/\/403/);
	expect(api.count(windowsPath)).toBe(0);
	expect(api.count('/api/lookup/staff')).toBe(0);
});
test('late closed create mutation cannot clear a newly opened draft', async ({ page }) => {
	const api = await mockWork(page, { hold: 'mutation' });
	await page.goto('/staff/work/manage');
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
	await page.getByRole('button', { name: 'เปิดฟอร์มสร้างรอบงาน', exact: true }).click();
	await page.getByLabel('ชื่อรอบงาน', { exact: true }).fill('รอบเดิม');
	await page.getByLabel('รหัสงาน', { exact: true }).fill('old-code');
	await page.getByRole('button', { name: 'สร้างรอบงาน', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.getByRole('button', { name: 'ปิดฟอร์มสร้างรอบงาน', exact: true }).click();
	await page.getByRole('button', { name: 'เปิดฟอร์มสร้างรอบงาน', exact: true }).click();
	await page.getByLabel('ชื่อรอบงาน', { exact: true }).fill('ร่างใหม่');
	api.release();
	await expect(page.getByTestId('work-windows')).toContainText('รอบเดิม');
	await expect(page.getByLabel('ชื่อรอบงาน', { exact: true })).toHaveValue('ร่างใหม่');
});
for (const kind of ['staff', 'units'])
	test(`opened ${kind} options show a focused first skeleton`, async ({ page }) => {
		const api = await mockWork(page, { hold: kind });
		await page.goto('/staff/work/manage');
		await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
		await page
			.getByRole('button', {
				name: kind === 'staff' ? 'เปิดฟอร์มมอบหมายงาน' : 'เปิดฟอร์มสร้างรอบงาน',
				exact: true
			})
			.click();
		await expect(
			page
				.getByTestId(kind === 'staff' ? 'work-staff-options' : 'work-unit-options')
				.getByRole('status')
		).toBeVisible();
		await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
		api.release();
		await expect(
			page
				.getByTestId(kind === 'staff' ? 'work-staff-options' : 'work-unit-options')
				.getByRole('status')
		).toHaveCount(0);
	});
test('late assignment cannot clear a draft for another selected window', async ({ page }) => {
	const api = await mockWork(page, { hold: 'mutation' });
	await page.goto('/staff/work/manage');
	await expect(page.getByTestId('work-windows')).toContainText('รอบงานแรก');
	await page.getByRole('button', { name: 'เปิดฟอร์มมอบหมายงาน', exact: true }).click();
	await page.getByLabel('ชื่องาน', { exact: true }).fill('งานเดิม');
	await page.getByRole('button', { name: 'ครู/บุคลากร', exact: true }).click();
	await page.getByRole('option', { name: 'ผู้รับงาน', exact: true }).click();
	await page.getByRole('button', { name: 'มอบหมายงาน', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.getByRole('button', { name: 'รอบงานสอง', exact: true }).click();
	await page.getByLabel('ชื่องาน', { exact: true }).fill('ร่างรอบใหม่');
	api.release();
	await expect(page.getByRole('button', { name: 'มอบหมายงาน', exact: true })).toBeEnabled();
	await expect(page.getByLabel('ชื่องาน', { exact: true })).toHaveValue('ร่างรอบใหม่');
	expect(api.count(windowsPath)).toBe(1);
});
