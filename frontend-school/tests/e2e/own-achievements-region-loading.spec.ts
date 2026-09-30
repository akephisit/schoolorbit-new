import { expect, test } from '@playwright/test';
import {
	mockOwnAchievements,
	achievementPath,
	ownCertificatePath,
	lookupPath
} from './fixtures/own-achievements-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});
const ownPath = '/staff/achievements/self-recorded';
test('own list is loader owned and no management or evidence workflows are read', async ({
	page
}) => {
	const api = await mockOwnAchievements(page, { hold: achievementPath });
	await page.goto(ownPath);
	await expect(page.getByRole('status', { name: 'กำลังโหลดผลงาน', exact: true })).toBeVisible();
	expect(api.count(lookupPath)).toBe(0);
	expect(api.count(ownCertificatePath)).toBe(0);
	api.release();
	await expect(page.getByText('ผลงานของฉัน', { exact: true })).toBeVisible();
	await expect(page.getByText('ผลงานบุคลากรอื่น', { exact: true })).toHaveCount(0);
	expect(api.count(achievementPath)).toBe(1);
});
test('failed own list retries without false empty or optional reads', async ({ page }) => {
	const api = await mockOwnAchievements(page, { fail: achievementPath });
	await page.goto(ownPath);
	await expect(page.getByText('ไม่พบข้อมูล', { exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'ลองโหลดผลงานอีกครั้ง', exact: true }).click();
	await expect(page.getByText('ผลงานของฉัน', { exact: true })).toBeVisible();
	expect(api.count(achievementPath)).toBe(2);
	expect(api.count(lookupPath)).toBe(0);
});
test('all tab is interaction lazy and late all response cannot replace returned own tab', async ({
	page
}) => {
	const api = await mockOwnAchievements(page, { hold: achievementPath, holdAt: 2 });
	await page.goto(ownPath);
	await expect(page.getByText('ผลงานของฉัน', { exact: true })).toBeVisible();
	expect(api.count(achievementPath)).toBe(1);
	await page.getByRole('tab', { name: 'ภาพรวม (ทั้งหมด)', exact: true }).click();
	await expect(page.getByRole('status', { name: 'กำลังโหลดผลงาน', exact: true })).toBeVisible();
	await page.getByRole('tab', { name: 'ของฉัน', exact: true }).click();
	await expect(page.getByText('ผลงานของฉัน', { exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('ผลงานบุคลากรอื่น', { exact: true })).toHaveCount(0);
	expect(api.count(achievementPath)).toBe(3);
});
test('owner selection is a bounded opened lookup and save patches own list', async ({ page }) => {
	const api = await mockOwnAchievements(page);
	await page.goto(ownPath);
	await page.getByRole('button', { name: 'เพิ่มรายการใหม่', exact: true }).first().click();
	expect(api.count(lookupPath)).toBe(0);
	await page.getByRole('dialog').getByRole('combobox').first().click();
	await expect(page.getByRole('option', { name: 'บัญชีของฉัน', exact: true })).toBeVisible();
	expect(api.reads.find((url) => url.pathname === lookupPath)?.searchParams.get('limit')).toBe(
		'50'
	);
	await page.getByRole('option', { name: 'บัญชีของฉัน', exact: true }).click();
	await page.getByLabel('ชื่อผลงาน / รางวัล', { exact: true }).fill('ผลงานสร้างใหม่');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText('ผลงานสร้างใหม่', { exact: true })).toBeVisible();
	expect(api.count(achievementPath)).toBe(1);
});
test('late closed update preserves a reopened achievement draft', async ({ page }) => {
	const api = await mockOwnAchievements(page, { hold: 'mutation' });
	await page.goto(ownPath);
	await page.getByRole('button', { name: 'แก้ไขผลงาน ผลงานของฉัน', exact: true }).click();
	await page.getByLabel('ชื่อผลงาน / รางวัล', { exact: true }).fill('ผลงานแก้ไข');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'แก้ไขผลงาน ผลงานของฉัน', exact: true }).click();
	await page.getByLabel('ชื่อผลงาน / รางวัล', { exact: true }).fill('ร่างใหม่');
	api.release();
	await expect(page.getByText('ผลงานแก้ไข', { exact: true })).toBeVisible();
	await expect(page.getByLabel('ชื่อผลงาน / รางวัล', { exact: true })).toHaveValue('ร่างใหม่');
	expect(api.count(achievementPath)).toBe(1);
});
test('empty achievement delete patches rows without a reread', async ({ page }) => {
	const api = await mockOwnAchievements(page);
	await page.goto(ownPath);
	await page.getByRole('button', { name: 'ลบผลงาน ผลงานของฉัน', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ลบข้อมูล', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText('ไม่พบข้อมูล', { exact: true })).toBeVisible();
	expect(api.count(achievementPath)).toBe(1);
});
for (const [role, path] of [
	['staff', '/staff/achievements/issued'],
	['student', '/student/certificates']
]) {
	test(`${role} own certificate primary read has skeleton and no unopened renderer`, async ({
		page
	}) => {
		const api = await mockOwnAchievements(page, {
			userType: role,
			permissions: ['certificate.read.own'],
			hold: ownCertificatePath
		});
		const assets: string[] = [];
		page.on('request', (request) => {
			if (/certificate.*renderer|pdfmake/.test(request.url())) assets.push(request.url());
		});
		await page.goto(path);
		await expect(
			page.getByRole('status', { name: 'กำลังโหลดคลังเกียรติบัตร', exact: true })
		).toBeVisible();
		api.release();
		await expect(page.getByTestId('my-certificate-card')).toContainText('เกียรติบัตรของบัญชี');
		expect(api.count(ownCertificatePath)).toBe(1);
		expect(api.count(achievementPath)).toBe(0);
		expect(api.writes).toHaveLength(0);
		expect(assets).toHaveLength(0);
	});
	test(`${role} retained certificate refresh failure keeps cards and retries only own list`, async ({
		page
	}) => {
		const api = await mockOwnAchievements(page, {
			userType: role,
			permissions: ['certificate.read.own'],
			fail: ownCertificatePath,
			failAt: 2
		});
		await page.goto(path);
		await expect(page.getByTestId('my-certificate-card')).toBeVisible();
		await page.getByRole('button', { name: 'โหลดใหม่', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ลองใหม่', exact: true })).toBeVisible();
		await expect(page.getByTestId('my-certificate-card')).toBeVisible();
		await page.getByRole('button', { name: 'ลองใหม่', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ลองใหม่', exact: true })).toHaveCount(0);
		expect(api.count(ownCertificatePath)).toBe(3);
	});
}
test('own-only achievement reader has no all tab or management requests', async ({ page }) => {
	const api = await mockOwnAchievements(page, { permissions: ['achievement.read.own'] });
	await page.goto(ownPath);
	await expect(page.getByText('ผลงานของฉัน', { exact: true })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'ภาพรวม (ทั้งหมด)', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เพิ่มรายการใหม่', exact: true })).toHaveCount(0);
	expect(api.count(lookupPath)).toBe(0);
	expect(api.writes).toHaveLength(0);
});
test('parent cannot start staff own certificate reads', async ({ page }) => {
	const api = await mockOwnAchievements(page, {
		userType: 'parent',
		permissions: ['certificate.read.own']
	});
	await page.goto('/staff/achievements/issued');
	await expect(page).toHaveURL(/\/403/);
	expect(api.count(ownCertificatePath)).toBe(0);
});
test('pending achievement read cannot restore a disposed route', async ({ page }) => {
	const api = await mockOwnAchievements(page, { hold: achievementPath });
	await page.goto(ownPath);
	await expect(page.getByRole('status', { name: 'กำลังโหลดผลงาน', exact: true })).toBeVisible();
	await navigate(page, '/staff');
	await expect(page).toHaveURL(/\/staff(?:\?|$)/);
	await expect(page.getByRole('status', { name: 'กำลังโหลดผลงาน', exact: true })).toHaveCount(0);
	api.release();
	await expect(page.getByText('ผลงานของฉัน', { exact: true })).toHaveCount(0);
});
