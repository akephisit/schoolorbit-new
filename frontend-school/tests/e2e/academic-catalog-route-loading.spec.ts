import { expect, test, type Page, type Route } from '@playwright/test';
import type { CatalogActivityOverview } from '../../src/lib/api/academic-core.js';

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
					displayName: 'ทดสอบ ทะเบียน',
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
	await page.goto('/staff/academic/catalog/subject-groups', { waitUntil: 'domcontentloaded' });
	await expect(page.getByTestId('catalog-subject-groups-ready')).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ', exact: true }).click();
	await expect(page.getByLabel('รหัส', { exact: true })).toHaveCount(0);
	await page.getByLabel('ชื่อกลุ่มสาระ').fill(group.nameTh);
	await page.getByLabel('ชื่อภาษาอังกฤษ').fill(group.nameEn);
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
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
	test(`${entry.kind} creation reconciles its overview and opens lazy history`, async ({
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
					if (entry.kind === 'activity')
						expect(route.request().postDataJSON()).toEqual({ activityType: 'guidance' });
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
		await page.goto(entry.path, { waitUntil: 'domcontentloaded' });
		await expect(
			page.getByTestId(`catalog-${entry.kind === 'subject' ? 'subjects' : 'activities'}-ready`)
		).toBeVisible();
		expect(historyReads).toBe(0);
		if (entry.kind === 'subject') await page.getByLabel(entry.codeLabel).fill(entry.code);
		else {
			await page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true }).click();
			await expect(page.getByLabel('เพิ่มรหัสกิจกรรม')).toHaveCount(0);
		}
		try {
			await page
				.getByRole('button', {
					name: entry.kind === 'activity' ? 'เพิ่มและกำหนดรายละเอียด' : entry.buttonLabel
				})
				.click();
			if (entry.kind === 'activity') {
				await expect(
					page.getByRole('dialog', { name: 'เพิ่มกิจกรรม', exact: true })
				).not.toBeVisible();
				await expect(page.getByRole('dialog')).toBeVisible();
				await expect.poll(() => historyReads).toBe(1);
				expect(requests.primary).toBe(1);
				expect(writes).toBe(1);
				expect(requests.otherCatalogReads).toBe(0);
				await expect(page.getByRole('dialog')).not.toContainText(entry.code);
				return;
			}
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
	test(`${route.path} retries only its failed primary region`, async ({ page }, testInfo) => {
		const requests = await mockCatalog(
			page,
			route.apiPath,
			route.response,
			Promise.resolve(),
			undefined,
			true
		);
		await page.goto(route.path, { waitUntil: 'domcontentloaded' });
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง' })).toBeVisible();
		if (route.path === activityPath)
			await page.screenshot({
				path: testInfo.outputPath('activities-error.png'),
				animations: 'disabled'
			});
		expect(requests.primary).toBe(1);
		await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(page.getByTestId(route.readyTestId)).toBeVisible();
		if (route.path === activityPath)
			await page.screenshot({
				path: testInfo.outputPath('activities-empty.png'),
				animations: 'disabled'
			});
		expect(requests.primary).toBe(2);
		expect(requests.history).toBe(0);
		expect(requests.otherCatalogReads).toBe(0);
	});

	test(`${route.path} renders its primary region without eager history reads`, async ({
		page
	}, testInfo) => {
		const gate = deferred();
		const requests = await mockCatalog(page, route.apiPath, route.response, gate.promise);
		try {
			await page.goto(route.path, { waitUntil: 'domcontentloaded' });
			await expect.poll(() => requests.primary).toBe(1);
			await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
			expect(requests.history).toBe(0);
			expect(requests.otherCatalogReads).toBe(0);
			await expect(page.getByTestId(route.readyTestId)).toHaveCount(0);
			if (route.path === activityPath)
				await page.screenshot({
					path: testInfo.outputPath('activities-loading.png'),
					animations: 'disabled'
				});
		} finally {
			gate.release();
		}
		await expect(page.getByTestId(route.readyTestId)).toBeVisible();
		if (route.path === activityPath)
			await page.screenshot({
				path: testInfo.outputPath('activities-empty.png'),
				animations: 'disabled'
			});
		expect(requests.primary).toBe(1);
		expect(requests.history).toBe(0);
		expect(requests.otherCatalogReads).toBe(0);
	});
}

const activityPath = '/staff/academic/catalog/activities';
const activityOverviewPath = '/api/academic/catalog/activities/overview';
const activityId = '40000000-0000-4000-8000-000000000002';
const activityCode = 'HIDDEN-INTERNAL-ACTIVITY-CODE';
const namedActivityOverview = {
	items: [
		{
			activity: {
				id: activityId,
				code: activityCode,
				activityType: 'club',
				archivedAt: null,
				rowVersion: 1,
				owningOrganizationUnitId: '30000000-0000-4000-8000-000000000002',
				createdAt: '2026-05-01T00:00:00Z',
				updatedAt: '2026-05-01T00:00:00Z'
			},
			displayVersion: {
				id: '50000000-0000-4000-8000-000000000002',
				activityId,
				versionNo: 1,
				name: 'ชุมนุมวิทยาศาสตร์และสิ่งแวดล้อมสำหรับนักเรียน',
				description: 'กิจกรรมทดลองและเรียนรู้ร่วมกัน',
				hoursPerWeek: '1.50',
				hoursPerTerm: '30.00',
				schedulingMode: 'synchronized',
				effectiveFrom: '2026-05-01',
				effectiveUntil: null,
				gradeLevelIds: [],
				status: 'published',
				rowVersion: 1,
				migrated: false,
				termCode: null,
				publishedAt: '2026-05-01T00:00:00Z',
				createdAt: '2026-05-01T00:00:00Z',
				updatedAt: '2026-05-01T00:00:00Z'
			},
			displayState: 'current',
			draftCount: 0,
			gradeLevels: [],
			canManage: true
		}
	],
	gradeLevelOptions: [],
	canCreate: true
} satisfies CatalogActivityOverview;

for (const width of [375, 1440]) {
	test(`activity names, compact filters and code-free dialogs at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 1000 });
		let historyReads = 0;
		await mockCatalog(
			page,
			activityOverviewPath,
			namedActivityOverview,
			Promise.resolve(),
			async (route, url) => {
				if (url.pathname.endsWith(`/${activityId}/versions`)) {
					historyReads++;
					await fulfill(route, [namedActivityOverview.items[0].displayVersion]);
					return true;
				}
				return false;
			}
		);
		await page.goto(activityPath, { waitUntil: 'domcontentloaded' });
		const ready = page.getByTestId('catalog-activities-ready');
		await expect(ready).toContainText(namedActivityOverview.items[0].displayVersion.name);
		await expect(page.locator('body')).not.toContainText(activityCode);
		expect(historyReads).toBe(0);
		await expect(page.getByLabel('กรองรูปแบบการจัด', { exact: true })).toHaveCount(0);
		for (const theme of ['light', 'dark']) {
			if (theme === 'dark') await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
				width
			);
			await page.screenshot({
				path: testInfo.outputPath(`activities-${width}-${theme}.png`),
				fullPage: true,
				animations: 'disabled'
			});
		}
		await page.getByRole('button', { name: 'ตัวกรองเพิ่มเติม' }).click();
		await expect(page.getByLabel('กรองรูปแบบการจัด', { exact: true })).toBeVisible();
		await page.getByLabel('ค้นหากิจกรรม', { exact: true }).fill('ไม่มีรายการที่ตรงกัน');
		await expect(page.getByText('ไม่พบกิจกรรมที่ตรงกับตัวกรอง')).toBeVisible();
		await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).first().click();
		await expect(ready).toContainText(namedActivityOverview.items[0].displayVersion.name);
		const open =
			width < 768
				? ready.getByRole('button', { name: /ชุมนุมวิทยาศาสตร์/ })
				: page.getByRole('button', { name: /เปิดรายละเอียด ชุมนุมวิทยาศาสตร์/ });
		const bounds = await open.boundingBox();
		expect(bounds!.width).toBeGreaterThan(0);
		if (width >= 768) expect(bounds!.height).toBeLessThan(44);
		await open.focus();
		await open.press('Enter');
		await expect(page.getByRole('dialog')).toContainText('ประวัติรุ่นกิจกรรม');
		await expect(page.getByRole('dialog')).not.toContainText(activityCode);
		await page.screenshot({
			path: testInfo.outputPath(`activity-details-${width}.png`),
			fullPage: true,
			animations: 'disabled'
		});
		await page.getByRole('dialog').getByLabel('ชื่อภาษาไทย').fill('กิจกรรมทดสอบวันที่');
		await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่' }).click();
		await expect(page.getByRole('dialog')).toContainText('กรุณาเลือกวันที่เริ่มใช้');
		await page.screenshot({
			path: testInfo.outputPath(`activity-validation-${width}.png`),
			animations: 'disabled'
		});
		await page.getByRole('dialog').press('Escape');
		await page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true }).click();
		await expect(page.getByRole('dialog')).toContainText('ประเภทกิจกรรม');
		await expect(page.getByLabel('เพิ่มรหัสกิจกรรม')).toHaveCount(0);
		await page.screenshot({
			path: testInfo.outputPath(`activity-create-${width}.png`),
			fullPage: true,
			animations: 'disabled'
		});
	});
}

test('activity create failure retains the selection and blocks duplicate pending writes', async ({
	page
}, testInfo) => {
	const gate = deferred();
	let writes = 0;
	await mockCatalog(
		page,
		activityOverviewPath,
		{ items: [], gradeLevelOptions: [], canCreate: true },
		Promise.resolve(),
		async (route, url) => {
			if (
				url.pathname === '/api/academic/catalog/activities' &&
				route.request().method() === 'POST'
			) {
				writes++;
				expect(route.request().postDataJSON()).toEqual({ activityType: 'club' });
				await gate.promise;
				await route.fulfill({
					status: 503,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'บันทึกไม่สำเร็จ กรุณาลองอีกครั้ง' })
				});
				return true;
			}
			return false;
		}
	);
	await page.goto(activityPath, { waitUntil: 'domcontentloaded' });
	await page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true }).click();
	await page
		.getByRole('dialog')
		.getByLabel(/^ประเภทกิจกรรม/)
		.click();
	await page.getByRole('option', { name: 'ชุมนุม', exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด' }).click();
	try {
		await expect(page.getByRole('button', { name: 'กำลังเพิ่มกิจกรรม...' })).toBeDisabled();
		await expect(page.getByRole('button', { name: 'ยกเลิก' })).toBeDisabled();
		expect(writes).toBe(1);
		await page.screenshot({
			path: testInfo.outputPath('activity-pending.png'),
			animations: 'disabled'
		});
	} finally {
		gate.release();
	}
	await expect(page.getByRole('alert')).toContainText('บันทึกไม่สำเร็จ');
	await page.screenshot({
		path: testInfo.outputPath('activity-save-error.png'),
		animations: 'disabled'
	});
	await expect(page.getByRole('dialog').getByLabel(/^ประเภทกิจกรรม/)).toContainText('ชุมนุม');
	await expect(page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด' })).toBeEnabled();
});

test('activity readers can inspect history without create or version actions', async ({ page }) => {
	await mockCatalog(
		page,
		activityOverviewPath,
		{
			...namedActivityOverview,
			canCreate: false,
			items: namedActivityOverview.items.map((item) => ({ ...item, canManage: false }))
		},
		Promise.resolve(),
		async (route, url) => {
			if (url.pathname.endsWith(`/${activityId}/versions`)) {
				await fulfill(route, [namedActivityOverview.items[0].displayVersion]);
				return true;
			}
			return false;
		}
	);
	await page.goto(activityPath, { waitUntil: 'domcontentloaded' });
	await expect(page.getByTestId('catalog-activities-ready')).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: /เปิดรายละเอียด ชุมนุมวิทยาศาสตร์/ }).click();
	await expect(page.getByRole('dialog')).toContainText('ประวัติรุ่นกิจกรรม');
	await expect(page.getByRole('button', { name: 'สร้างรุ่นใหม่', exact: true })).toHaveCount(0);
	await expect(page.getByRole('dialog').getByLabel('ชื่อภาษาไทย')).toHaveCount(0);
});

test('activity history failure retries lazily without reloading the overview', async ({ page }) => {
	let historyReads = 0;
	const requests = await mockCatalog(
		page,
		activityOverviewPath,
		namedActivityOverview,
		Promise.resolve(),
		async (route, url) => {
			if (url.pathname.endsWith(`/${activityId}/versions`)) {
				historyReads++;
				if (historyReads === 1)
					await route.fulfill({
						status: 503,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'ประวัติไม่พร้อมชั่วคราว' })
					});
				else await fulfill(route, [namedActivityOverview.items[0].displayVersion]);
				return true;
			}
			return false;
		}
	);
	await page.goto(activityPath, { waitUntil: 'domcontentloaded' });
	await page.getByRole('button', { name: /เปิดรายละเอียด ชุมนุมวิทยาศาสตร์/ }).click();
	await expect(page.getByRole('dialog')).toContainText('โหลดประวัติไม่สำเร็จ');
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByRole('dialog')).toContainText('ประวัติรุ่นกิจกรรม');
	expect(historyReads).toBe(2);
	expect(requests.primary).toBe(1);
});
