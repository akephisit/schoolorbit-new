import { expect, test, type Page, type Route } from '@playwright/test';
import { RoutePerformanceProbe, summarizeFiveWarmRuns } from './helpers/route-performance';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const yearId = '10000000-0000-4000-8000-000000000001';
const termId = '20000000-0000-4000-8000-000000000001';
const nextTermId = '20000000-0000-4000-8000-000000000002';
const deliveryPath = `/staff/academic/delivery?academicYearId=${yearId}&academicTermId=${termId}`;

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

test('a late old-term homeroom result never replaces the selected term', async ({ page }) => {
	const gate = deferred();
	const requests = await mockDelivery(page, 'homerooms', gate.promise);
	try {
		await page.goto(deliveryPath);
		await expect(page.getByRole('region', { name: 'มุมมองรายห้อง' })).toHaveAttribute(
			'aria-busy',
			'true'
		);
		await page.getByLabel('เลือกภาคเรียน', { exact: true }).click();
		await page.getByRole('option', { name: 'ภาคเรียนใหม่' }).click();
		await expect(page).toHaveURL(new RegExp(`academicTermId=${nextTermId}`));
		await expect(page.getByText('ห้องภาคใหม่', { exact: true })).toBeVisible();
		expect(requests.homerooms).toBe(2);
	} finally {
		gate.release();
	}
	await requests.oldHomeroomSettled;
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await expect(page.getByText('ห้องภาคแรก', { exact: true })).toHaveCount(0);
});

async function fulfill(route: Route, data: unknown, headers?: Record<string, string>) {
	await route.fulfill({
		contentType: 'application/json',
		headers,
		body: JSON.stringify({ success: true, data })
	});
}

function homeroomWorkspace(academicTermId: string) {
	return {
		academicYearId: yearId,
		academicTermId,
		timetableVersionId: null,
		timetableVersionStatus: null,
		timetableVersionEffectiveFrom: null,
		homerooms: [
			{
				homeroom: {
					id: '60000000-0000-4000-8000-000000000001',
					name: academicTermId === termId ? 'ห้องภาคแรก' : 'ห้องภาคใหม่',
					gradeLevel: 'มัธยมศึกษาปีที่ 1',
					gradeLevelId: '30000000-0000-4000-8000-000000000001'
				},
				gradeLevel: {
					id: '30000000-0000-4000-8000-000000000001',
					code: 'M1',
					name: 'มัธยมศึกษาปีที่ 1',
					short_name: 'ม.1',
					level_type: 'secondary',
					level_order: 301
				},
				studyProgram: {
					id: '40000000-0000-4000-8000-000000000001',
					code: 'DEFAULT',
					name: 'แผนมาตรฐาน',
					curriculumId: '50000000-0000-4000-8000-000000000001',
					curriculumName: 'หลักสูตรทดสอบ'
				},
				curriculumVersionId: '50000000-0000-4000-8000-000000000001',
				expectedCount: 0,
				readyCount: 0,
				blockers: [],
				extraOfferings: [],
				items: []
			}
		],
		unlinked: []
	};
}

async function mockDelivery(page: Page, delayed: 'homerooms' | 'changes', gate: Promise<void>) {
	const oldHomeroomSettled = deferred();
	const requests = {
		homerooms: 0,
		changes: 0,
		workspace: 0,
		pageView: 0,
		oldHomeroomSettled: oldHomeroomSettled.promise
	};
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			switch (url.pathname) {
				case '/api/auth/me':
					await fulfill(route, {
						id: '90000000-0000-4000-8000-000000000001',
						username: 'route-region-test',
						firstName: 'ทดสอบ',
						lastName: 'การโหลด',
						userType: 'staff',
						status: 'ACTIVE',
						permissions: ['*']
					});
					return;
				case '/api/academic/context/options':
					await fulfill(route, {
						activeAcademicYearId: yearId,
						activeAcademicTermId: termId,
						years: [{ id: yearId, name: 'ปีทดสอบ', year: 2570, status: 'active' }],
						terms: [
							{
								id: termId,
								academicYearId: yearId,
								name: 'ภาคเรียนทดสอบ',
								code: '1',
								sequence: 1,
								termType: 'regular',
								status: 'active'
							},
							{
								id: nextTermId,
								academicYearId: yearId,
								name: 'ภาคเรียนใหม่',
								code: '2',
								sequence: 2,
								termType: 'regular',
								status: 'active'
							}
						]
					});
					return;
				case '/api/menu/user':
					await fulfill(route, { groups: [] });
					return;
				case '/api/academic/delivery/homerooms': {
					requests.homerooms += 1;
					const requestedTermId = url.searchParams.get('academicTermId') ?? termId;
					try {
						if (delayed === 'homerooms' && requestedTermId === termId) await gate;
						await fulfill(route, homeroomWorkspace(requestedTermId), {
							'server-timing': 'authorization;dur=4.0, resources;dur=12.0, total;dur=20.0'
						});
					} finally {
						if (requestedTermId === termId) oldHomeroomSettled.release();
					}
					return;
				}
				case '/api/academic/term-change-sets':
					requests.changes += 1;
					if (delayed === 'changes') await gate;
					await fulfill(route, []);
					return;
				case '/api/academic/delivery/workspace':
					requests.workspace += 1;
					await fulfill(route, { academicTermId: termId, offerings: [] });
					return;
				case '/api/academic/delivery/page-view':
					requests.pageView += 1;
					await fulfill(route, {});
					return;
				default:
					await fulfill(route, {});
			}
		}
	);
	return requests;
}

for (const delayed of ['homerooms', 'changes'] as const) {
	test(`Delivery paints the ready region while ${delayed} is unresolved`, async ({ page }) => {
		const gate = deferred();
		const requests = await mockDelivery(page, delayed, gate.promise);
		const readyTestId =
			delayed === 'homerooms' ? 'delivery-change-set-ready' : 'delivery-homerooms-ready';
		const waitingTestId =
			delayed === 'homerooms' ? 'delivery-homerooms-ready' : 'delivery-change-set-ready';
		try {
			await page.goto(deliveryPath);
			await expect(page.getByTestId(readyTestId)).toBeVisible();
			await expect(page.getByTestId(waitingTestId)).toHaveCount(0);
			expect(requests.homerooms).toBe(1);
			expect(requests.changes).toBe(1);
			expect(requests.workspace).toBe(0);
			expect(requests.pageView).toBe(0);
		} finally {
			gate.release();
		}
		await expect(page.getByTestId(waitingTestId)).toBeVisible();
		await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม' }).click();
		await expect.poll(() => requests.workspace).toBe(1);
		expect(requests.pageView).toBe(0);
	});
}

test('reports sanitized medians from five warm Delivery navigations', async ({ page }) => {
	await mockDelivery(page, 'homerooms', Promise.resolve());
	expect(() => new RoutePerformanceProbe(page, `staff/academic/delivery/${yearId}`, [])).toThrow(
		'inventoried route template'
	);
	const probe = new RoutePerformanceProbe(page, 'staff/academic/delivery', [
		{
			region: 'homerooms',
			apiPath: '/api/academic/delivery/homerooms',
			readyTestId: 'delivery-homerooms-ready'
		}
	]);
	const samples = [];
	try {
		for (let index = 0; index < 5; index += 1) {
			probe.beginNavigation();
			await page.goto(deliveryPath, { waitUntil: 'commit' });
			samples.push(await probe.sample('homerooms'));
		}
	} finally {
		probe.dispose();
	}
	const summary = summarizeFiveWarmRuns(samples);
	expect(summary.route).toBe('staff/academic/delivery');
	expect(summary.region).toBe('homerooms');
	expect(summary.navigationToRequestMs).toBeGreaterThanOrEqual(0);
	expect(summary.requestDurationMs).toBeGreaterThanOrEqual(0);
	expect(summary.firstUsefulRegionMs).toBeGreaterThanOrEqual(summary.navigationToRequestMs);
	expect(summary.responseBytes).toBeGreaterThan(0);
	expect(summary.serverTiming.total).toBe(20);
	expect(JSON.stringify(summary)).not.toContain(yearId);
	expect(JSON.stringify(summary)).not.toContain(termId);
	console.info(`Mocked route-region median: ${JSON.stringify(summary)}`);
	await test.info().attach('route-region-summary.json', {
		body: JSON.stringify(summary),
		contentType: 'application/json'
	});
});
