import { test, expect } from '@playwright/test';
import { activityStart, mockSessionActivity } from './fixtures/session-activity';

test.use({ serviceWorkers: 'block', launchOptions: { args: ['--no-proxy-server'] } });

for (const role of ['staff', 'student', 'parent'] as const) {
	test(`${role}: trusted input renews centrally without refetching the preloaded timetable`, async ({
		context,
		page
	}) => {
		const api = await mockSessionActivity(context, role);
		await api.open(page);
		await expect(
			page.getByRole('heading', {
				name: role === 'staff' ? 'ยังไม่มีตารางสอน' : 'ยังไม่มีตารางเรียน',
				exact: true
			})
		).toBeVisible();
		const reads = api.requests.filter((r) => r.method === 'GET').map((r) => r.path);
		expect(api.count('/api/auth/me')).toBe(1);
		expect(api.count('/api/auth/activity')).toBe(0);
		api.setNow(300_100);
		await page.clock.runFor(300_000);
		expect(api.count('/api/auth/activity')).toBe(0);
		await page.keyboard.press('ArrowDown');
		await page.clock.runFor(10);
		await expect.poll(() => api.count('/api/auth/activity')).toBe(1);
		await page.clock.runFor(20);
		expect(api.requests.filter((r) => r.method === 'GET').map((r) => r.path)).toEqual(reads);
		await page.mouse.move(650, 350);
		await page.mouse.wheel(0, 200);
		await page.clock.runFor(10);
		expect(api.count('/api/auth/activity')).toBe(1);
		await expect(page.getByTestId('session-warning')).toHaveCount(0);
		expect(JSON.parse(api.requests.find((r) => r.path === '/api/auth/activity')!.body!)).toEqual({
			expectedSessionId: '01900000-0000-7000-8000-000000000001'
		});
	});
}

test('idle warning has a keyboard accessible Continue action using the same activity operation', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context, 'staff', {
		idleExpiresAt: new Date(activityStart + 180_000).toISOString(),
		nextRenewalAt: new Date(activityStart).toISOString()
	});
	await api.open(page);
	const warning = page.getByTestId('session-warning');
	await expect(warning).toContainText('3 นาที');
	const button = warning.getByRole('button', { name: 'ใช้งานต่อ', exact: true });
	await button.focus();
	await expect(button).toBeFocused();
	await page.keyboard.press('Enter');
	await page.clock.runFor(10);
	await expect.poll(() => api.count('/api/auth/activity')).toBe(1);
	await expect(warning).toHaveCount(0);
	expect(api.count('/api/auth/me')).toBe(1);
});

test('absolute warning explains that activity cannot extend the sign-in limit', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context, 'staff', {
		absoluteExpiresAt: new Date(activityStart + 180_000).toISOString()
	});
	await api.open(page);
	await expect(page.getByTestId('session-warning')).toContainText('เข้าสู่ระบบอีกครั้ง');
	await expect(page.getByTestId('session-warning')).toContainText('ไม่สามารถขยายเวลานี้ได้');
	await expect(page.getByTestId('session-warning').getByRole('button')).toHaveCount(0);
	expect(api.count('/api/auth/activity')).toBe(0);
});

for (const fallback of [false, true]) {
	test(`two tabs share metadata and coalesce renewal with ${fallback ? 'storage lease' : 'real Web Locks'}`, async ({
		context,
		page
	}) => {
		if (fallback)
			await context.addInitScript(() =>
				Object.defineProperty(navigator, 'locks', { value: undefined })
			);
		const api = await mockSessionActivity(context);
		await api.open(page);
		const other = await context.newPage();
		await api.open(other);
		api.setNow(300_100);
		await page.clock.runFor(300_000);
		await page.keyboard.press('ArrowDown');
		await other.mouse.move(650, 350);
		await page.clock.runFor(200);
		await expect.poll(() => api.count('/api/auth/activity')).toBe(1);
		await other.mouse.wheel(0, 100);
		await other.clock.runFor(200);
		expect(api.count('/api/auth/activity')).toBe(1);
		expect(api.count('/api/auth/me')).toBe(2);
	});
}

test('layout disposal cancels pending work and removes input listeners', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context);
	await api.open(page);
	await page.goto('/404');
	await page.clock.runFor(300_000);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(20);
	expect(api.count('/api/auth/activity')).toBe(0);
});

test('login in a new tab announces same-user session replacement to the existing tab', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context);
	await api.open(page);
	const other = await context.newPage();
	api.requireLogin(other);
	await other.goto('/login');
	await expect(other.getByLabel('ชื่อผู้ใช้งาน (Username)', { exact: true })).toBeVisible();
	await other.getByLabel('ชื่อผู้ใช้งาน (Username)', { exact: true }).fill('session-fixture');
	await other.getByLabel('รหัสผ่าน', { exact: true }).fill('synthetic-password');
	api.setNow(1000);
	await other.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
	await expect.poll(() => api.count('/api/auth/me', page)).toBe(2);
	await expect
		.poll(() =>
			page.evaluate(async () => {
				const modulePath = '/src/lib/stores/auth.ts';
				const { authStore }: typeof import('../../src/lib/stores/auth.ts') = await import(
					modulePath
				);
				let sessionId: string | undefined;
				const unsubscribe = authStore.subscribe((state) => {
					sessionId = state.session?.sessionId;
				});
				unsubscribe();
				return sessionId;
			})
		)
		.toBe('01900000-0000-7000-8000-000000000002');
});
