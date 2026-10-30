import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';
import type { AcademicTermChangeSet } from '../../src/lib/api/learning-delivery.js';

test.use({ serviceWorkers: 'block' });

const ids = {
	year: '10000000-0000-4000-8000-000000000201',
	term: '20000000-0000-4000-8000-000000000201',
	offering: '30000000-0000-4000-8000-000000000201',
	groupA: '40000000-0000-4000-8000-000000000201',
	groupB: '40000000-0000-4000-8000-000000000202',
	version: '50000000-0000-4000-8000-000000000201'
};

const paths = {
	offering: `/api/academic/offerings/${ids.offering}`,
	groups: `/api/academic/offerings/${ids.offering}/groups`,
	versions: '/api/academic/delivery-versions',
	version: `/api/academic/delivery-versions/${ids.version}`,
	groupA: `/api/academic/learning-groups/${ids.groupA}`,
	groupB: `/api/academic/learning-groups/${ids.groupB}`,
	membershipsA: `/api/academic/learning-groups/${ids.groupA}/memberships`,
	management: '/api/academic/delivery/management-options'
};

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

function offering() {
	return {
		id: ids.offering,
		academicYearId: ids.year,
		academicTermId: ids.term,
		kind: 'course',
		codeSnapshot: 'ค21101',
		nameSnapshot: 'คณิตศาสตร์พื้นฐาน',
		status: 'published',
		rowVersion: 1,
		startsOn: '2026-05-01',
		endsOn: null,
		stopReason: null,
		targets: [],
		snapshot: {
			kind: 'course',
			subjectId: ids.offering,
			subjectVersionId: ids.offering,
			credit: '1.0',
			hours: '40',
			standardPeriodsPerWeek: 2,
			curriculumCourseRequirementId: null,
			assessmentTotalScore: '100.00'
		}
	};
}

function group(id: string, published = false) {
	return {
		id,
		learningOfferingId: ids.offering,
		academicTermId: ids.term,
		academicYearId: ids.year,
		code: id === ids.groupA ? 'A' : 'B',
		name: id === ids.groupA ? 'กลุ่ม ก' : 'กลุ่ม ข',
		description: null,
		capacity: 40,
		status: published ? 'published' : 'draft',
		rosterStatus: published ? 'published' : 'draft',
		rosterPublishedAt: published ? '2026-05-01T00:00:00Z' : null,
		teachersLocked: published,
		teacherAssignments: [],
		homeroomIds: [ids.year],
		preferredRoomIds: [],
		rowVersion: 1
	};
}

function version(published = false) {
	return {
		id: ids.version,
		academicTermId: ids.term,
		academicYearId: ids.year,
		status: published ? 'published' : 'draft',
		sourceVersionId: null,
		changeSetId: null,
		effectiveFrom: '2026-05-01',
		effectiveUntil: null,
		rowVersion: 1,
		offeringCount: 1,
		groupCount: 2,
		teacherAssignmentCount: 0,
		updatedAt: '2026-05-01T00:00:00Z'
	};
}
function versionGraph(published = false) {
	const catalog = offering().snapshot;
	return {
		...version(published),
		createdBy: null,
		publishedBy: null,
		publishedAt: null,
		createdAt: '2026-05-01T00:00:00Z',
		courseMetadata: [],
		snapshot: {
			offerings: [
				{
					id: ids.offering,
					kind: 'course',
					code: 'ค21101',
					name: 'คณิตศาสตร์พื้นฐาน',
					owningOrganizationUnitId: ids.offering,
					sourceRequirementId: null,
					sourceRequirementKind: null,
					weeklyPeriodTarget: 2,
					catalog,
					targets: [],
					homeroomIds: [],
					groups: [group(ids.groupA, published), group(ids.groupB)].map((row) => ({
						id: row.id,
						code: row.code,
						name: row.name,
						description: null,
						capacity: 40,
						homeroomIds: [],
						preferredRoomIds: [],
						teachers: []
					}))
				}
			]
		}
	};
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-test-csrf' },
		body: JSON.stringify({ success: true, data })
	});
}

async function mockOffering(
	page: Page,
	options: {
		gate?: { path: string; promise: Promise<void> };
		failOnce?: string;
		published?: boolean;
		permissions?: string[];
		trackingMode?: 'manual' | 'homeroom';
		failTrackingSave?: boolean;
		saveGate?: Promise<void>;
	} = {}
) {
	const counts = new Map<string, number>();
	let tracking = {
		mode: options.trackingMode ?? 'manual',
		effectiveFrom: options.trackingMode === 'homeroom' ? '2026-10-09' : null,
		groupRowVersion: 1,
		startsOn: '2026-05-01',
		endsOn: '2027-04-30'
	};
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			if (path === '/api/auth/me') {
				await fulfill(route, {
					id: '90000000-0000-4000-8000-000000000201',
					username: 'offering-reader',
					firstName: 'ทดสอบ',
					displayName: 'ทดสอบ วิชาการ',
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					permissions: options.permissions ?? ['learning_offering.read.school']
				});
				return;
			}
			if (path === '/api/academic/context/options') {
				await fulfill(route, {
					defaultAcademicYearId: ids.year,
					defaultAcademicTermId: ids.term,
					years: [
						{
							isVisible: true,
							id: ids.year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: ids.term,
							academicYearId: ids.year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active'
						}
					]
				});
				return;
			}
			if (path === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			const count = (counts.get(path) ?? 0) + 1;
			counts.set(path, count);
			if (options.gate?.path === path) await options.gate.promise;
			if (options.failOnce === path && count === 1) {
				await route.fulfill({
					status: 503,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'ข้อมูลยังไม่พร้อม' })
				});
				return;
			}
			if (path.endsWith('/roster-tracking')) {
				if (route.request().method() === 'PUT') {
					if (options.saveGate) await options.saveGate;
					if (options.failTrackingSave)
						return void (await route.fulfill({
							status: 409,
							contentType: 'application/json',
							body: JSON.stringify({ success: false, error: 'กลุ่มนี้เปลี่ยนแล้ว กรุณาโหลดใหม่' })
						}));
					const body = route.request().postDataJSON();
					tracking = {
						...tracking,
						mode: body.mode,
						effectiveFrom: body.effectiveFrom,
						groupRowVersion: tracking.groupRowVersion + 1
					};
				}
				return void (await fulfill(route, tracking));
			}
			if (path === paths.offering) return void (await fulfill(route, offering()));
			if (path === paths.groups)
				return void (await fulfill(route, [
					group(ids.groupA, options.published),
					group(ids.groupB)
				]));
			if (path === paths.versions) return void (await fulfill(route, [version(options.published)]));
			if (path === paths.version)
				return void (await fulfill(route, versionGraph(options.published)));
			if (path === paths.groupA)
				return void (await fulfill(route, {
					...group(ids.groupA, options.published),
					rowVersion: tracking.groupRowVersion
				}));
			if (path === paths.groupB) return void (await fulfill(route, group(ids.groupB)));
			if (path === paths.membershipsA) return void (await fulfill(route, []));
			if (path === paths.management)
				return void (await fulfill(route, { rooms: [], teachers: [], homerooms: [] }));
			await fulfill(route, {});
		}
	);
	return Object.assign((path: string) => counts.get(path) ?? 0, {
		bumpTrackingRevision() {
			tracking = { ...tracking, groupRowVersion: tracking.groupRowVersion + 1 };
		}
	});
}

function url(groupId: string | null = ids.groupA) {
	return `/staff/academic/delivery/${ids.offering}?academicYearId=${ids.year}&academicTermId=${ids.term}${groupId ? `&groupId=${groupId}` : ''}`;
}

test('starts independent group reads while the opening identity is delayed', async ({ page }) => {
	const gate = deferred();
	const count = await mockOffering(page, { gate: { path: paths.offering, promise: gate.promise } });
	try {
		await page.goto(url());
		await expect.poll(() => count(paths.groups)).toBe(1);
		await expect.poll(() => count(paths.groupA)).toBe(1);
		await expect(page.getByTestId('delivery-offering-ready')).toHaveCount(0);
		expect(count(paths.versions)).toBe(0);
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('delivery-offering-ready')).toBeVisible();
	await expect(page.getByTestId('delivery-versions-ready')).toBeVisible();
	expect(count(paths.offering)).toBe(1);
	expect(count(paths.groups)).toBe(1);
	expect(count(paths.groupA)).toBe(1);
	expect(count(paths.versions)).toBe(1);
	expect(count(paths.management)).toBe(0);
});

test('offering, version, and direct group detail paint while the group list is delayed', async ({
	page
}) => {
	const gate = deferred();
	const count = await mockOffering(page, { gate: { path: paths.groups, promise: gate.promise } });
	try {
		await page.goto(url());
		await expect(page.getByTestId('delivery-offering-ready')).toBeVisible();
		await expect(page.getByTestId('delivery-versions-ready')).toBeVisible();
		await expect(page.getByTestId('delivery-selected-group-ready')).toBeVisible();
		await expect(page.getByTestId('delivery-groups-ready')).toHaveCount(0);
		expect(count(paths.groups)).toBe(1);
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
});

test('default group reuses the complete group list row without another GET', async ({ page }) => {
	const gate = deferred();
	const count = await mockOffering(page, { gate: { path: paths.groups, promise: gate.promise } });
	try {
		await page.goto(url(null));
		await expect(page.getByTestId('delivery-offering-ready')).toBeVisible();
		await expect(page.getByTestId('delivery-selected-group-ready')).toHaveCount(0);
		expect(count(paths.groupA)).toBe(0);
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('delivery-selected-group-ready')).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`groupId=${ids.groupA}`));
	expect(count(paths.groupA)).toBe(0);
});

test('group selection and Back load only the selected group', async ({ page }) => {
	const count = await mockOffering(page);
	await page.goto(url());
	await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
	await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ก');
	await page.getByRole('button', { name: /กลุ่ม ข/ }).click();
	await expect(page).toHaveURL(new RegExp(`groupId=${ids.groupB}`));
	await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ข');
	await page.goBack();
	await expect(page).toHaveURL(new RegExp(`groupId=${ids.groupA}`));
	await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ก');
	expect(count(paths.offering)).toBe(1);
	expect(count(paths.groups)).toBe(1);
	expect(count(paths.versions)).toBe(1);
	expect(count(paths.groupB)).toBe(0);
});

test('a late old selected group never replaces the newly selected group', async ({ page }) => {
	const gate = deferred();
	const count = await mockOffering(page, { gate: { path: paths.groupA, promise: gate.promise } });
	try {
		await page.goto(url());
		await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
		await expect.poll(() => count(paths.groupA)).toBe(1);
		await page.getByRole('button', { name: /กลุ่ม ข/ }).click();
		await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ข');
	} finally {
		gate.release();
	}
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ข');
});

test('late management options do not open the editor for a different group', async ({ page }) => {
	const gate = deferred();
	const count = await mockOffering(page, {
		permissions: ['*'],
		gate: { path: paths.management, promise: gate.promise }
	});
	try {
		await page.goto(url());
		await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ก');
		await page.getByRole('button', { name: 'จัดการกลุ่ม' }).click();
		await expect.poll(() => count(paths.management)).toBe(1);
		await page.getByRole('button', { name: /กลุ่ม ข/ }).click();
		await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('กลุ่ม ข');
	} finally {
		gate.release();
	}
	await expect(page.getByRole('heading', { name: /จัดการกลุ่ม ·/ })).toHaveCount(0);
});

test('published membership history starts from the route without management options', async ({
	page
}) => {
	const count = await mockOffering(page, { published: true });
	await page.goto(url());
	await expect(page.getByRole('heading', { name: 'ประวัติสมาชิกกลุ่มเรียน' })).toBeVisible();
	await expect.poll(() => count(paths.membershipsA)).toBe(1);
	expect(count(paths.management)).toBe(0);
	expect(count(`${paths.groupA}/roster`)).toBe(0);
});

test('published history has its own first skeleton and ready state', async ({ page }) => {
	const gate = deferred();
	const count = await mockOffering(page, {
		published: true,
		gate: { path: paths.membershipsA, promise: gate.promise }
	});
	try {
		await page.goto(url());
		await expect(page.getByTestId('delivery-version-roster')).toBeVisible();
		await expect.poll(() => count(paths.membershipsA)).toBe(1);
		await expect(page.getByLabel('กำลังโหลดประวัติสมาชิก')).toBeVisible();
		await expect(page.getByTestId('delivery-memberships-ready')).toHaveCount(0);
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('delivery-memberships-ready')).toBeVisible();
});

test('a failed versions region retries without rerunning offering or groups', async ({ page }) => {
	const count = await mockOffering(page, { failOnce: paths.versions });
	await page.goto(url());
	await expect(page.getByRole('button', { name: 'ลองอีกครั้ง' })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByTestId('delivery-versions-ready')).toBeVisible();
	expect(count(paths.versions)).toBe(2);
	expect(count(paths.offering)).toBe(1);
	expect(count(paths.groups)).toBe(1);
});

for (const width of [375, 1280]) {
	test(`automatic roster mode is editable on published versions at ${width}px`, async ({
		page
	}) => {
		await page.setViewportSize({ width, height: 900 });
		const count = await mockOffering(page, {
			published: true,
			permissions: ['learning_offering.read.school', 'learning_offering.manage.school']
		});
		await page.goto(url());
		const panel = page.getByTestId('roster-tracking');
		await expect(panel).toContainText('จัดรายชื่อเอง');
		await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
		await page.getByRole('option', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).click();
		await expect(
			panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })
		).toBeVisible();
		await panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
		await expect(panel).toContainText('บันทึกวิธีจัดรายชื่อแล้ว');
		await expect(
			page.getByRole('button', { name: 'เพิ่มนักเรียนตามวันที่', exact: true })
		).toHaveCount(0);
		expect(count(paths.offering)).toBe(1);
		expect(count(paths.groups)).toBe(1);
		await page.screenshot({
			path: `/tmp/schoolorbit-roster-tracking-${width}-light.png`,
			fullPage: true
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({
			path: `/tmp/schoolorbit-roster-tracking-${width}-dark.png`,
			fullPage: true
		});
		await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
		await page.getByRole('option', { name: 'จัดรายชื่อเอง', exact: true }).click();
		await panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
		await expect(
			page.getByRole('button', { name: 'เพิ่มนักเรียนตามวันที่', exact: true })
		).toBeVisible();
	});
}
test('tracking save is pending locally and a conflict preserves mode and date', async ({
	page
}) => {
	const gate = deferred();
	await mockOffering(page, {
		published: true,
		permissions: ['learning_offering.read.school', 'learning_offering.manage.school'],
		failTrackingSave: true,
		saveGate: gate.promise
	});
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
	await page.getByRole('option', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).click();
	const date = await panel
		.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })
		.innerText();
	await panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
	await expect(panel.getByRole('button', { name: 'กำลังบันทึก', exact: true })).toBeDisabled();
	gate.release();
	await expect(panel.getByRole('alert')).toContainText('กลุ่มนี้เปลี่ยนแล้ว');
	await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toContainText(
		'ติดตามห้องอัตโนมัติ'
	);
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toHaveText(
		date
	);
});

test('automatic roster focus refresh retains draft and updates only changed group regions', async ({
	page
}) => {
	const count = await mockOffering(page, {
		published: true,
		trackingMode: 'homeroom',
		permissions: ['learning_offering.read.school', 'learning_offering.manage.school']
	});
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	const mode = panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true });
	await expect(mode).toContainText('ติดตามห้องอัตโนมัติ');
	await mode.click();
	await page.getByRole('option', { name: 'จัดรายชื่อเอง', exact: true }).click();
	await page.evaluate(() => window.dispatchEvent(new Event('focus')));
	await expect.poll(() => count(paths.groupA)).toBe(2);
	await expect(mode).toContainText('จัดรายชื่อเอง');
	expect(count(`${paths.groupA}/roster-tracking`)).toBe(1);
	count.bumpTrackingRevision();
	await page.evaluate(() => window.dispatchEvent(new Event('focus')));
	await expect.poll(() => count(`${paths.groupA}/roster-tracking`)).toBe(2);
	await expect.poll(() => count(paths.membershipsA)).toBe(2);
	await expect(mode).toContainText('จัดรายชื่อเอง');
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeEnabled();
	expect(count(paths.offering)).toBe(1);
	expect(count(paths.groups)).toBe(1);
});

const bulkTeacherId = '90000000-0000-4000-8000-000000000220';
const existingPrimaryId = '90000000-0000-4000-8000-000000000221';
function bulkTeacher(teacherId: string, role: 'primary' | 'secondary' | 'assistant', name: string) {
	return {
		id: `${teacherId}-${role}`,
		teacherId,
		displayName: name,
		role,
		rowVersion: 1,
		startsOn: '2026-05-01',
		endsOn: null
	};
}
async function mockBulkSetup(
	page: Page,
	options: {
		applyGate?: Promise<void>;
		conflictOnce?: boolean;
		optionsGate?: Promise<void>;
		blocked?: boolean;
		onlySkipped?: boolean;
		preparationGate?: Promise<void>;
		initialTrackingMode?: 'manual' | 'homeroom';
		trackingGate?: Promise<void>;
		trackingRefreshGate?: Promise<void>;
		trackingSaveGate?: Promise<void>;
		trackingStartsOn?: string;
	} = {}
) {
	const count = await mockOffering(page, { permissions: ['*'] });
	let rows = [
		{
			...group(ids.groupA),
			name: 'ม.1/1',
			teacherAssignments: [bulkTeacher('assistant', 'assistant', 'ครูผู้ช่วยเดิม')]
		},
		{
			...group(ids.groupB),
			name: 'ม.1/2',
			teacherAssignments: [
				bulkTeacher(existingPrimaryId, 'primary', 'ครูหลักเดิม'),
				bulkTeacher('secondary', 'secondary', 'ครูร่วมเดิม')
			]
		}
	];
	const trackingByGroup = new Map(
		rows.map((row) => [
			row.id,
			{
				mode: options.initialTrackingMode ?? 'manual',
				effectiveFrom: options.initialTrackingMode === 'homeroom' ? '2026-10-27' : null,
				groupRowVersion: row.rowVersion,
				startsOn: options.trackingStartsOn ?? '2026-05-01',
				endsOn: '2027-04-30'
			}
		])
	);
	const trackingReads: string[] = [];
	const trackingSaves: Array<{
		mode: 'manual' | 'homeroom';
		effectiveFrom: string | null;
		rowVersion: number;
	}> = [];
	let currentVersion = 1;
	let groupReads = 0;
	let failPreparation = false;
	const previewRequests: Array<{
		groups: Array<{ learningGroupId: string; rowVersion: number }>;
		primaryTeacher?: { teacherId: string; replaceExistingPrimary: boolean };
		rosterTracking?: { mode: 'manual' | 'homeroom'; effectiveFrom: string | null };
		deliveryVersionId: string;
		deliveryVersionRowVersion: number;
	}> = [];
	const applyRequests: typeof previewRequests = [];
	await page.route(`**${paths.groups}`, async (route) => {
		groupReads += 1;
		if (failPreparation) {
			failPreparation = false;
			return void (await route.fulfill({
				status: 503,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'เตรียมข้อมูลไม่สำเร็จ' })
			}));
		}
		if (groupReads > 1 && options.preparationGate) await options.preparationGate;
		return fulfill(route, rows);
	});
	await page.route(`**${paths.groupA}`, (route) => fulfill(route, rows[0]));
	await page.route(`**${paths.groupB}`, (route) => fulfill(route, rows[1]));
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/academic/learning-groups/') &&
			url.pathname.endsWith('/roster-tracking'),
		async (route) => {
			const id = new URL(route.request().url()).pathname.split('/')[4];
			const config = trackingByGroup.get(id);
			if (!config) return route.fallback();
			if (route.request().method() === 'PUT') {
				const body = route.request().postDataJSON();
				trackingSaves.push(body);
				const updated = {
					...config,
					mode: body.mode,
					effectiveFrom: body.effectiveFrom,
					groupRowVersion:
						(rows.find((row) => row.id === id)?.rowVersion ?? config.groupRowVersion) + 1
				};
				trackingByGroup.set(id, updated);
				rows = rows.map((row) =>
					row.id === id ? { ...row, rowVersion: updated.groupRowVersion } : row
				);
				if (options.trackingSaveGate) await options.trackingSaveGate;
				return fulfill(route, updated);
			}
			const snapshot = {
				...config,
				groupRowVersion: rows.find((row) => row.id === id)?.rowVersion ?? config.groupRowVersion
			};
			trackingReads.push(id);
			if (options.trackingGate) await options.trackingGate;
			if (trackingReads.length > 1 && options.trackingRefreshGate)
				await options.trackingRefreshGate;
			return fulfill(route, snapshot);
		}
	);
	await page.route(`**${paths.version}`, (route) =>
		fulfill(route, { ...versionGraph(), rowVersion: currentVersion })
	);
	await page.route('**/api/academic/delivery/management-options?**', async (route) => {
		if (options.optionsGate) await options.optionsGate;
		await fulfill(route, {
			teachers: [
				{ id: bulkTeacherId, name: 'ครูหลักใหม่' },
				{ id: existingPrimaryId, name: 'ครูหลักเดิม' },
				{ id: 'secondary', name: 'ครูร่วมเดิม' },
				{ id: 'assistant', name: 'ครูผู้ช่วยเดิม' }
			],
			homerooms: [],
			rooms: [],
			gradeLevels: [],
			studyPrograms: [],
			catalogVersions: [],
			learningGroups: []
		});
	});
	await page.route(
		(url) =>
			url.pathname === `${paths.groups}/bulk-setup` ||
			url.pathname === `${paths.groups}/bulk-setup/preview`,
		async (route) => {
			const pathname = new URL(route.request().url()).pathname;
			const method = route.request().method();
			if (!(
				(method === 'POST' && pathname === `${paths.groups}/bulk-setup/preview`) ||
				(method === 'PUT' && pathname === `${paths.groups}/bulk-setup`)
			)) {
				return void (await route.fulfill({
					status: 405,
					contentType: 'application/json',
					body: JSON.stringify({
						success: false,
						error: 'Method not allowed for this bulk setup route'
					})
				}));
			}
			const body = route.request().postDataJSON();
			if (route.request().method() === 'POST') {
				previewRequests.push(body);
				const targets = body.groups.map((target: { learningGroupId: string }) => {
					const row = rows.find((row) => row.id === target.learningGroupId)!;
					return {
						group: row,
						rosterTracking: { ...trackingByGroup.get(row.id)!, groupRowVersion: row.rowVersion },
						primaryTeacherAction: !body.primaryTeacher
							? 'unchanged'
							: row.id === ids.groupB
								? body.primaryTeacher.replaceExistingPrimary
									? 'replace'
									: 'preserve_existing'
								: 'set',
						rosterTrackingChanged: Boolean(
							body.rosterTracking &&
							(body.rosterTracking.mode !== trackingByGroup.get(row.id)?.mode ||
								body.rosterTracking.effectiveFrom !== trackingByGroup.get(row.id)?.effectiveFrom)
						),
						blockedReason: options.blocked ? 'กลุ่มนี้ไม่พร้อมแก้ไข' : null
					};
				});
				const changedCount = targets.filter(
					(target: {
						primaryTeacherAction: string;
						rosterTrackingChanged: boolean;
						blockedReason: string | null;
					}) =>
						!target.blockedReason &&
						(['set', 'replace'].includes(target.primaryTeacherAction) ||
							target.rosterTrackingChanged)
				).length;
				return fulfill(route, {
					deliveryVersionRowVersion: currentVersion,
					targets,
					changedCount,
					skippedCount: options.blocked ? 0 : targets.length - changedCount,
					blockedCount: options.blocked ? targets.length : 0
				});
			}
			applyRequests.push(body);
			if (options.onlySkipped)
				return void (await route.fulfill({
					status: 409,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'ไม่มีกลุ่มเรียนที่ต้องเปลี่ยนการตั้งค่า' })
				}));

			if (options.blocked)
				return void (await route.fulfill({
					status: 409,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'กลุ่มนี้ไม่พร้อมแก้ไข' })
				}));
			if (options.applyGate) await options.applyGate;
			if (options.conflictOnce && applyRequests.length === 1) {
				currentVersion = 2;
				rows = rows.map((row) => ({ ...row, rowVersion: 2 }));
				return void (await route.fulfill({
					status: 409,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'ข้อมูลเปลี่ยนแล้ว' })
				}));
			}
			rows = rows.map((row) => {
				if (
					!body.groups.some(
						(target: { learningGroupId: string }) => target.learningGroupId === row.id
					)
				)
					return row;
				const teachers =
					body.primaryTeacher &&
					(row.id !== ids.groupB || body.primaryTeacher.replaceExistingPrimary)
						? [
								...row.teacherAssignments.filter((teacher) => teacher.role !== 'primary'),
								bulkTeacher(bulkTeacherId, 'primary', 'ครูหลักใหม่')
							]
						: row.teacherAssignments;
				return { ...row, rowVersion: row.rowVersion + 1, teacherAssignments: teachers };
			});
			for (const row of rows) {
				if (
					!body.groups.some(
						(target: { learningGroupId: string }) => target.learningGroupId === row.id
					)
				)
					continue;
				const previous = trackingByGroup.get(row.id)!;
				trackingByGroup.set(row.id, {
					...previous,
					...(body.rosterTracking ?? {}),
					groupRowVersion: row.rowVersion
				});
			}
			currentVersion += 1;
			return fulfill(route, {
				deliveryVersionRowVersion: currentVersion,
				groups: rows.filter((row) =>
					body.groups.some(
						(target: { learningGroupId: string }) => target.learningGroupId === row.id
					)
				),
				rosterTracking: rows
					.filter((row) =>
						body.groups.some(
							(target: { learningGroupId: string }) => target.learningGroupId === row.id
						)
					)
					.map((row) => ({
						learningGroupId: row.id,
						tracking: trackingByGroup.get(row.id)
					}))
			});
		}
	);
	return {
		count,
		previewRequests,
		applyRequests,
		trackingReads,
		trackingSaves,
		advanceGroupRevision() {
			currentVersion += 1;
			rows[0] = { ...rows[0], rowVersion: rows[0].rowVersion + 1 };
		},
		failPreparationOnce() {
			failPreparation = true;
		},
		renameGroup(name: string) {
			currentVersion += 1;
			rows[0] = { ...rows[0], name, rowVersion: rows[0].rowVersion + 1 };
			return rows[0];
		}
	};
}
async function chooseBulkTeacher(page: Page) {
	const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await dialog.getByRole('checkbox', { name: 'กำหนดครูหลัก', exact: true }).check();
	await dialog
		.getByRole('combobox', { name: 'เลือกครูหลักสำหรับกลุ่มเป้าหมาย', exact: true })
		.click();
	await page.getByRole('option', { name: 'ครูหลักใหม่', exact: true }).click();
	return dialog;
}

for (const width of [1280, 390]) {
	for (const dark of [false, true]) {
		test(`bulk setup saves explicit scope in one step, skips existing primary and preserves co-teachers at ${width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			const fixture = await mockBulkSetup(page);
			await page.goto(url());
			if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
			expect(fixture.count(paths.management)).toBe(0);
			await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
			const dialog = await chooseBulkTeacher(page);
			await expect(
				dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
			).toBeDisabled();
			await expect(dialog.getByRole('button', { name: 'ตรวจการตั้งค่า', exact: true })).toHaveCount(
				0
			);
			await dialog.getByLabel('ค้นหากลุ่มเป้าหมาย', { exact: true }).fill('ม.1/1');
			await dialog
				.getByRole('button', { name: 'เลือกกลุ่มที่แก้ไขได้ 2 กลุ่ม (ทุกหน้า)', exact: true })
				.click();
			await dialog.getByLabel('ค้นหากลุ่มเป้าหมาย', { exact: true }).fill('');
			await expect(dialog).toContainText('เลือก 2/2 กลุ่ม');
			await expect(
				dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
			).toBeEnabled();
			await page.screenshot({
				animations: 'disabled',
				path: testInfo.outputPath(`bulk-direct-save-${width}-${dark ? 'dark' : 'light'}.png`)
			});
			const bounds = await dialog.boundingBox();
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			const saveBounds = await dialog
				.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
				.boundingBox();
			expect(saveBounds!.y + saveBounds!.height).toBeLessThanOrEqual(900);
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
			await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(
				page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true })
			).toBeFocused();
			await page.getByRole('main').evaluate((element) => (element.scrollTop = 0));
			await page.mouse.move(width - 20, 500);
			await page.mouse.wheel(0, 300);
			await expect
				.poll(() => page.getByRole('main').evaluate((element) => element.scrollTop))
				.toBeGreaterThan(0);
			expect(fixture.applyRequests).toEqual([
				{
					deliveryVersionId: ids.version,
					deliveryVersionRowVersion: 1,
					groups: [
						{ learningGroupId: ids.groupA, rowVersion: 1 },
						{ learningGroupId: ids.groupB, rowVersion: 1 }
					],
					primaryTeacher: { teacherId: bulkTeacherId, replaceExistingPrimary: false }
				}
			]);
			expect(fixture.previewRequests).toHaveLength(0);
			await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
			const reopened = await chooseBulkTeacher(page);
			await reopened.getByRole('checkbox', { name: /ม.1\/1/ }).check();
			await expect(
				reopened.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
			).toBeEnabled();
			await reopened.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
			await page.getByRole('button', { name: /ม.1\/2/ }).click();
			await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
			await expect(
				page.getByRole('heading', { name: 'จัดการกลุ่ม · ม.1/2', exact: true })
			).toBeVisible();
			await expect(page.getByText('ครูหลักเดิม', { exact: true })).toBeVisible();
			await expect(page.getByText('ครูร่วมเดิม', { exact: true })).toBeVisible();
			await expect(
				page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true })
			).toBeDisabled();
			await page.getByRole('button', { name: 'ยกเลิกการแก้ไขกลุ่ม', exact: true }).click();
			await expect(
				page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true })
			).toBeEnabled();
		});
	}
}

test('bulk explicit replacement and roster mode use one atomic apply and prevent double-submit or dismissal', async ({
	page
}) => {
	const gate = deferred();
	const fixture = await mockBulkSetup(page, { applyGate: gate.promise });
	await page.goto(url());
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = await chooseBulkTeacher(page);
	await dialog.getByRole('checkbox', { name: /ม.1\/2/ }).check();
	await dialog
		.getByRole('checkbox', { name: 'แทนครูหลักเดิมในกลุ่มที่เลือกด้วย', exact: true })
		.check();
	await dialog.getByRole('checkbox', { name: 'กำหนดวิธีจัดรายชื่อ', exact: true }).check();
	await dialog.getByRole('radio', { name: 'จัดรายชื่อเอง', exact: true }).check();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	expect(fixture.applyRequests[0]).toMatchObject({
		groups: [{ learningGroupId: ids.groupB, rowVersion: 1 }],
		primaryTeacher: { replaceExistingPrimary: true },
		rosterTracking: { mode: 'manual', effectiveFrom: null }
	});
	await expect(dialog.getByRole('button', { name: 'กำลังบันทึก', exact: true })).toBeDisabled();
	await page.keyboard.press('Escape');
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
	expect(fixture.applyRequests).toHaveLength(1);
	gate.release();
	await expect(dialog).toHaveCount(0);
});

test('bulk conflict preserves choices, requires explicit latest read and sends fresh revisions', async ({
	page
}) => {
	const fixture = await mockBulkSetup(page, { conflictOnce: true });
	await page.goto(url());
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = await chooseBulkTeacher(page);
	await dialog.getByRole('checkbox', { name: /ม.1\/1/ }).check();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('โหลดกลุ่มล่าสุด');
	await expect(dialog.getByRole('checkbox', { name: /ม.1\/1/ })).toBeChecked();
	await expect(
		dialog.getByRole('combobox', { name: 'เลือกครูหลักสำหรับกลุ่มเป้าหมาย', exact: true })
	).toContainText('ครูหลักใหม่');
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeDisabled();
	await dialog.getByRole('button', { name: 'โหลดกลุ่มล่าสุด', exact: true }).click();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(fixture.applyRequests).toHaveLength(2);
	expect(fixture.applyRequests[1]).toMatchObject({
		deliveryVersionRowVersion: 2,
		groups: [{ learningGroupId: ids.groupA, rowVersion: 2 }]
	});
});

test('bulk apply rejects unsafe targets atomically and a late options response cannot reopen a closed dialog', async ({
	page
}) => {
	const gate = deferred();
	const fixture = await mockBulkSetup(page, { optionsGate: gate.promise, blocked: true });
	await page.goto(url());
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	let dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	gate.release();
	await expect(dialog).toHaveCount(0);
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	dialog = await chooseBulkTeacher(page);
	await dialog.getByRole('checkbox', { name: /ม.1\/1/ }).check();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('กลุ่มนี้ไม่พร้อมแก้ไข');
	await expect(dialog.getByRole('alert')).toContainText('โหลดกลุ่มล่าสุด');
	await expect(dialog.getByRole('checkbox', { name: /ม.1\/1/ })).toBeChecked();
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeDisabled();
	expect(fixture.previewRequests).toHaveLength(0);
	expect(fixture.applyRequests).toHaveLength(1);
});

test('bulk only-skipped group refusal preserves the actual server reason and choices until explicit reload', async ({
	page
}) => {
	const fixture = await mockBulkSetup(page, { onlySkipped: true });
	await page.goto(url());
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = await chooseBulkTeacher(page);
	await dialog.getByRole('checkbox', { name: /ม.1\/2/ }).check();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('ไม่มีกลุ่มเรียนที่ต้องเปลี่ยนการตั้งค่า');
	await expect(dialog.getByRole('alert')).toContainText('โหลดกลุ่มล่าสุด');
	await expect(dialog.getByRole('checkbox', { name: /ม.1\/2/ })).toBeChecked();
	await expect(
		dialog.getByRole('combobox', { name: 'เลือกครูหลักสำหรับกลุ่มเป้าหมาย', exact: true })
	).toContainText('ครูหลักใหม่');
	await expect(
		dialog.getByRole('checkbox', { name: 'แทนครูหลักเดิมในกลุ่มที่เลือกด้วย', exact: true })
	).not.toBeChecked();
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeDisabled();
	expect(fixture.applyRequests).toHaveLength(1);
	expect(fixture.previewRequests).toHaveLength(0);
	expect(fixture.applyRequests[0]).toMatchObject({
		groups: [{ learningGroupId: ids.groupB, rowVersion: 1 }],
		primaryTeacher: { replaceExistingPrimary: false }
	});
	expect(fixture.applyRequests[0].rosterTracking).toBeUndefined();
	await dialog.getByRole('button', { name: 'โหลดกลุ่มล่าสุด', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveCount(0);
	await expect(dialog.getByRole('checkbox', { name: /ม.1\/2/ })).toBeChecked();
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeEnabled();
	expect(fixture.applyRequests).toHaveLength(1);
});

test('bulk automatic roster configuration defaults to Bangkok today and patches selected tracking without rebuilding members', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-26T18:00:00Z'));
	const fixture = await mockBulkSetup(page);
	await page.goto(url());
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await dialog.getByRole('checkbox', { name: /ม.1\/1/ }).check();
	await dialog.getByRole('checkbox', { name: 'กำหนดวิธีจัดรายชื่อ', exact: true }).check();
	await expect(
		dialog.getByRole('radio', { name: 'ติดตามห้องอัตโนมัติ', exact: true })
	).toBeChecked();
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeEnabled();
	await expect(
		dialog.getByRole('button', { name: 'วันที่เริ่มติดตามสำหรับกลุ่มเป้าหมาย', exact: true })
	).toContainText('27 ต.ค. 2569');
	await expect(dialog).toContainText('สมาชิกใหม่เริ่มนับจากวันที่นี้');
	const date = dialog.getByRole('button', {
		name: 'วันที่เริ่มติดตามสำหรับกลุ่มเป้าหมาย',
		exact: true
	});
	await date.click();
	await page.locator('[data-bits-day][data-selected]').click();
	await expect(date).toContainText('27 ต.ค. 2569');
	await expect(page.locator('[data-bits-day]')).toHaveCount(0);
	await date.click();
	await page.locator('[data-bits-day][data-selected]').focus();
	await page.keyboard.press('Enter');
	await expect(date).toContainText('27 ต.ค. 2569');
	await expect(page.locator('[data-bits-day]')).toHaveCount(0);
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeEnabled();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByTestId('roster-tracking')).toContainText('ติดตามนักเรียนจากห้องต้นทาง');
	const panel = page.getByTestId('roster-tracking');
	await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
		'ติดตามห้องอัตโนมัติ'
	);
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'27 ต.ค. 2569'
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeDisabled();
	expect(fixture.trackingReads).toHaveLength(1);
	expect(fixture.trackingSaves).toHaveLength(0);
	expect(fixture.previewRequests).toHaveLength(0);
	expect(fixture.applyRequests).toHaveLength(1);
	expect(fixture.applyRequests[0]).toMatchObject({
		rosterTracking: { mode: 'homeroom', effectiveFrom: '2026-10-27' }
	});
	expect(fixture.applyRequests[0].primaryTeacher).toBeUndefined();
	expect(fixture.count(`${paths.groupA}/roster`)).toBe(0);
	expect(fixture.count(paths.membershipsA)).toBe(0);
});

test('opening bulk setup reads the fresh offering frame after a successful individual group save', async ({
	page
}) => {
	const fixture = await mockBulkSetup(page);
	await page.route(`**${paths.groupA}`, async (route) => {
		if (route.request().method() !== 'PATCH') return route.fallback();
		return fulfill(route, fixture.renameGroup(route.request().postDataJSON().name));
	});
	await page.goto(url());
	expect(fixture.previewRequests).toHaveLength(0);
	expect(fixture.count(paths.management)).toBe(0);
	await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
	await page
		.getByRole('textbox', { name: 'ชื่อกลุ่ม', exact: true })
		.fill('ม.1/1 ชื่อที่บันทึกใหม่');
	await page.getByRole('button', { name: 'บันทึกข้อมูลกลุ่ม', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'จัดการกลุ่ม · ม.1/1 ชื่อที่บันทึกใหม่', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'ยกเลิกการแก้ไขกลุ่ม', exact: true }).click();
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = await chooseBulkTeacher(page);
	await dialog.getByRole('checkbox', { name: /ม.1\/1 ชื่อที่บันทึกใหม่/ }).check();
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(fixture.previewRequests).toHaveLength(0);
	expect(fixture.applyRequests).toHaveLength(1);
	expect(fixture.applyRequests[0]).toMatchObject({
		deliveryVersionRowVersion: 2,
		groups: [{ learningGroupId: ids.groupA, rowVersion: 2 }]
	});
	await expect(page).toHaveURL(new RegExp(`groupId=${ids.groupA}`));
});

test('bulk opening preparation has a focused retry and can be cancelled while its read is pending', async ({
	page
}) => {
	const gate = deferred();
	const fixture = await mockBulkSetup(page, { preparationGate: gate.promise });
	await page.goto(url());
	await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
	fixture.failPreparationOnce();
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await expect(dialog).toContainText('เตรียมข้อมูลไม่สำเร็จ');
	await dialog.getByRole('button', { name: 'ลองเตรียมกลุ่มอีกครั้ง', exact: true }).click();
	await expect(
		dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
	).toBeDisabled();
	await expect(dialog.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeEnabled();
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	gate.release();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true })).toBeFocused();
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	await expect(dialog).toContainText('เลือก 0/2 กลุ่ม');
	await expect(dialog.getByRole('alert')).toHaveCount(0);
	expect(fixture.previewRequests).toHaveLength(0);
	expect(fixture.applyRequests).toHaveLength(0);
});

test('a fresh bulk opening resolves the selected group while an older direct read is pending', async ({
	page
}) => {
	const gate = deferred();
	await mockBulkSetup(page);
	await page.route(`**${paths.groupA}`, async (route) => {
		await gate.promise;
		await fulfill(route, { ...group(ids.groupA), name: 'ข้อมูลเก่าที่มาช้า' });
	});
	await page.goto(url());
	await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
	await expect(page.getByTestId('delivery-selected-group-ready')).toHaveCount(0);
	try {
		await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
		await expect(dialog).toContainText('เลือก 0/2 กลุ่ม');
		await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('ม.1/1');
		await expect(page).toHaveURL(new RegExp(`groupId=${ids.groupA}`));
	} finally {
		gate.release();
	}
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await expect(page.getByTestId('delivery-selected-group-ready')).toContainText('ม.1/1');
	await expect(page.getByText('ข้อมูลเก่าที่มาช้า', { exact: true })).toHaveCount(0);
});

async function applyBulkRosterMode(page: Page, mode: 'manual' | 'homeroom', tomorrow = false) {
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await dialog.getByRole('checkbox', { name: /ม.1\/1/ }).check();
	await dialog.getByRole('checkbox', { name: 'กำหนดวิธีจัดรายชื่อ', exact: true }).check();
	await dialog
		.getByRole('radio', {
			name: mode === 'homeroom' ? 'ติดตามห้องอัตโนมัติ' : 'จัดรายชื่อเอง',
			exact: true
		})
		.check();
	if (mode === 'homeroom') {
		await dialog
			.getByRole('button', { name: 'วันที่เริ่มติดตามสำหรับกลุ่มเป้าหมาย', exact: true })
			.click();
		const today = page.locator('[data-bits-day][data-today]');
		if (tomorrow) {
			await today.focus();
			await page.keyboard.press('ArrowRight');
			await page.keyboard.press('Enter');
		} else await today.click();
	}
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
}

test('returned bulk manual mode updates individual controls without another tracking read or stale Save', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	const fixture = await mockBulkSetup(page, { initialTrackingMode: 'homeroom' });
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
		'ติดตามห้องอัตโนมัติ'
	);
	await applyBulkRosterMode(page, 'manual');
	await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
		'จัดรายชื่อเอง'
	);
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toHaveCount(
		0
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeDisabled();
	expect(fixture.trackingReads).toHaveLength(1);
	expect(fixture.trackingSaves).toHaveLength(0);
});

test('a pending old tracking read cannot replace the newer bulk configuration or its controls', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	const gate = deferred();
	const fixture = await mockBulkSetup(page, { trackingGate: gate.promise });
	await page.goto(url());
	await expect.poll(() => fixture.trackingReads.length).toBe(1);
	const panel = page.getByTestId('roster-tracking');
	try {
		await applyBulkRosterMode(page, 'homeroom');
		await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
			'ติดตามห้องอัตโนมัติ'
		);
		await expect(
			panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
		).toBeDisabled();
	} finally {
		gate.release();
	}
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
		'ติดตามห้องอัตโนมัติ'
	);
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'27 ต.ค. 2569'
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeDisabled();
	expect(fixture.trackingReads).toHaveLength(1);
	expect(fixture.trackingSaves).toHaveLength(0);
});

test('a changed bulk configuration preserves a genuine individual draft and blocks Save until resolved', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	const fixture = await mockBulkSetup(page);
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
	await page.getByRole('option', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).click();
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'27 ต.ค. 2569'
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeEnabled();
	await applyBulkRosterMode(page, 'homeroom', true);
	expect(fixture.applyRequests[0]).toMatchObject({
		rosterTracking: { mode: 'homeroom', effectiveFrom: '2026-10-28' }
	});
	await expect(panel.getByRole('alert')).toContainText('การแก้ไขของคุณยังอยู่');
	await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
		'ติดตามห้องอัตโนมัติ'
	);
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'27 ต.ค. 2569'
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeDisabled();
	await panel.getByRole('button', { name: 'ใช้วิธีจัดรายชื่อล่าสุด', exact: true }).click();
	await expect(panel.getByRole('alert')).toHaveCount(0);
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'28 ต.ค. 2569'
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeDisabled();
	expect(fixture.trackingReads).toHaveLength(1);
	expect(fixture.trackingSaves).toHaveLength(0);
});

test('a delayed individual save acknowledgment cannot cancel a newer tracking read or regress revisions', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	const saveGate = deferred();
	const refreshGate = deferred();
	const fixture = await mockBulkSetup(page, {
		trackingSaveGate: saveGate.promise,
		trackingRefreshGate: refreshGate.promise
	});
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
	await page.getByRole('option', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).click();
	await panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
	await expect.poll(() => fixture.trackingSaves.length).toBe(1);
	fixture.advanceGroupRevision();
	try {
		await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
		await expect(dialog).toContainText('เลือก 0/2 กลุ่ม');
		await expect.poll(() => fixture.trackingReads.length).toBe(2);
		await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		const oldSaveResponse = page.waitForResponse(
			(response) =>
				new URL(response.url()).pathname === `${paths.groupA}/roster-tracking` &&
				response.request().method() === 'PUT'
		);
		saveGate.release();
		await (await oldSaveResponse).finished();
		await expect(panel.getByRole('button', { name: 'กำลังบันทึก', exact: true })).toHaveCount(0);
		const newReadResponse = page.waitForResponse(
			(response) =>
				new URL(response.url()).pathname === `${paths.groupA}/roster-tracking` &&
				response.request().method() === 'GET'
		);
		refreshGate.release();
		await (await newReadResponse).finished();
		await expect(panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })).toHaveText(
			'ติดตามห้องอัตโนมัติ'
		);
		await expect(
			panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
		).toBeDisabled();
		await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
		await page.getByRole('option', { name: 'จัดรายชื่อเอง', exact: true }).click();
		await panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
		await expect.poll(() => fixture.trackingSaves.length).toBe(2);
		expect(fixture.trackingSaves[1].rowVersion).toBe(3);
		await expect(panel.getByRole('status')).toContainText('บันทึกวิธีจัดรายชื่อแล้ว');
		await expect(panel.getByRole('alert')).toHaveCount(0);
	} finally {
		saveGate.release();
		refreshGate.release();
	}
});

test('bulk dialog offers direct save and releases underlying actions with real touch input', async ({
	browser,
	baseURL
}, testInfo) => {
	const context = await browser.newContext({
		baseURL,
		hasTouch: true,
		isMobile: true,
		viewport: { width: 390, height: 844 },
		serviceWorkers: 'block'
	});
	try {
		const page = await context.newPage();
		const fixture = await mockBulkSetup(page);
		await page.goto(url());
		expect(await page.evaluate(() => navigator.maxTouchPoints)).toBeGreaterThan(0);
		expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
		await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).tap();
		const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
		await dialog.getByRole('checkbox', { name: /ม.1\/1/ }).tap();
		await dialog.getByRole('checkbox', { name: 'กำหนดครูหลัก', exact: true }).tap();
		await dialog
			.getByRole('combobox', { name: 'เลือกครูหลักสำหรับกลุ่มเป้าหมาย', exact: true })
			.tap();
		await page.getByRole('option', { name: 'ครูหลักใหม่', exact: true }).tap();
		await expect(
			dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true })
		).toBeEnabled();
		await expect(dialog.getByRole('button', { name: 'ตรวจการตั้งค่า', exact: true })).toHaveCount(
			0
		);
		await page.screenshot({ path: testInfo.outputPath('bulk-touch-preview.png') });
		await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).tap();
		await expect(dialog).toHaveCount(0);
		await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).tap();
		await expect(
			page.getByRole('heading', { name: 'จัดการกลุ่ม · ม.1/1', exact: true })
		).toBeVisible();
		expect(fixture.previewRequests).toHaveLength(0);
		expect(fixture.applyRequests).toHaveLength(0);
	} finally {
		await context.close();
	}
});

const periodChangeSetId = '60000000-0000-4000-8000-000000000201';
const periodChangeSetPath = `/api/academic/term-change-sets/${periodChangeSetId}`;

async function mockWeeklyPeriods(
	page: Page,
	options: {
		activityMode?: 'independent' | 'synchronized';
		permissions?: string[];
		published?: boolean;
		conflictOnce?: boolean;
		failSaveOnce?: boolean;
		failRefreshOnce?: boolean;
		saveGate?: Promise<void>;
		readGate?: Promise<void>;
	} = {}
) {
	const count = await mockOffering(page, {
		permissions: options.permissions ?? ['*'],
		published: options.published
	});
	const initialGraph = versionGraph(options.published);
	const teacher = {
		id: '80000000-0000-4000-8000-000000000201',
		teacherId: '90000000-0000-4000-8000-000000000202',
		displayName: 'ครูกิจกรรมที่คงไว้',
		role: 'primary' as const,
		startsOn: '2026-05-01',
		endsOn: null,
		rowVersion: 3
	};
	const activityCatalog = {
		kind: 'activity' as const,
		activityId: ids.offering,
		activityVersionId: ids.offering,
		hours: '40',
		registrationType: 'assigned',
		schedulingMode: options.activityMode ?? 'independent',
		attendanceRequirement: { minimumPercent: '80' },
		passCriteria: {
			requireAttendance: true,
			requireTeacherConfirmation: true,
			outcomes: ['pass', 'fail']
		}
	};
	let graph = {
		...initialGraph,
		effectiveFrom: options.published ? '2026-05-01' : null,
		referenceDate: '2026-05-01',
		snapshot: {
			offerings: initialGraph.snapshot.offerings.map((row) =>
				options.activityMode
					? {
							...row,
							kind: 'activity',
							name: 'กิจกรรมจิตอาสา',
							weeklyPeriodTarget: 1,
							catalog: activityCatalog,
							groups: row.groups.map((source) => ({
								...source,
								teachers: [{ ...teacher, assignmentId: teacher.id }]
							}))
						}
					: row
			)
		}
	};
	if (options.activityMode) {
		await page.route(`**${paths.offering}`, (route) =>
			fulfill(route, {
				...offering(),
				kind: 'activity',
				nameSnapshot: 'กิจกรรมจิตอาสา',
				snapshot: activityCatalog
			})
		);
		const activityGroups = [group(ids.groupA), group(ids.groupB)].map((row) => ({
			...row,
			teacherAssignments: [teacher]
		}));
		await page.route(`**${paths.groups}`, (route) => fulfill(route, activityGroups));
		for (const row of activityGroups) {
			await page.route(`**/api/academic/learning-groups/${row.id}`, (route) => fulfill(route, row));
		}
		await page.route(`**${paths.management}*`, (route) =>
			fulfill(route, {
				rooms: [],
				homerooms: [],
				teachers: [{ id: teacher.teacherId, name: teacher.displayName }]
			})
		);
	}
	const timestamp = '2026-10-27T00:00:00Z';
	const item = {
		id: '70000000-0000-4000-8000-000000000201',
		actionKind: 'adjust_weekly_period_target' as const,
		learningOfferingId: ids.offering,
		weeklyPeriodTarget: options.activityMode ? 1 : 2,
		rowVersion: 5,
		createdAt: timestamp,
		updatedAt: timestamp,
		createdBy: ids.year
	};
	let changeSet: AcademicTermChangeSet = {
		id: periodChangeSetId,
		academicYearId: ids.year,
		academicTermId: ids.term,
		status: 'draft',
		reason: 'ปรับจำนวนคาบ',
		effectiveFrom: null,
		referenceDate: '2026-05-01',
		targetDeliveryVersionId: ids.version,
		rowVersion: 7,
		changes: [],
		offeringLabels: [],
		createdAt: timestamp,
		updatedAt: timestamp,
		createdBy: ids.year,
		items: [
			{ ...item, id: 'unrelated-item', learningOfferingId: 'other-offering', rowVersion: 19 },
			item
		]
	};
	const reads: string[] = [];
	const writes: Array<{ method: string; path: string; body: Record<string, unknown> }> = [];
	let failSave = options.failSaveOnce ?? false;
	let conflict = options.conflictOnce ?? false;
	let failRefresh = options.failRefreshOnce ?? false;
	let saved = false;
	await page.route(`**${paths.versions}*`, (route) =>
		fulfill(route, [
			{
				...version(options.published),
				changeSetId: periodChangeSetId,
				effectiveFrom: graph.effectiveFrom
			}
		])
	);
	await page.route(`**${paths.version}`, async (route) => {
		reads.push(paths.version);
		if (saved && failRefresh) {
			failRefresh = false;
			await route.fulfill({
				status: 503,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'โหลดรุ่นล่าสุดไม่สำเร็จ' })
			});
			return;
		}
		await fulfill(route, graph);
	});
	await page.route(`**${periodChangeSetPath}`, async (route) => {
		expect(route.request().method()).toBe('GET');
		reads.push(periodChangeSetPath);
		if (options.readGate) await options.readGate;
		await fulfill(route, changeSet);
	});
	await page.route(`**${periodChangeSetPath}/items`, async (route) => {
		const body = route.request().postDataJSON();
		writes.push({
			method: route.request().method(),
			path: new URL(route.request().url()).pathname,
			body
		});
		if (options.saveGate) await options.saveGate;
		if (conflict || failSave) {
			const status = conflict ? 409 : 503;
			if (conflict) {
				changeSet = {
					...changeSet,
					rowVersion: 8,
					items: changeSet.items.map((row) =>
						row.id === item.id ? { ...row, rowVersion: 6 } : row
					)
				};
			}
			conflict = false;
			failSave = false;
			await route.fulfill({
				status,
				contentType: 'application/json',
				body: JSON.stringify({
					success: false,
					error: status === 409 ? 'แบบร่างถูกแก้ไขจากที่อื่น' : 'บันทึกยังไม่สำเร็จ'
				})
			});
			return;
		}
		expect(route.request().method()).toBe('PUT');
		expect(body.action).toBe('adjust_weekly_period_target');
		expect(body.changeSetRowVersion).toBe(changeSet.rowVersion);
		const existing = changeSet.items.find((row) => row.id === item.id)!;
		expect(body.itemRowVersion).toBe(existing.rowVersion);
		expect(body.learningOfferingId).toBe(ids.offering);
		changeSet = {
			...changeSet,
			rowVersion: changeSet.rowVersion + 1,
			items: changeSet.items.map((row) =>
				row.id === item.id
					? { ...item, rowVersion: row.rowVersion + 1, weeklyPeriodTarget: body.weeklyPeriodTarget }
					: row
			)
		};
		graph = {
			...graph,
			rowVersion: graph.rowVersion + 1,
			snapshot: {
				offerings: graph.snapshot.offerings.map((row) => ({
					...row,
					weeklyPeriodTarget: body.weeklyPeriodTarget
				}))
			}
		};
		saved = true;
		await fulfill(route, changeSet);
	});
	return { count, reads, writes, changeSet: () => changeSet, graph: () => graph };
}

for (const mode of ['independent', 'synchronized'] as const) {
	for (const scenario of [
		{ width: 1280, dark: false, conflict: false },
		{ width: 390, dark: true, conflict: true }
	]) {
		test(`zero weekly periods keep ${mode} activity open through ${scenario.conflict ? 'conflict' : 'failed save'} at ${scenario.width}px ${scenario.dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width: scenario.width, height: 900 });
			const gate = deferred();
			const fixture = await mockWeeklyPeriods(page, {
				activityMode: mode,
				conflictOnce: scenario.conflict,
				failSaveOnce: !scenario.conflict,
				failRefreshOnce: true
			});
			const initial = structuredClone(fixture.graph().snapshot.offerings[0]);
			await page.goto(url());
			if (scenario.dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
				await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			}
			const groups = page.getByTestId('delivery-groups-ready');
			await expect(groups.getByRole('button', { name: /^กลุ่ม ก ครู / })).toBeVisible();
			await expect(groups.getByRole('button', { name: /^กลุ่ม ข ครู / })).toBeVisible();
			const trigger = page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true });
			await trigger.focus();
			await page.keyboard.press('Enter');
			const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
			const input = dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true });
			await expect(input).toHaveValue('1');
			await expect(input).toHaveAttribute('min', '0');
			await expect(dialog).toContainText('0 คาบ = เปิดกิจกรรมโดยไม่จัดคาบประจำสัปดาห์ในตารางสอน');
			await expect(dialog).toContainText('ยังประเมินผ่าน/ไม่ผ่านตามปกติ');
			await input.fill('-1');
			const save = dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true });
			await expect(save).toBeDisabled();
			await input.fill('0.5');
			await expect(save).toBeDisabled();
			await input.fill('0');
			await input.press('Enter');
			await expect(dialog.getByRole('alert')).toHaveText(
				scenario.conflict
					? 'แบบร่างถูกแก้ไขจากที่อื่น กรุณาโหลดข้อมูลล่าสุดหรือสร้างตัวอย่างใหม่ก่อนดำเนินการ'
					: 'บันทึกยังไม่สำเร็จ'
			);
			await expect(input).toHaveValue('0');
			expect(fixture.writes).toHaveLength(1);
			expect(fixture.writes[0].body).toEqual({
				action: 'adjust_weekly_period_target',
				changeSetRowVersion: 7,
				itemRowVersion: 5,
				learningOfferingId: ids.offering,
				weeklyPeriodTarget: 0
			});
			if (scenario.conflict) {
				await expect(save).toBeDisabled();
				await dialog
					.getByRole('button', { name: 'โหลดข้อมูลล่าสุดก่อนบันทึก', exact: true })
					.click();
				await expect(dialog.getByRole('alert')).toHaveCount(0);
				await expect(input).toHaveValue('0');
				expect(fixture.writes).toHaveLength(1);
			}
			await page.route(`**${periodChangeSetPath}/items`, async (route) => {
				await gate.promise;
				await route.fallback();
			});
			try {
				await save.click();
				await expect(
					dialog.getByRole('button', { name: 'กำลังบันทึก', exact: true })
				).toBeDisabled();
				await expect(input).toBeDisabled();
				await page.keyboard.press('Enter');
				await page.keyboard.press('Escape');
				await expect(dialog).toBeVisible();
				expect(fixture.writes).toHaveLength(1);
			} finally {
				gate.release();
			}
			await expect(dialog.getByRole('status')).toContainText('บันทึกจำนวนคาบแล้ว');
			await expect(dialog.getByRole('alert')).toHaveText('โหลดรุ่นล่าสุดไม่สำเร็จ');
			await expect(input).toHaveValue('0');
			await expect(save).toBeDisabled();
			expect(fixture.writes).toHaveLength(2);
			expect(fixture.writes[1].body).toEqual({
				action: 'adjust_weekly_period_target',
				changeSetRowVersion: scenario.conflict ? 8 : 7,
				itemRowVersion: scenario.conflict ? 6 : 5,
				learningOfferingId: ids.offering,
				weeklyPeriodTarget: 0
			});
			await dialog.getByRole('button', { name: 'โหลดจำนวนคาบที่บันทึกแล้ว', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(trigger).toBeFocused();
			await expect(page.getByTestId('delivery-versions-ready')).toContainText('0 คาบ/สัปดาห์');
			expect(fixture.graph().snapshot.offerings[0]).toEqual({ ...initial, weeklyPeriodTarget: 0 });
			await expect(groups.getByRole('button', { name: /^กลุ่ม ก ครู / })).toContainText('ครู 1 คน');
			await expect(groups.getByRole('button', { name: /^กลุ่ม ข ครู / })).toContainText('ครู 1 คน');
			await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
			await expect(page.getByText('ครูกิจกรรมที่คงไว้', { exact: true })).toBeVisible();
			expect(fixture.writes).toHaveLength(2);
			await page.screenshot({
				path: testInfo.outputPath('activity-zero-weekly-periods.png'),
				animations: 'disabled'
			});
		});
	}
}

for (const width of [390, 1280]) {
	for (const dark of [false, true]) {
		test(`offering period dialog edits the selected draft and restores focus and native scrolling at ${width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			const fixture = await mockWeeklyPeriods(page);
			await page.goto(url());
			if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
			if (dark) await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
			const card = page.getByTestId('delivery-versions-ready');
			await expect(card).toContainText('เลือกวันเริ่มใช้ตอนเผยแพร่');
			await expect(card).not.toContainText('null');
			await expect(page.getByTestId('delivery-version-selector')).not.toContainText('null');
			expect(fixture.reads.filter((path) => path === periodChangeSetPath)).toHaveLength(0);
			const trigger = card.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true });
			await trigger.focus();
			await page.keyboard.press('Enter');
			const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
			await expect(dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true })).toHaveValue('2');
			await expect(dialog).toHaveCSS('opacity', '1');
			expect(fixture.writes).toHaveLength(0);
			await page.screenshot({
				path: testInfo.outputPath(`offering-periods-${width}-${dark ? 'dark' : 'light'}.png`),
				animations: 'disabled'
			});
			const bounds = await dialog.boundingBox();
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(900);
			await page.keyboard.press('Escape');
			await expect(dialog).toHaveCount(0);
			await expect(trigger).toBeFocused();
			const main = page.locator('main');
			await main.evaluate((element) => {
				element.scrollTop = 0;
			});
			await page.mouse.move(width - 20, 500);
			await page.mouse.wheel(0, 500);
			await expect.poll(() => main.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
			await trigger.click();
			await dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true }).fill('0');
			await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(card).toContainText('0 คาบ/สัปดาห์');
			await expect(trigger).toBeFocused();
			expect(fixture.writes).toEqual([
				{
					method: 'PUT',
					path: `${periodChangeSetPath}/items`,
					body: {
						action: 'adjust_weekly_period_target',
						changeSetRowVersion: 7,
						itemRowVersion: 5,
						learningOfferingId: ids.offering,
						weeklyPeriodTarget: 0
					}
				}
			]);
			expect(fixture.changeSet().items[0].rowVersion).toBe(19);
			expect(fixture.count(paths.offering)).toBe(1);
			expect(fixture.count(paths.groups)).toBe(1);
			await trigger.click();
			await expect(dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true })).toHaveValue('0');
			await dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true }).fill('4');
			await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
			await expect(card).toContainText('4 คาบ/สัปดาห์');
			expect(fixture.writes[1].body).toMatchObject({
				changeSetRowVersion: 8,
				itemRowVersion: 6,
				weeklyPeriodTarget: 4
			});
		});
	}
}

for (const conflict of [false, true]) {
	test(`offering period ${conflict ? 'conflict' : 'network failure'} retains input and saves only after explicit retry`, async ({
		page
	}) => {
		const fixture = await mockWeeklyPeriods(page, {
			conflictOnce: conflict,
			failSaveOnce: !conflict
		});
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
		const input = dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true });
		await input.fill('6');
		await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
		await expect(dialog.getByRole('alert')).toBeVisible();
		await expect(input).toHaveValue('6');
		expect(fixture.writes).toHaveLength(1);
		if (conflict) {
			await expect(
				dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true })
			).toBeDisabled();
			await dialog.getByRole('button', { name: 'โหลดข้อมูลล่าสุดก่อนบันทึก', exact: true }).click();
			await expect(dialog.getByRole('alert')).toHaveCount(0);
			await expect(input).toHaveValue('6');
		}
		await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
		await expect(dialog).toHaveCount(0);
		await expect(page.getByTestId('delivery-versions-ready')).toContainText('6 คาบ/สัปดาห์');
		expect(fixture.writes[1].body).toMatchObject({
			changeSetRowVersion: conflict ? 8 : 7,
			itemRowVersion: conflict ? 6 : 5,
			weeklyPeriodTarget: 6
		});
	});
}

test('a saved period change with a failed refresh retries only the read', async ({ page }) => {
	const fixture = await mockWeeklyPeriods(page, { failRefreshOnce: true });
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
	await dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true }).fill('3');
	await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
	await expect(dialog.getByRole('status')).toContainText('บันทึกจำนวนคาบแล้ว');
	await expect(dialog.getByRole('alert')).toContainText('โหลดรุ่นล่าสุดไม่สำเร็จ');
	await expect(dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: 'โหลดจำนวนคาบที่บันทึกแล้ว', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByTestId('delivery-versions-ready')).toContainText('3 คาบ/สัปดาห์');
	expect(fixture.writes).toHaveLength(1);
});

test('cancelling a pending period read prevents late responses from reopening the dialog', async ({
	page
}) => {
	const gate = deferred();
	const fixture = await mockWeeklyPeriods(page, { readGate: gate.promise });
	await page.goto(url());
	const trigger = page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true });
	try {
		await trigger.click();
		const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
		await expect
			.poll(() => fixture.reads.filter((path) => path === periodChangeSetPath).length)
			.toBe(1);
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await expect(dialog).toHaveCount(0);
		await expect(trigger).toBeFocused();
	} finally {
		gate.release();
	}
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(fixture.writes).toHaveLength(0);
	await trigger.click();
	await expect(
		page.getByRole('dialog').getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true })
	).toHaveValue('2');
});

test('pending period save prevents dismissal and duplicate writes', async ({ page }) => {
	const gate = deferred();
	const fixture = await mockWeeklyPeriods(page, { saveGate: gate.promise });
	try {
		await page.goto(url());
		await page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: 'แก้ไขจำนวนคาบต่อสัปดาห์', exact: true });
		await dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์', { exact: true }).fill('3');
		await dialog.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
		await expect(dialog.getByRole('button', { name: 'กำลังบันทึก', exact: true })).toBeDisabled();
		await page.keyboard.press('Escape');
		await expect(dialog).toBeVisible();
		expect(fixture.writes).toHaveLength(1);
	} finally {
		gate.release();
	}
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

for (const published of [false, true]) {
	test(`period editing stays hidden for ${published ? 'published snapshots' : 'read-only users'}`, async ({
		page
	}) => {
		const fixture = await mockWeeklyPeriods(page, {
			published,
			permissions: published ? ['*'] : ['learning_offering.read.school']
		});
		await page.goto(url());
		await expect(
			page.getByTestId(published ? 'delivery-version-offering' : 'delivery-offering-ready')
		).toBeVisible();
		await expect(page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true })).toHaveCount(0);
		expect(fixture.reads.filter((path) => path === periodChangeSetPath)).toHaveLength(0);
		expect(fixture.writes).toHaveLength(0);
	});
}

test('bulk tracking dates retain custom choices through mode flips and refresh each fresh opening', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	const fixture = await mockBulkSetup(page, { conflictOnce: true });
	await page.goto(url());
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await dialog.getByRole('checkbox', { name: /ม.1\/1/ }).check();
	await dialog.getByRole('checkbox', { name: 'กำหนดวิธีจัดรายชื่อ', exact: true }).check();
	await dialog.getByRole('radio', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).check();
	const date = dialog.getByRole('button', {
		name: 'วันที่เริ่มติดตามสำหรับกลุ่มเป้าหมาย',
		exact: true
	});
	await expect(date).toContainText('27 ต.ค. 2569');
	await date.click();
	await page.locator('[data-bits-day][data-today]').focus();
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('Enter');
	await expect(date).toContainText('28 ต.ค. 2569');
	await dialog.getByRole('radio', { name: 'จัดรายชื่อเอง', exact: true }).check();
	await dialog.getByRole('radio', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).check();
	await expect(date).toContainText('28 ต.ค. 2569');
	await dialog.getByRole('button', { name: 'บันทึกให้กลุ่มที่เลือก', exact: true }).click();
	await expect(dialog.getByRole('alert')).toBeVisible();
	await dialog.getByRole('button', { name: 'โหลดกลุ่มล่าสุด', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveCount(0);
	await expect(date).toContainText('28 ต.ค. 2569');
	expect(fixture.applyRequests[0].rosterTracking?.effectiveFrom).toBe('2026-10-28');
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.clock.setFixedTime(new Date('2026-10-29T05:00:00Z'));
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	await dialog.getByRole('checkbox', { name: 'กำหนดวิธีจัดรายชื่อ', exact: true }).check();
	await dialog.getByRole('radio', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).check();
	await expect(date).toContainText('29 ต.ค. 2569');
});

test('single and bulk tracking defaults respect the already-loaded future teaching start', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	await mockBulkSetup(page, { trackingStartsOn: '2026-11-01' });
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	await panel.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
	await page.getByRole('option', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).click();
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'1 พ.ย. 2569'
	);
	await page.getByRole('button', { name: 'ตั้งค่าหลายกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: /ตั้งค่าหลายกลุ่ม/ });
	await dialog.getByRole('checkbox', { name: 'กำหนดวิธีจัดรายชื่อ', exact: true }).check();
	await dialog.getByRole('radio', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).check();
	await expect(
		dialog.getByRole('button', { name: 'วันที่เริ่มติดตามสำหรับกลุ่มเป้าหมาย', exact: true })
	).toContainText('1 พ.ย. 2569');
});

test('rendering an existing past tracking date preserves the saved configuration', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-27T05:00:00Z'));
	const writes: string[] = [];
	page.on('request', (request) => {
		if (request.method() !== 'GET') writes.push(request.url());
	});
	await mockOffering(page, { permissions: ['*'], trackingMode: 'homeroom' });
	await page.goto(url());
	const panel = page.getByTestId('roster-tracking');
	await expect(panel.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })).toContainText(
		'9 ต.ค. 2569'
	);
	await expect(
		panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true })
	).toBeDisabled();
	await expect(panel).toContainText('สมาชิกใหม่เริ่มนับจากวันที่นี้');
	await expect(panel).toContainText('กลุ่มร่างต้องเตรียมและเผยแพร่รายชื่อก่อนติดตามจริง');
	expect(writes).toEqual([]);
});
