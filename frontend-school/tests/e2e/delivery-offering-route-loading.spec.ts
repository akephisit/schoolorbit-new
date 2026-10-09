import { expect, test, type Page, type Route } from '@playwright/test';

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
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: options.permissions ?? ['learning_offering.read.school']
				});
				return;
			}
			if (path === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: ids.year,
					activeAcademicTermId: ids.term,
					years: [{ id: ids.year, name: 'ปีการศึกษา 2569', year: 2569, status: 'active' }],
					terms: [
						{
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
		await panel.getByRole('button', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
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
		await panel.getByRole('button', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
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
	await panel.getByRole('button', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
	await page.getByRole('option', { name: 'ติดตามห้องอัตโนมัติ', exact: true }).click();
	const date = await panel
		.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })
		.innerText();
	await panel.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
	await expect(panel.getByRole('button', { name: 'กำลังบันทึก', exact: true })).toBeDisabled();
	gate.release();
	await expect(panel.getByRole('alert')).toContainText('กลุ่มนี้เปลี่ยนแล้ว');
	await expect(panel.getByRole('button', { name: 'วิธีจัดรายชื่อ', exact: true })).toContainText(
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
	const mode = panel.getByRole('button', { name: 'วิธีจัดรายชื่อ', exact: true });
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
