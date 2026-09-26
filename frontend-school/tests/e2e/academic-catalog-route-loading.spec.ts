import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const routes = [
	{
		path: '/staff/academic/catalog/subject-groups',
		apiPath: '/api/academic/catalog/subject-groups',
		readyTestId: 'catalog-subject-groups-ready',
		response: []
	},
	{
		path: '/staff/academic/catalog/subjects',
		apiPath: '/api/academic/catalog/subjects/overview',
		readyTestId: 'catalog-subjects-ready',
		response: { items: [], gradeLevelOptions: [], subjectGroupOptions: [] }
	},
	{
		path: '/staff/academic/catalog/activities',
		apiPath: '/api/academic/catalog/activities/overview',
		readyTestId: 'catalog-activities-ready',
		response: { items: [], gradeLevelOptions: [], canCreate: false }
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

async function mockCatalog(
	page: Page,
	apiPath: string,
	response: unknown | (() => unknown),
	gate: Promise<void> | ((readNumber: number) => Promise<void>),
	mutate?: (route: Route, url: URL) => Promise<boolean>,
	failFirst = false
) {
	const requests = { primary: 0, history: 0, otherCatalogReads: 0 };
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: '90000000-0000-4000-8000-000000000001',
					username: 'catalog-reader',
					firstName: 'ทดสอบ',
					lastName: 'ทะเบียน',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: ['*']
				});
				return;
			}
			if (url.pathname === apiPath && route.request().method() === 'GET') {
				requests.primary += 1;
				await (typeof gate === 'function' ? gate(requests.primary) : gate);
				if (failFirst && requests.primary === 1) {
					await route.fulfill({
						status: 503,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'ทะเบียนไม่พร้อมชั่วคราว' })
					});
					return;
				}
				await fulfill(route, typeof response === 'function' ? response() : response);
				return;
			}
			if (mutate && (await mutate(route, url))) return;
			if (/^\/api\/academic\/catalog\//.test(url.pathname)) {
				if (url.pathname.endsWith('/versions')) requests.history += 1;
				else requests.otherCatalogReads += 1;
			}
			if (url.pathname === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			await fulfill(route, {});
		}
	);
	return requests;
}

test('subject-group creation patches its card without rereading the collection', async ({
	page
}) => {
	const group = {
		id: '30000000-0000-4000-8000-000000000001',
		code: 'MATH',
		nameTh: 'คณิตศาสตร์',
		nameEn: 'Mathematics',
		displayOrder: 1,
		isActive: true,
		rowVersion: 1
	};
	let writes = 0;
	const requests = await mockCatalog(
		page,
		'/api/academic/catalog/subject-groups',
		[],
		Promise.resolve(),
		async (route, url) => {
			if (url.pathname !== '/api/academic/catalog/subject-groups') return false;
			writes += 1;
			await fulfill(route, group);
			return true;
		}
	);
	await page.goto('/staff/academic/catalog/subject-groups');
	await expect(page.getByTestId('catalog-subject-groups-ready')).toBeVisible();
	await page.getByLabel('รหัส', { exact: true }).fill(group.code);
	await page.getByLabel('ชื่อภาษาไทย').fill(group.nameTh);
	await page.getByLabel('ชื่อภาษาอังกฤษ').fill(group.nameEn);
	await page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ' }).click();
	await expect(page.getByRole('heading', { name: group.nameTh })).toBeVisible();
	expect(writes).toBe(1);
	expect(requests.primary).toBe(1);
});

for (const entry of [
	{
		kind: 'subject',
		path: '/staff/academic/catalog/subjects',
		overviewPath: '/api/academic/catalog/subjects/overview',
		createPath: '/api/academic/catalog/subjects',
		codeLabel: 'เพิ่มรหัสรายวิชา',
		buttonLabel: 'เพิ่มรายวิชา',
		code: 'SCI101'
	},
	{
		kind: 'activity',
		path: '/staff/academic/catalog/activities',
		overviewPath: '/api/academic/catalog/activities/overview',
		createPath: '/api/academic/catalog/activities',
		codeLabel: 'เพิ่มรหัสกิจกรรม',
		buttonLabel: 'เพิ่มกิจกรรม',
		code: 'CLUB101'
	}
] as const) {
	test(`${entry.kind} creation refreshes only its overview and opens lazy history`, async ({
		page
	}) => {
		const id = '40000000-0000-4000-8000-000000000001';
		const groupId = '30000000-0000-4000-8000-000000000001';
		const record = {
			id,
			code: entry.code,
			subjectGroupId: groupId,
			activityType: 'club',
			archivedAt: null,
			rowVersion: 1
		};
		let created = false;
		let writes = 0;
		let historyReads = 0;
		const refreshGate = deferred();
		const overview = () => ({
			items: created
				? [
						{
							[entry.kind]: record,
							displayVersion: null,
							displayState: 'unpublished',
							draftCount: 0,
							gradeLevels: [],
							canManage: true
						}
					]
				: [],
			gradeLevelOptions: [],
			subjectGroupOptions:
				entry.kind === 'subject'
					? [{ subjectGroupId: groupId, name: 'วิทยาศาสตร์', canManage: true }]
					: undefined,
			canCreate: entry.kind === 'activity'
		});
		const requests = await mockCatalog(
			page,
			entry.overviewPath,
			overview,
			(readNumber) => (readNumber === 2 ? refreshGate.promise : Promise.resolve()),
			async (route, url) => {
				if (url.pathname === entry.createPath && route.request().method() === 'POST') {
					writes += 1;
					created = true;
					await fulfill(route, record);
					return true;
				}
				if (url.pathname === `${entry.createPath}/${id}/versions`) {
					historyReads += 1;
					await fulfill(route, []);
					return true;
				}
				return false;
			}
		);
		await page.goto(entry.path);
		await expect(
			page.getByTestId(`catalog-${entry.kind === 'subject' ? 'subjects' : 'activities'}-ready`)
		).toBeVisible();
		expect(historyReads).toBe(0);
		await page.getByLabel(entry.codeLabel).fill(entry.code);
		try {
			await page.getByRole('button', { name: entry.buttonLabel }).click();
			await expect.poll(() => requests.primary).toBe(2);
			await expect(
				page.getByTestId(`catalog-${entry.kind === 'subject' ? 'subjects' : 'activities'}-ready`)
			).toBeVisible();
			await expect(
				page.getByRole('status', {
					name: entry.kind === 'subject' ? 'กำลังอัปเดตทะเบียนรายวิชา' : 'กำลังอัปเดตทะเบียนกิจกรรม'
				})
			).toBeVisible();
			await expect(page.locator('main [data-slot="skeleton"]')).toHaveCount(0);
		} finally {
			refreshGate.release();
		}
		await expect(page.getByRole('dialog')).toBeVisible();
		await expect.poll(() => historyReads).toBe(1);
		expect(writes).toBe(1);
		expect(requests.otherCatalogReads).toBe(0);
	});
}

for (const route of routes) {
	test(`${route.path} retries only its failed primary region`, async ({ page }) => {
		const requests = await mockCatalog(
			page,
			route.apiPath,
			route.response,
			Promise.resolve(),
			undefined,
			true
		);
		await page.goto(route.path);
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง' })).toBeVisible();
		expect(requests.primary).toBe(1);
		await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(page.getByTestId(route.readyTestId)).toBeVisible();
		expect(requests.primary).toBe(2);
		expect(requests.history).toBe(0);
		expect(requests.otherCatalogReads).toBe(0);
	});

	test(`${route.path} renders its primary region without eager history reads`, async ({ page }) => {
		const gate = deferred();
		const requests = await mockCatalog(page, route.apiPath, route.response, gate.promise);
		try {
			await page.goto(route.path);
			await expect.poll(() => requests.primary).toBe(1);
			await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
			expect(requests.history).toBe(0);
			expect(requests.otherCatalogReads).toBe(0);
			await expect(page.getByTestId(route.readyTestId)).toHaveCount(0);
		} finally {
			gate.release();
		}
		await expect(page.getByTestId(route.readyTestId)).toBeVisible();
		expect(requests.primary).toBe(1);
		expect(requests.history).toBe(0);
		expect(requests.otherCatalogReads).toBe(0);
	});
}
