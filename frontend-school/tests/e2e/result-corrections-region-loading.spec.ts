import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';

type Schemas = components['schemas'];
test.use({ serviceWorkers: 'block' });

const ids = {
	year: '10000000-0000-4000-8000-000000000001',
	term: '20000000-0000-4000-8000-000000000001',
	user: '30000000-0000-4000-8000-000000000001',
	student: '40000000-0000-4000-8000-000000000001',
	result: '50000000-0000-4000-8000-000000000001',
	group: '60000000-0000-4000-8000-000000000001',
	offering: '70000000-0000-4000-8000-000000000001'
};

const routeUrl = `/staff/academic/result-corrections?academicYearId=${ids.year}&academicTermId=${ids.term}`;
const initialValue = { kind: 'course' as const, outcome: 'numeric' as const, numericGrade: '2' };
const initialResult: Schemas['EffectiveResult'] = {
	resultId: ids.result,
	studentAcademicYearId: ids.student,
	initial: initialValue,
	effective: initialValue,
	effectiveVersion: 1,
	corrections: []
};
const item: Schemas['EffectiveResultSearchItem'] = {
	kind: 'course',
	studentCode: '001',
	displayName: 'นักเรียน ทดสอบ',
	learningGroupId: ids.group,
	learningOfferingId: ids.offering,
	offeringCode: 'ค21101',
	offeringName: 'คณิตศาสตร์',
	groupName: 'ม.1/1',
	result: initialResult
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(
	page: Page,
	options: { holdFirstRead?: boolean; emptyFirstRead?: boolean; holdSecondRead?: boolean } = {}
) {
	let reads = 0;
	let writes = 0;
	let releaseFirstRead = () => {};
	let releaseSecondRead = () => {};
	const firstReadGate = new Promise<void>((resolve) => {
		releaseFirstRead = resolve;
	});
	const secondReadGate = new Promise<void>((resolve) => {
		releaseSecondRead = resolve;
	});
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.user,
					username: 'academic',
					firstName: 'ฝ่าย',
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-09-01T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['academic_result.correct.school']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: ids.year,
					activeAcademicTermId: ids.term,
					years: [
						{
							id: ids.year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-03-31'
						}
					],
					terms: [
						{
							id: ids.term,
							academicYearId: ids.year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2026-10-31',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/results/effective') {
				reads += 1;
				if (options.holdFirstRead && reads === 1) await firstReadGate;
				if (options.holdSecondRead && reads === 2) await secondReadGate;
				await fulfill(
					route,
					options.emptyFirstRead && reads === 1
						? []
						: [
								{
									...item,
									displayName: url.searchParams.get('search') ? 'นักเรียน ใหม่' : item.displayName
								}
							]
				);
				return;
			}
			if (url.pathname === '/api/academic/results/corrections' && request.method() === 'POST') {
				writes += 1;
				await fulfill(route, {
					...initialResult,
					effective: { kind: 'course', outcome: 'numeric', numericGrade: '3' },
					effectiveVersion: 2
				});
				return;
			}
			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}
			if (url.pathname === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			if (url.pathname === '/api/me/work-items/counts') {
				await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
				return;
			}
			if (url.pathname === '/api/notifications') {
				await fulfill(route, { items: [], unread_count: 0 });
				return;
			}
			if (url.pathname === '/api/school/settings') {
				await fulfill(route, 'forbidden', 403);
				return;
			}
			await fulfill(route, {});
		}
	);
	return { releaseFirstRead, releaseSecondRead, counts: () => ({ reads, writes }) };
}

test('initial correction search renders its skeleton and URL-owned filters', async ({ page }) => {
	const observed = await mock(page, { holdFirstRead: true });
	await page.goto(routeUrl);
	await expect(page.getByLabel('ค้นหานักเรียนหรือรายวิชา')).toBeVisible();
	await expect(page.getByText('นักเรียน ทดสอบ')).toHaveCount(0);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	observed.releaseFirstRead();
	await expect(page.getByText('นักเรียน ทดสอบ')).toBeVisible();
	await page.getByLabel('ค้นหานักเรียนหรือรายวิชา').fill('ใหม่');
	await page.getByRole('button', { name: 'ค้นหา' }).click();
	await expect(page).toHaveURL(/search=%E0%B9%83%E0%B8%AB%E0%B8%A1%E0%B9%88/);
	await expect(page.getByText('นักเรียน ใหม่')).toBeVisible();
	expect(observed.counts()).toEqual({ reads: 2, writes: 0 });
});

test('a late initial search keeps its URL query key when the user has typed a new filter', async ({
	page
}) => {
	const observed = await mock(page, { holdFirstRead: true, holdSecondRead: true });
	await page.goto(routeUrl);
	await page.getByLabel('ค้นหานักเรียนหรือรายวิชา').fill('ใหม่');
	observed.releaseFirstRead();
	await expect(page.getByText('นักเรียน ทดสอบ')).toBeVisible();
	await page.getByRole('button', { name: 'ค้นหา' }).click();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await expect(page.getByText('นักเรียน ทดสอบ')).toHaveCount(0);
	observed.releaseSecondRead();
	await expect(page.getByText('นักเรียน ใหม่')).toBeVisible();
});

test('empty correction results remain a usable region during same-query refresh', async ({
	page
}) => {
	const observed = await mock(page, { emptyFirstRead: true, holdSecondRead: true });
	await page.goto(routeUrl);
	await expect(page.getByText('ยังไม่พบผลที่ล็อกแล้ว')).toBeVisible();
	await page.getByRole('button', { name: 'ค้นหา' }).click();
	await expect(page.getByText('กำลังอัปเดตผลที่ค้นพบ...')).toBeVisible();
	await expect(page.getByText('ยังไม่พบผลที่ล็อกแล้ว')).toBeVisible();
	observed.releaseSecondRead();
	await expect(page.getByText('นักเรียน ทดสอบ')).toBeVisible();
});

test('correction patches the returned result without a broad search refetch', async ({ page }) => {
	const observed = await mock(page);
	await page.goto(routeUrl);
	await expect(page.getByText('นักเรียน ทดสอบ')).toBeVisible();
	await page.getByRole('button', { name: 'แก้ผล' }).click();
	await page.getByRole('button', { name: 'ผลใหม่', exact: true }).click();
	await page.getByRole('option', { name: '3', exact: true }).click();
	await page.getByRole('button', { name: /บันทึก/ }).click();
	await expect(page.getByRole('row', { name: /นักเรียน ทดสอบ/ })).toContainText('3');
	expect(observed.counts()).toEqual({ reads: 1, writes: 1 });
});
