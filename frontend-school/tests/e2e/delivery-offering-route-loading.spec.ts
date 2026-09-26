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
	versions: '/api/academic/timetable-versions',
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
			gradingPolicy: { policyCode: 'school_default', totalScore: '100.00', passingScore: '50.00' }
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
		homeroomIds: [],
		preferredRoomIds: [],
		rowVersion: 1
	};
}

function version() {
	return {
		id: ids.version,
		academicTermId: ids.term,
		academicYearId: ids.year,
		status: 'published',
		displayState: 'current',
		effectiveFrom: '2026-05-01',
		targets: [
			{
				timetableVersionId: ids.version,
				learningOfferingId: ids.offering,
				standardPeriodsPerWeek: 2,
				weeklyPeriodTarget: 2
			}
		]
	};
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
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
	} = {}
) {
	const counts = new Map<string, number>();
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
			if (path === paths.offering) return void (await fulfill(route, offering()));
			if (path === paths.groups)
				return void (await fulfill(route, [
					group(ids.groupA, options.published),
					group(ids.groupB)
				]));
			if (path === paths.versions) return void (await fulfill(route, [version()]));
			if (path === paths.groupA)
				return void (await fulfill(route, group(ids.groupA, options.published)));
			if (path === paths.groupB) return void (await fulfill(route, group(ids.groupB)));
			if (path === paths.membershipsA) return void (await fulfill(route, []));
			if (path === paths.management)
				return void (await fulfill(route, { rooms: [], teachers: [], homerooms: [] }));
			await fulfill(route, {});
		}
	);
	return (path: string) => counts.get(path) ?? 0;
}

function url(groupId: string | null = ids.groupA) {
	return `/staff/academic/delivery/${ids.offering}?academicYearId=${ids.year}&academicTermId=${ids.term}${groupId ? `&groupId=${groupId}` : ''}`;
}

test('group list and direct group detail paint while offering is delayed', async ({ page }) => {
	const gate = deferred();
	const count = await mockOffering(page, { gate: { path: paths.offering, promise: gate.promise } });
	try {
		await page.goto(url());
		await expect(page.getByTestId('delivery-groups-ready')).toBeVisible();
		await expect(page.getByTestId('delivery-selected-group-ready')).toBeVisible();
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
		await expect(page.getByTestId('delivery-selected-group-ready')).toBeVisible();
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
