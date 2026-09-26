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

test('curriculum creation loads options on open and survives a stale initial overview', async ({
	page
}) => {
	const entry = routes[1];
	const curriculumId = '50000000-0000-4000-8000-000000000001';
	const gradeLevelId = '60000000-0000-4000-8000-000000000001';
	let writes = 0;
	const initialGate = deferred();
	const requests = await mockRoute(page, entry, initialGate.promise, false, async (route, url) => {
		if (url.pathname === '/api/academic/curricula/management-options') {
			await fulfill(route, {
				ownerOptions: [{ organizationUnitId: null, name: 'โรงเรียน', code: null }],
				gradeLevels: [
					{
						id: gradeLevelId,
						name: 'มัธยมศึกษาปีที่ 1',
						short_name: 'ม.1',
						code: 'M1',
						level_type: 'secondary',
						year: 1
					}
				]
			});
			return true;
		}
		if (url.pathname === '/api/academic/curricula' && route.request().method() === 'POST') {
			writes += 1;
			await fulfill(route, {
				id: curriculumId,
				code: 'CUR2570',
				nameTh: 'หลักสูตรทดสอบ',
				nameEn: null,
				gradeLevelIds: [gradeLevelId],
				owningOrganizationUnitId: null,
				rowVersion: 1
			});
			return true;
		}
		return false;
	});
	await page.goto(entry.path);
	try {
		await expect.poll(() => requests.primary).toBe(1);
		await expect(page.getByTestId(entry.readyTestId)).toHaveCount(0);
		expect(requests.optional).toBe(0);
		await page.getByRole('button', { name: 'เพิ่มหลักสูตร' }).click();
		await expect(page.getByRole('dialog')).toBeVisible();
		await expect.poll(() => requests.optional).toBe(1);
		await page.getByLabel('รหัสหลักสูตร').fill('CUR2570');
		await page.getByLabel('ชื่อหลักสูตรภาษาไทย').fill('หลักสูตรทดสอบ');
		await page.getByRole('combobox', { name: 'เลือกระดับชั้นของหลักสูตร' }).click();
		await page.getByRole('button', { name: 'เลือกทั้งหมด' }).click();
		await page.getByRole('button', { name: 'สร้างหลักสูตร' }).click();
		await expect(page.getByRole('link', { name: 'เปิดหลักสูตร หลักสูตรทดสอบ' })).toBeVisible();
	} finally {
		initialGate.release();
	}
	await expect(page.getByRole('link', { name: 'เปิดหลักสูตร หลักสูตรทดสอบ' })).toBeVisible();
	expect(writes).toBe(1);
	expect(requests.primary).toBe(1);
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
