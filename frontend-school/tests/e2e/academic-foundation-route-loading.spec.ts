import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const routes = [
	{
		path: '/staff/academic/core',
		apiPath: '/api/academic/setup/workspace',
		readyTestId: 'academic-setup-ready',
		response: { years: [], terms: [], bellSchedules: [] },
		optionalPath: '/api/academic/bell-schedules/'
	},
	{
		path: '/staff/academic/curricula',
		apiPath: '/api/academic/curricula/overview',
		readyTestId: 'curricula-overview-ready',
		response: { items: [] },
		optionalPath: '/api/academic/curricula/management-options'
	}
] as const;

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function mockRoute(
	page: Page,
	entry: (typeof routes)[number],
	gate: Promise<void>,
	failFirst = false,
	interaction?: (route: Route, url: URL) => Promise<boolean>
) {
	const requests = { primary: 0, optional: 0 };
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: '90000000-0000-4000-8000-000000000001',
					username: 'foundation-reader',
					firstName: 'ทดสอบ',
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: ['*']
				});
				return;
			}
			if (url.pathname === entry.apiPath && route.request().method() === 'GET') {
				requests.primary += 1;
				await gate;
				if (failFirst && requests.primary === 1) {
					await route.fulfill({
						status: 503,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'ข้อมูลยังไม่พร้อม' })
					});
					return;
				}
				await fulfill(route, entry.response);
				return;
			}
			if (url.pathname.startsWith(entry.optionalPath)) requests.optional += 1;
			if (interaction && (await interaction(route, url))) return;
			if (url.pathname === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			await fulfill(route, {});
		}
	);
	return requests;
}

test('edition creation needs no owner or grade reads and survives a stale overview', async ({
	page
}) => {
	const entry = routes[1];
	let body: Record<string, unknown> | null = null;
	const gate = deferred();
	const requests = await mockRoute(page, entry, gate.promise, false, async (route, url) => {
		if (url.pathname === '/api/academic/curricula' && route.request().method() === 'POST') {
			body = route.request().postDataJSON();
			await fulfill(route, {
				...body,
				id: '50000000-0000-4000-8000-000000000001',
				status: 'draft',
				isActive: true,
				rowVersion: 1,
				publishedAt: null,
				createdAt: '2026-05-01T00:00:00Z',
				updatedAt: '2026-05-01T00:00:00Z'
			});
			return true;
		}
		return false;
	});
	await page.goto(entry.path);
	try {
		await expect.poll(() => requests.primary).toBe(1);
		await page.getByRole('button', { name: 'เพิ่มฉบับหลักสูตร', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await dialog.getByLabel('ปีปรับปรุง (พุทธศักราช) *').fill('2570');
		await expect(dialog.getByLabel('รหัสหลักสูตร')).toHaveCount(0);
		await expect(dialog.getByLabel('หน่วยงานเจ้าของหลักสูตร')).toHaveCount(0);
		await dialog.getByRole('button', { name: 'สร้างฉบับร่าง' }).click();
		await expect(
			page.getByRole('link', { name: 'ฉบับปรับปรุง พุทธศักราช 2570', exact: true })
		).toBeVisible();
	} finally {
		gate.release();
	}
	await expect(
		page.getByRole('link', { name: 'ฉบับปรับปรุง พุทธศักราช 2570', exact: true })
	).toBeVisible();
	expect(body).toEqual({
		name: 'ฉบับปรับปรุง พุทธศักราช 2570',
		revisionYear: 2570,
		description: null
	});
	expect(requests.primary).toBe(1);
	expect(requests.optional).toBe(0);
});

for (const entry of routes) {
	test(`${entry.path} shows a first-load skeleton and keeps optional data lazy`, async ({
		page
	}) => {
		const gate = deferred();
		const requests = await mockRoute(page, entry, gate.promise);
		try {
			await page.goto(entry.path);
			await expect.poll(() => requests.primary).toBe(1);
			await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
			await expect(page.getByTestId(entry.readyTestId)).toHaveCount(0);
			expect(requests.optional).toBe(0);
		} finally {
			gate.release();
		}
		await expect(page.getByTestId(entry.readyTestId)).toBeVisible();
		expect(requests.primary).toBe(1);
		expect(requests.optional).toBe(0);
	});

	test(`${entry.path} retries only its failed primary region`, async ({ page }) => {
		const requests = await mockRoute(page, entry, Promise.resolve(), true);
		await page.goto(entry.path);
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง' })).toBeVisible();
		expect(requests.primary).toBe(1);
		await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(page.getByTestId(entry.readyTestId)).toBeVisible();
		expect(requests.primary).toBe(2);
		expect(requests.optional).toBe(0);
	});
}
