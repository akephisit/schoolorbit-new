import { test, expect } from '@playwright/test';
import {
	mockFacility,
	buildingsPath,
	roomsPath,
	secondBuilding
} from './fixtures/facility-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});
const path = '/staff/facility/buildings';
test('default buildings are loader owned and unopened rooms issue no read', async ({ page }) => {
	const api = await mockFacility(page, { hold: buildingsPath });
	await page.goto(path);
	await expect(page.getByRole('status', { name: 'กำลังโหลดอาคาร' })).toBeVisible();
	expect(api.count(buildingsPath)).toBe(1);
	expect(api.count(roomsPath)).toBe(0);
	api.release();
	await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
	expect(api.count(buildingsPath)).toBe(1);
});
test('opened rooms render independently while building options are still pending', async ({
	page
}) => {
	const api = await mockFacility(page, { hold: buildingsPath });
	await page.goto(path);
	await page.getByRole('tab', { name: 'ห้องเรียน/ห้องปฏิบัติการ', exact: true }).click();
	await expect(page.getByTestId('facility-rooms')).toContainText('ห้องแรก');
	await expect(page.getByRole('button', { name: 'เพิ่มห้อง', exact: true })).toBeDisabled();
	expect(api.count(buildingsPath)).toBe(1);
	expect(api.count(roomsPath)).toBe(1);
	api.release();
	await expect(page.getByRole('button', { name: 'เพิ่มห้อง', exact: true })).toBeEnabled();
});
for (const endpoint of [buildingsPath, roomsPath])
	test(`${endpoint} has focused error and retry`, async ({ page }) => {
		const api = await mockFacility(page, { fail: endpoint });
		await page.goto(path);
		if (endpoint === roomsPath)
			await page.getByRole('tab', { name: 'ห้องเรียน/ห้องปฏิบัติการ', exact: true }).click();
		const region = page.getByTestId(
			endpoint === roomsPath ? 'facility-rooms' : 'facility-buildings'
		);
		await expect(region).toContainText('ส่วนนี้ไม่พร้อม');
		await region.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(region).toContainText(endpoint === roomsPath ? 'ห้องแรก' : 'อาคารแรก');
		expect(api.count(endpoint)).toBe(2);
		expect(api.count(endpoint === roomsPath ? buildingsPath : roomsPath)).toBe(
			endpoint === roomsPath ? 1 : 0
		);
	});
test('old room filters cannot overwrite a later building selection', async ({ page }) => {
	const api = await mockFacility(page, { hold: roomsPath });
	await page.goto(path);
	await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
	await page.getByRole('tab', { name: 'ห้องเรียน/ห้องปฏิบัติการ', exact: true }).click();
	await expect(page.getByRole('status', { name: 'กำลังโหลดห้อง', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'เลือกอาคารสำหรับห้อง', exact: true }).click();
	await page.getByRole('option', { name: 'อาคารสอง', exact: true }).click();
	await expect(page.getByTestId('facility-rooms')).toContainText('ห้องสอง');
	api.release();
	await expect(page.getByTestId('facility-rooms')).not.toContainText('ห้องแรก');
	expect(
		api.reads
			.filter((url) => url.pathname === roomsPath)
			.at(-1)
			?.searchParams.get('building_id')
	).toBe(secondBuilding);
	expect(api.count(buildingsPath)).toBe(1);
});
test('typed room save patches only matching rooms without primary rereads', async ({ page }) => {
	const api = await mockFacility(page);
	await page.goto(path);
	await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
	await page.getByRole('tab', { name: 'ห้องเรียน/ห้องปฏิบัติการ', exact: true }).click();
	await page.getByRole('button', { name: 'แก้ไขห้อง ห้องแรก', exact: true }).click();
	await page.getByRole('dialog').locator('input[name=name_th]').fill('ห้องแก้ไข');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByTestId('facility-rooms')).toContainText('ห้องแก้ไข');
	expect(api.count(buildingsPath)).toBe(1);
	expect(api.count(roomsPath)).toBe(1);
});
test('building deletion leaves visible rooms present with the nullable building relationship', async ({
	page
}) => {
	const api = await mockFacility(page, { hold: 'mutation' });
	await page.goto(path);
	await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
	await page.getByRole('button', { name: 'ลบอาคาร อาคารแรก', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยันลบ', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('tab', { name: 'ห้องเรียน/ห้องปฏิบัติการ', exact: true }).click();
	const row = page.getByTestId('facility-rooms').getByRole('row').filter({ hasText: 'ห้องแรก' });
	await expect(row).toContainText('อาคารแรก');
	api.release();
	await expect(row).not.toContainText('อาคารแรก');
	await expect(row).toContainText('ห้องแรก');
	expect(api.count(buildingsPath)).toBe(1);
	expect(api.count(roomsPath)).toBe(1);
});
test('a late closed building save cannot close a reopened draft', async ({ page }) => {
	const api = await mockFacility(page, { hold: 'mutation' });
	await page.goto(path);
	await page.getByRole('button', { name: 'แก้ไขอาคาร อาคารแรก', exact: true }).click();
	await page.getByRole('dialog').locator('input[name=name_th]').fill('อาคารแก้ไข');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'เพิ่มอาคาร', exact: true }).click();
	await page.getByRole('dialog').locator('input[name=name_th]').fill('ร่างใหม่');
	api.release();
	await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแก้ไข');
	await expect(page.getByRole('dialog').locator('input[name=name_th]')).toHaveValue('ร่างใหม่');
});
test('read-only facility visits do not start mutation workflows', async ({ page }) => {
	const api = await mockFacility(page, { permissions: ['facility.read.all'] });
	await page.goto(path);
	await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
	await expect(page.getByRole('button', { name: 'เพิ่มอาคาร', exact: true })).toHaveCount(0);
	expect(api.writes).toHaveLength(0);
	expect(api.count(roomsPath)).toBe(0);
});
test('a disposed room read does not restore its route', async ({ page }) => {
	const api = await mockFacility(page, { hold: roomsPath });
	await page.goto(path);
	await page.getByRole('tab', { name: 'ห้องเรียน/ห้องปฏิบัติการ', exact: true }).click();
	await expect(page.getByRole('status', { name: 'กำลังโหลดห้อง', exact: true })).toBeVisible();
	await navigate(page, '/staff');
	await expect(page.getByTestId('facility-rooms')).toHaveCount(0);
	await expect(page).toHaveURL(/\/staff(?:\?|$)/);
	api.release();
	await expect(page.getByTestId('facility-rooms')).toHaveCount(0);
});
