import { test, expect } from '@playwright/test';
import { mockSelfProfile, studentRoute, year } from './fixtures/self-profile-route-data';
test.use({ serviceWorkers: 'block' });
for (const route of ['/student', '/student/profile']) {
	test(`${route}: delayed primary has first skeleton`, async ({ page }) => {
		const api = await mockSelfProfile(page, { hold: 'profile' });
		await page.goto(studentRoute(route));
		await expect(
			page.getByRole('status', { name: 'กำลังโหลดข้อมูลนักเรียน', exact: true })
		).toBeVisible();
		await expect.poll(() => api.count('context')).toBe(1);
		await expect.poll(() => api.count('profile')).toBe(1);
		expect(api.writes).toEqual([]);
		api.release();
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toBeVisible();
	});
	test(`${route}: missing year repairs URL without duplicate profile or identity GET`, async ({
		page
	}) => {
		const api = await mockSelfProfile(page);
		await page.goto(route);
		await expect(page).toHaveURL(new RegExp(`academicYearId=${year}`));
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toBeVisible();
		expect(api.count('context')).toBe(1);
		expect(api.count('profile')).toBe(1);
		expect(api.identityCount()).toBe(1);
	});
	test(`${route}: focused profile retry never reloads scoped options`, async ({ page }) => {
		const api = await mockSelfProfile(page, { fail: 'profile' });
		await page.goto(studentRoute(route));
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toBeVisible();
		expect(api.count('profile')).toBe(2);
		expect(api.count('context')).toBe(1);
	});
	test(`${route}: retained refresh failure keeps profile visible`, async ({ page }) => {
		const api = await mockSelfProfile(page, { fail: 'profile', failAt: 2 });
		await page.goto(studentRoute(route));
		await page.getByRole('button', { name: 'โหลดข้อมูลใหม่', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeVisible();
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toBeVisible();
		expect(api.count('context')).toBe(1);
	});
	test(`${route}: late old year cannot replace new year and Back restores selection`, async ({
		page
	}) => {
		const api = await mockSelfProfile(page, { hold: 'profile' });
		const oldResponse = page.waitForResponse((response) => {
			const url = new URL(response.url());
			return (
				url.pathname === '/api/student/profile' && url.searchParams.get('academicYearId') === year
			);
		});
		await page.goto(studentRoute(route));
		await page.getByRole('button', { name: 'ปีการศึกษา', exact: true }).click();
		await page.getByRole('option', { name: '2570', exact: true }).click();
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีถัดไป/)).toBeVisible();
		api.release();
		await oldResponse;
		await page.evaluate(
			() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
		);
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toHaveCount(0);
		await page.goBack();
		await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toBeVisible();
	});
}
test('context failure blocks dependent profile and retry restores it once', async ({ page }) => {
	const api = await mockSelfProfile(page, { fail: 'context' });
	await page.goto('/student');
	await expect(
		page.getByText('ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้', { exact: true })
	).toHaveCount(0);
	expect(api.count('profile')).toBe(0);
	await page.getByRole('button', { name: 'ลองบริบทอีกครั้ง', exact: true }).click();
	await expect(page.getByText(/ม\.1\s*\/\s*ห้องปีเดิม/)).toBeVisible();
	expect(api.count('context')).toBe(2);
	expect(api.count('profile')).toBe(1);
});
test('empty authorized history never requests self profile', async ({ page }) => {
	const api = await mockSelfProfile(page, { empty: true });
	await page.goto('/student');
	await expect(
		page.getByText('ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้', { exact: true })
	).toBeVisible();
	expect(api.count('profile')).toBe(0);
});
test('failed save preserves draft; successful empty save refreshes profile only', async ({
	page
}) => {
	const api = await mockSelfProfile(page, { fail: 'save' });
	await page.goto(studentRoute('/student/profile'));
	await page.getByRole('button', { name: 'แก้ไขข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อเล่น', { exact: true }).fill('draft ใหม่');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByLabel('ชื่อเล่น', { exact: true })).toHaveValue('draft ใหม่');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByText('draft ใหม่', { exact: true })).toBeVisible();
	expect(api.count('context')).toBe(1);
	expect(api.count('profile')).toBe(2);
	expect(api.count('save')).toBe(2);
});

test('logout while scoped history is pending stops dependent profile GET', async ({ page }) => {
	const api = await mockSelfProfile(page, { hold: 'context' });
	const pending = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/me/academic-context/options'
	);
	await page.goto('/student');
	await page.getByRole('banner').getByRole('button', { name: 'เโ', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ออกจากระบบ', exact: true }).click();
	await expect(page).toHaveURL(/\/login/);
	api.release();
	await pending;
	await page.evaluate(
		() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
	);
	expect(api.count('profile')).toBe(0);
});
