import { test, expect } from '@playwright/test';
import { activityStart, mockSessionActivity } from './fixtures/session-activity';

test.use({ serviceWorkers: 'block', launchOptions: { args: ['--no-proxy-server'] } });

test('native timers renew on trusted input, reschedule and dispose without page errors or extra reads', async ({
	context,
	page
}) => {
	const pageErrors: string[] = [];
	page.on('pageerror', (error) => pageErrors.push(error.message));
	const api = await mockSessionActivity(
		context,
		'staff',
		{ nextRenewalAt: new Date(activityStart).toISOString() },
		{ nativeTimers: true, renewalIntervalMs: 2_000 }
	);
	await api.open(page);
	const reads = api.requests
		.filter((request) => request.method === 'GET')
		.map((request) => request.path);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/auth/activity')).toBe(0);
	await page.keyboard.press('ArrowDown');
	expect(pageErrors).toEqual([]);
	await expect.poll(() => api.activityWrites).toBe(1);
	// Fresh input cancels and replaces the native timer until the returned renewal deadline.
	await page.keyboard.press('ArrowUp');
	await page.mouse.move(650, 350);
	await page.mouse.wheel(0, 100);
	await expect.poll(() => api.activityWrites).toBe(2);
	expect(api.count('/api/auth/activity')).toBe(2);
	expect(
		api.requests.filter((request) => request.method === 'GET').map((request) => request.path)
	).toEqual(reads);
	expect(pageErrors).toEqual([]);
	// Leave the authenticated layout in the same document while another renewal is scheduled.
	await page.keyboard.press('ArrowDown');
	await page.evaluate(() => {
		document.documentElement.dataset.sessionActivityNative = 'retained';
		const link = document.createElement('a');
		link.href = '/privacy-policy';
		link.textContent = 'Leave session activity fixture';
		document.body.append(link);
	});
	await page.getByRole('link', { name: 'Leave session activity fixture', exact: true }).focus();
	await page.keyboard.press('Enter');
	await expect(page).toHaveURL(/\/privacy-policy$/);
	expect(await page.evaluate(() => document.documentElement.dataset.sessionActivityNative)).toBe(
		'retained'
	);
	// Let the scheduled deadline pass on the real browser clock to expose leaked callbacks.
	await page.waitForTimeout(2_200);
	await page.keyboard.press('ArrowDown');
	expect(api.count('/api/auth/activity')).toBe(2);
	expect(pageErrors).toEqual([]);
});

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
		await expect.poll(() => api.activityWrites).toBe(1);
		const attempts = api.count('/api/auth/activity');
		expect(attempts).toBeGreaterThanOrEqual(1);
		expect(attempts).toBeLessThanOrEqual(fallback ? 2 : 1);
		await other.mouse.wheel(0, 100);
		await other.clock.runFor(200);
		expect(api.count('/api/auth/activity')).toBe(attempts);
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
	// The notification owner can also validate identity when the other tab first sees a 401.
	// Require this tab to observe the replacement, independent of those existing validations.
	await expect
		.poll(
			() =>
				api.requests.filter(
					(request) =>
						request.path === '/api/auth/me' &&
						request.page === page &&
						request.sessionId === '01900000-0000-7000-8000-000000000002'
				).length
		)
		.toBeGreaterThanOrEqual(1);
	await expect(page.getByRole('heading', { name: 'ยังไม่มีตารางสอน', exact: true })).toBeVisible();
	api.setNow(301_100);
	await page.clock.runFor(301_000);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(20);
	await expect.poll(() => api.count('/api/auth/activity', page)).toBe(1);
	const activity = api.requests.find(
		(request) => request.path === '/api/auth/activity' && request.page === page
	);
	expect(JSON.parse(activity!.body!)).toEqual({
		expectedSessionId: '01900000-0000-7000-8000-000000000002'
	});
});

test('503 retains the timetable and retries after Retry-After without another current-user read', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context, 'staff', {
		idleExpiresAt: new Date(activityStart + 180_000).toISOString(),
		nextRenewalAt: new Date(activityStart).toISOString()
	});
	api.status(503);
	await api.open(page);
	await page.getByRole('button', { name: 'ใช้งานต่อ', exact: true }).click();
	await page.clock.runFor(10);
	await expect(page.getByTestId('session-warning')).toContainText('ยังต่อเวลาการใช้งานไม่ได้');
	await expect(page.getByRole('heading', { name: 'ยังไม่มีตารางสอน', exact: true })).toBeVisible();
	await page.clock.runFor(19_000);
	expect(api.count('/api/auth/activity')).toBe(1);
	api.status(200);
	api.setNow(20_100);
	await page.clock.runFor(1_100);
	await expect.poll(() => api.count('/api/auth/activity')).toBe(2);
	await expect(page.getByTestId('session-warning')).toHaveCount(0);
	expect(api.count('/api/auth/me')).toBe(1);
});

test('403 refreshes identity and CSRF once without replaying the activity or a business mutation', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context, 'staff', {
		nextRenewalAt: new Date(activityStart).toISOString()
	});
	api.status(403);
	await api.open(page);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(10);
	await expect.poll(() => api.count('/api/auth/me')).toBe(2);
	await page.mouse.move(600, 400);
	await page.clock.runFor(1_000);
	expect(api.count('/api/auth/activity')).toBe(1);
	expect(
		api.requests.filter((request) => request.method !== 'GET').map((request) => request.path)
	).toEqual(['/api/auth/activity']);
	await expect(page.getByRole('heading', { name: 'ยังไม่มีตารางสอน', exact: true })).toBeVisible();
});

test('an actual activity 401 follows the existing sign-in redirect', async ({ context, page }) => {
	const api = await mockSessionActivity(context, 'staff', {
		nextRenewalAt: new Date(activityStart).toISOString()
	});
	await api.open(page);
	api.status(401);
	api.requireLogin(page);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(20);
	await expect(page).toHaveURL(/\/login\/?$/);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	expect(api.count('/api/auth/activity')).toBe(1);
});

test('leaving the app while renewal is pending aborts the request and ignores late completion', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context, 'staff', {
		nextRenewalAt: new Date(activityStart).toISOString()
	});
	await api.open(page);
	api.hold();
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(10);
	await expect.poll(() => api.count('/api/auth/activity')).toBe(1);
	await page.goto('/404');
	api.release();
	await page.clock.runFor(30_000);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(20);
	expect(api.count('/api/auth/activity')).toBe(1);
	await expect(page.getByTestId('session-warning')).toHaveCount(0);
	await expect(page).toHaveURL(/\/404$/);
});

test('a suspended clock validates actual expiry before fresh input can POST activity', async ({
	context,
	page
}) => {
	const api = await mockSessionActivity(context);
	await api.open(page);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(10);
	expect(api.count('/api/auth/activity')).toBe(0);
	api.setNow(3 * 3_600_000);
	await page.clock.setSystemTime(activityStart + 3 * 3_600_000);
	await page.keyboard.press('ArrowDown');
	await page.clock.runFor(20);
	await expect(page).toHaveURL(/\/login\/?$/);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	expect(api.count('/api/auth/activity')).toBe(0);
	expect(api.count('/api/auth/me')).toBeGreaterThanOrEqual(2);
});

test.describe('warning layout and touch access', () => {
	test.use({ hasTouch: true });
	for (const width of [375, 1280]) {
		for (const theme of ['light', 'dark'] as const) {
			test(`${width}px ${theme} warning wraps and its action remains reachable`, async ({
				context,
				page
			}) => {
				await page.setViewportSize({ width, height: 850 });
				await page.emulateMedia({ colorScheme: theme });
				const api = await mockSessionActivity(context, 'staff', {
					idleExpiresAt: new Date(activityStart + 180_000).toISOString(),
					nextRenewalAt: new Date(activityStart).toISOString()
				});
				await api.open(page);
				const warning = page.getByTestId('session-warning');
				await expect(warning).toBeVisible();
				const box = await warning.boundingBox();
				expect(box).not.toBeNull();
				expect(box!.x).toBeGreaterThanOrEqual(0);
				expect(box!.x + box!.width).toBeLessThanOrEqual(width);
				const action = warning.getByRole('button', { name: 'ใช้งานต่อ', exact: true });
				await action.focus();
				await expect(action).toBeFocused();
				const actionBox = await action.boundingBox();
				expect(actionBox!.y + actionBox!.height).toBeLessThanOrEqual(box!.y + box!.height);
				await page.screenshot({
					path: test.info().outputPath('session-warning.png'),
					animations: 'disabled'
				});
				api.hold();
				await action.tap();
				await page.clock.runFor(10);
				await expect(
					warning.getByRole('button', { name: 'กำลังต่อเวลา...', exact: true })
				).toBeDisabled();
				api.release();
				await expect(warning).toHaveCount(0);
			});
		}
	}
});
