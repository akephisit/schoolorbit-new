import { test, expect } from '@playwright/test';
import { year } from './fixtures/staff-home-route-data';
import {
	mockAccountConsent,
	currentSession,
	otherSession
} from './fixtures/account-consent-route-data';
test.use({ serviceWorkers: 'block' });
for (const [route, region] of [
	['/account/security', 'sessions'],
	['/settings/consent', 'consent']
] as const) {
	test(`${region}: tap preload starts primary before navigation and navigation reuses it`, async ({
		page
	}) => {
		const api = await mockAccountConsent(page);
		await page.goto(`/staff?academicYearId=${year}`);
		await expect(page.getByRole('heading', { name: 'หน้าหลักของฉัน', exact: true })).toBeVisible();
		await page.evaluate((url) => {
			const a = document.createElement('a');
			a.href = url;
			a.textContent = 'เปิดการตั้งค่า';
			a.dataset.sveltekitPreloadData = 'tap';
			document.querySelector('main')?.append(a);
		}, route);
		await page
			.getByRole('link', { name: 'เปิดการตั้งค่า', exact: true })
			.dispatchEvent('mousedown', { button: 0 });
		await expect.poll(() => api.count(region)).toBe(1);
		await expect(page).toHaveURL(/\/staff/);
		await page.getByRole('link', { name: 'เปิดการตั้งค่า', exact: true }).click();
		await expect(
			page.getByTestId(region === 'sessions' ? 'session-list' : 'consent-region')
		).toContainText(region === 'sessions' ? 'อุปกรณ์ปัจจุบัน' : 'ความยินยอมเสริม');
		expect(api.count(region)).toBe(1);
		expect(api.writes).toEqual([]);
	});
	test(`${region}: first read shows named local skeleton`, async ({ page }) => {
		const api = await mockAccountConsent(page, { hold: region });
		await page.goto(route);
		await expect(
			page.getByRole('status', {
				name: region === 'sessions' ? 'กำลังโหลดรายการอุปกรณ์' : 'กำลังโหลดความยินยอม',
				exact: true
			})
		).toBeVisible();
		if (region === 'sessions')
			await page.getByLabel('รหัสผ่านปัจจุบัน', { exact: true }).fill('synthetic-draft');
		api.release();
		await expect(
			page.getByTestId(region === 'sessions' ? 'session-list' : 'consent-region')
		).toContainText(region === 'sessions' ? 'อุปกรณ์ปัจจุบัน' : 'ความยินยอมเสริม');
	});
	test(`${region}: local retry does not refetch shared layout or optional catalog`, async ({
		page
	}) => {
		const api = await mockAccountConsent(page, { fail: region });
		await page.goto(route);
		if (region === 'sessions')
			await page.getByLabel('รหัสผ่านปัจจุบัน', { exact: true }).fill('synthetic-draft');
		await expect
			.poll(() => api.reads.filter((p) => p === '/api/menu/user' || p.endsWith('/counts')).length)
			.toBe(2);
		const sharedReads = api.reads.filter(
			(p) => p === '/api/menu/user' || p.endsWith('/counts')
		).length;
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(
			page.getByTestId(region === 'sessions' ? 'session-list' : 'consent-region')
		).toContainText(region === 'sessions' ? 'อุปกรณ์ปัจจุบัน' : 'ความยินยอมเสริม');
		if (region === 'sessions')
			await expect(page.getByLabel('รหัสผ่านปัจจุบัน', { exact: true })).toHaveValue(
				'synthetic-draft'
			);
		expect(api.count(region)).toBe(2);
		expect(api.reads.filter((p) => p === '/api/menu/user' || p.endsWith('/counts')).length).toBe(
			sharedReads
		);
		expect(api.reads).not.toContain('/api/consent/types');
		expect(api.writes).toEqual([]);
	});
	test(`${region}: failed refresh retains usable rows`, async ({ page }) => {
		const api = await mockAccountConsent(page, { fail: region, failAt: 2 });
		await page.goto(route);
		const ready = page.getByTestId(region === 'sessions' ? 'session-list' : 'consent-region');
		const label = region === 'sessions' ? 'อุปกรณ์ปัจจุบัน' : 'ความยินยอมเสริม';
		await expect(ready).toContainText(label);
		await page.getByRole('button', { name: 'โหลดข้อมูลใหม่', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeVisible();
		await expect(ready).toContainText(label);
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(ready).toContainText(label);
		expect(api.count(region)).toBe(3);
	});
}
test('session removal patches one row and a pending refresh cannot restore it', async ({
	page
}) => {
	const api = await mockAccountConsent(page, { hold: 'sessions', holdAt: 2 });
	await page.goto('/account/security');
	await expect(page.getByTestId(`session-row-${otherSession}`)).toBeVisible();
	await page.getByRole('button', { name: 'โหลดข้อมูลใหม่', exact: true }).click();
	await expect(
		page.getByRole('status', { name: 'กำลังอัปเดตรายการอุปกรณ์', exact: true })
	).toBeVisible();
	await page
		.getByTestId(`session-row-${otherSession}`)
		.getByRole('button', { name: 'นำอุปกรณ์ออก', exact: true })
		.click();
	await expect(page.getByTestId(`session-row-${otherSession}`)).toHaveCount(0);
	api.release();
	await api.completed;
	await page.evaluate(
		() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
	);
	await expect(page.getByTestId(`session-row-${otherSession}`)).toHaveCount(0);
	await expect(page.getByTestId(`session-row-${currentSession}`)).toBeVisible();
	expect(api.count('sessions')).toBe(2);
	expect(api.writes).toHaveLength(1);
});
test('password completion keeps current row without list reread', async ({ page }) => {
	const api = await mockAccountConsent(page);
	await page.goto('/account/security');
	await expect(page.getByTestId(`session-row-${otherSession}`)).toBeVisible();
	await page.getByLabel('รหัสผ่านปัจจุบัน', { exact: true }).fill('synthetic-current');
	await page.getByLabel('รหัสผ่านใหม่', { exact: true }).fill('synthetic-next');
	await page.getByLabel('ยืนยันรหัสผ่านใหม่', { exact: true }).fill('synthetic-next');
	await page.getByRole('button', { name: 'เปลี่ยนรหัสผ่าน', exact: true }).click();
	await expect(page.getByTestId(`session-row-${otherSession}`)).toHaveCount(0);
	await expect(page.getByTestId(`session-row-${currentSession}`)).toBeVisible();
	await expect(page.getByLabel('รหัสผ่านปัจจุบัน', { exact: true })).toHaveValue('');
	expect(api.count('sessions')).toBe(1);
	expect(api.writes).toHaveLength(1);
});
test('current session logout preserves confirmation and redirects', async ({ page }) => {
	const api = await mockAccountConsent(page);
	await page.goto('/account/security');
	await page.getByRole('button', { name: 'ออกจากระบบอุปกรณ์นี้', exact: true }).click();
	expect(api.writes).toHaveLength(0);
	await page
		.getByRole('alertdialog')
		.getByRole('button', { name: 'ออกจากระบบ', exact: true })
		.click();
	await expect(page).toHaveURL(/\/login/);
	expect(api.writes).toHaveLength(1);
});
test('consent withdrawal refreshes only its status and late refresh cannot undo it', async ({
	page
}) => {
	const api = await mockAccountConsent(page, { hold: 'consent', holdAt: 2 });
	await page.goto('/settings/consent');
	await expect(page.getByRole('button', { name: 'ถอนความยินยอม', exact: true })).toHaveCount(1);
	await page.getByRole('button', { name: 'โหลดข้อมูลใหม่', exact: true }).click();
	await expect(
		page.getByRole('status', { name: 'กำลังอัปเดตความยินยอม', exact: true })
	).toBeVisible();
	page.once('dialog', (dialog) => dialog.accept());
	await page.getByRole('button', { name: 'ถอนความยินยอม', exact: true }).click();
	await expect(page.getByText('ความยินยอมที่ถอนแล้ว', { exact: true })).toBeVisible();
	api.release();
	await api.completed;
	await page.evaluate(
		() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
	);
	await expect(page.getByRole('button', { name: 'ถอนความยินยอม', exact: true })).toHaveCount(0);
	expect(api.count('consent')).toBe(3);
	expect(api.writes).toHaveLength(1);
});
test('disposed consent action never starts a new status read', async ({ page }) => {
	const api = await mockAccountConsent(page, { hold: 'mutation' });
	await page.goto('/settings/consent');
	page.once('dialog', (dialog) => dialog.accept());
	await page.getByRole('button', { name: 'ถอนความยินยอม', exact: true }).click();
	await expect.poll(() => api.count('mutation')).toBe(1);
	await page.evaluate(() => {
		const a = document.createElement('a');
		a.href = '/staff';
		a.textContent = 'กลับหน้าแรก';
		document.querySelector('main')?.append(a);
	});
	await page.getByRole('link', { name: 'กลับหน้าแรก', exact: true }).click();
	await expect(page).toHaveURL(/\/staff/);
	api.release();
	await api.completed;
	await page.evaluate(
		() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
	);
	expect(api.count('consent')).toBe(1);
});
