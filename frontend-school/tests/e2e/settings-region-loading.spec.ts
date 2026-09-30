import { test, expect } from '@playwright/test';
import {
	mockSettings,
	featurePath,
	settingsPath,
	fontsPath,
	uploadedId
} from './fixtures/settings-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});
for (const [route, endpoint, region, text] of [
	['features', featurePath, 'settings-features', 'ระบบงานทดสอบ'],
	['school-settings', settingsPath, 'settings-school', 'Logo โรงเรียน'],
	['school-fonts', fontsPath, 'school-font-library', 'ฟอนต์ทดสอบ']
]) {
	test(`${route} owns one startup read and has a first skeleton`, async ({ page }) => {
		const api = await mockSettings(page, { hold: endpoint });
		await page.goto(`/staff/${route}`);
		await expect(page.getByTestId(region).getByRole('status')).toBeVisible();
		expect(api.count(endpoint)).toBe(1);
		expect(api.writes).toHaveLength(0);
		api.release();
		await expect(page.getByTestId(region)).toContainText(text);
		expect(api.count(endpoint)).toBe(1);
	});
	test(`${route} retries only its failed primary region`, async ({ page }) => {
		const api = await mockSettings(page, { fail: endpoint });
		await page.goto(`/staff/${route}`);
		await expect(page.getByTestId(region)).toContainText('ส่วนนี้ไม่พร้อม');
		await page.getByTestId(region).getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(page.getByTestId(region)).toContainText(text);
		expect(api.count(endpoint)).toBe(2);
		for (const sibling of [featurePath, settingsPath, fontsPath].filter(
			(path) => path !== endpoint
		))
			expect(api.count(sibling)).toBe(0);
	});
}
test('feature refresh failure retains usable cards and typed toggle does no reread', async ({
	page
}) => {
	const api = await mockSettings(page, { fail: featurePath, failAt: 2 });
	await page.goto('/staff/features');
	await expect(page.getByTestId('settings-features')).toContainText('ระบบงานทดสอบ');
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('settings-features')).toContainText('ส่วนนี้ไม่พร้อม');
	await expect(page.getByTestId('settings-features')).toContainText('ระบบงานทดสอบ');
	await page.getByRole('switch').click();
	await expect(page.getByRole('switch')).toBeChecked();
	expect(api.count(featurePath)).toBe(2);
});
test('read-only features have disabled actions and no mutation requests', async ({ page }) => {
	const api = await mockSettings(page, { permissions: ['features.read.all'] });
	await page.goto('/staff/features');
	await expect(page.getByTestId('settings-features')).toContainText('ระบบงานทดสอบ');
	await expect(page.getByRole('switch')).toBeDisabled();
	expect(api.writes).toHaveLength(0);
});
test('logo delete patches its visible state without reloading school settings', async ({
	page
}) => {
	const api = await mockSettings(page);
	await page.goto('/staff/school-settings');
	await expect(page.getByRole('img', { name: 'school logo' })).toBeVisible();
	await page.getByRole('button', { name: /ลบ logo/ }).click();
	await expect(page.getByRole('img', { name: 'school logo' })).toHaveCount(0);
	expect(api.count(settingsPath)).toBe(1);
});
const file = {
	name: 'logo.png',
	mimeType: 'image/png',
	buffer: Buffer.from(
		'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aR1MAAAAASUVORK5CYII=',
		'base64'
	)
};
test('failed logo commit keeps its draft and retries without uploading twice', async ({ page }) => {
	const api = await mockSettings(page, { fail: 'mutation' });
	await page.goto('/staff/school-settings');
	await expect(page.getByTestId('settings-school')).toContainText('Logo โรงเรียน');
	await page.locator('input[type=file]').setInputFiles(file);
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByTestId('settings-school')).toContainText('ยังไม่ได้บันทึก');
	await expect(page.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByRole('img', { name: 'school logo' })).toHaveAttribute(
		'src',
		new RegExp(uploadedId)
	);
	expect(api.count('upload')).toBe(1);
	expect(api.count(settingsPath)).toBe(1);
});
test('late logo upload cannot commit school settings after leaving', async ({ page }) => {
	const api = await mockSettings(page, { hold: 'upload' });
	await page.goto('/staff/school-settings');
	await expect(page.getByTestId('settings-school')).toContainText('Logo โรงเรียน');
	await page.locator('input[type=file]').setInputFiles(file);
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => api.count('upload')).toBe(1);
	await navigate(page, '/staff');
	await expect(page).toHaveURL(/\/staff$/);
	await expect(page.getByRole('heading', { name: 'ตั้งค่าโรงเรียน', exact: true })).toHaveCount(0);
	const uploaded = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/files'
	);
	api.release();
	await (await uploaded).finished();
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
	await expect(page.getByRole('heading', { name: 'ตั้งค่าโรงเรียน', exact: true })).toHaveCount(0);
	await expect.poll(() => api.writes.filter((write) => write.startsWith('PATCH')).length).toBe(0);
});
test('font refresh failure retains its list and delete patches without an extra read', async ({
	page
}) => {
	const api = await mockSettings(page, { fail: fontsPath, failAt: 2 });
	await page.goto('/staff/school-fonts');
	await expect(page.getByTestId('school-font-library')).toContainText('ฟอนต์ทดสอบ');
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('school-font-library')).toContainText('ส่วนนี้ไม่พร้อม');
	await expect(page.getByTestId('school-font-library')).toContainText('ฟอนต์ทดสอบ');
	await page.getByRole('button', { name: 'ลบฟอนต์ ฟอนต์ทดสอบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันลบฟอนต์', exact: true }).click();
	await expect(page.getByTestId('school-font-library')).toContainText('คลังฟอนต์ยังว่าง');
	expect(api.count(fontsPath)).toBe(2);
	expect(api.count('/api/files')).toBe(0);
});
