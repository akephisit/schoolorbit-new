import { expect, test, type Page, type Route } from '@playwright/test';

import {
	makeStructuralTimetableBlock,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });

const academicYearId = '20000000-0000-4000-8000-000000000001';
const academicTermId = '30000000-0000-4000-8000-000000000001';
const configuredPeriodEntries = [
	makeStructuralTimetableBlock(timetableIds.blockA, timetableIds.period1, 'ช่วงเตรียมการ 1', [
		timetableIds.teacherA
	]),
	makeStructuralTimetableBlock(timetableIds.blockB, timetableIds.period2, 'ช่วงเตรียมการ 2', [
		timetableIds.teacherA
	]),
	makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)
];

function fulfillJson(route: Route, data: unknown) {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function mockStaffTimetableApis(page: Page, items: unknown[]) {
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfillJson(route, {
					id: '10000000-0000-4000-8000-000000000001',
					username: 'teacher1',
					firstName: 'สายใจ',
					lastName: 'วิทยา',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-08-07T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: []
				});
				return;
			}

			if (url.pathname === '/api/academic/context/options') {
				await fulfillJson(route, {
					years: [
						{
							id: academicYearId,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							startDate: '2026-05-01',
							endDate: '2027-03-31',
							status: 'active'
						}
					],
					terms: [
						{
							id: academicTermId,
							academicYearId,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							startDate: '2026-05-01',
							endDate: '2026-10-31',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					],
					activeAcademicYearId: academicYearId,
					activeAcademicTermId: academicTermId
				});
				return;
			}

			if (url.pathname === '/api/me/timetable') {
				await fulfillJson(route, items);
				return;
			}

			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}

			if (url.pathname === '/api/school/settings') {
				await route.fulfill({
					status: 403,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'forbidden' })
				});
				return;
			}

			if (url.pathname === '/api/menu/user') {
				await fulfillJson(route, { groups: [] });
				return;
			}

			if (url.pathname === '/api/me/work-items/counts') {
				await fulfillJson(route, {
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
				await fulfillJson(route, { items: [], unread_count: 0 });
				return;
			}

			if (url.pathname === '/api/school/public') {
				await fulfillJson(route, { schoolName: 'ซับน้อยเหนือวิทยาคม' });
				return;
			}

			await fulfillJson(route, {});
		}
	);
}

async function expectConfiguredGrid(page: Page) {
	await expect(page.getByRole('columnheader', { name: /คาบ 1/ })).toBeVisible();
	await expect(page.getByRole('columnheader', { name: /คาบ 2/ })).toBeVisible();
	await expect(page.getByRole('columnheader', { name: /คาบ 3/ })).toBeVisible();
	await expect(page.getByText('จันทร์', { exact: true })).toBeVisible();
	await expect(page.getByText('เสาร์', { exact: true })).toHaveCount(0);
	await expect(page.getByText('อาทิตย์', { exact: true })).toHaveCount(0);
}

test('shows configured periods that precede the teacher first lesson', async ({ page }) => {
	await mockStaffTimetableApis(page, configuredPeriodEntries);

	await page.goto('/staff/timetable');

	await expectConfiguredGrid(page);
	await expect(page.getByText('ค21101')).toBeVisible();
});

test('shows an explicit empty state when the teacher has no lessons', async ({ page }) => {
	await mockStaffTimetableApis(page, []);

	await page.goto('/staff/timetable');

	await expect(page.getByText('ยังไม่มีตารางสอน', { exact: true })).toBeVisible();
	await expect(page.getByText('ยังไม่มีคาบสอนของคุณในภาคเรียนนี้')).toBeVisible();
});

test('shows compact multiline shared cards without codes teachers or shared labels', async ({
	page
}) => {
	const structuralTitle = 'ประชุมครู\nประจำเดือน';
	const synchronizedTitle = 'ชุมนุม\nถ่ายภาพ';
	const structuralBlock = makeStructuralTimetableBlock(
		timetableIds.blockA,
		timetableIds.period1,
		structuralTitle,
		[timetableIds.teacherA]
	);
	const synchronizedBlock = {
		...makeSynchronizedTimetableBlock(timetableIds.blockB, timetableIds.period2, [
			timetableIds.teacherB
		]),
		offeringName: synchronizedTitle
	};
	const courseBlock = makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3);
	await mockStaffTimetableApis(page, [structuralBlock, synchronizedBlock, courseBlock]);

	await page.goto('/staff/timetable');

	const structuralTitleElement = page.getByText(structuralTitle, { exact: true });
	await expect(structuralTitleElement).toBeVisible();
	expect(
		await structuralTitleElement.evaluate((element) => ({
			whiteSpace: getComputedStyle(element).whiteSpace,
			lineClamp: getComputedStyle(element).webkitLineClamp
		}))
	).toEqual({ whiteSpace: 'pre-line', lineClamp: '3' });

	const synchronizedTitleElement = page.getByText(synchronizedTitle, { exact: true });
	await expect(synchronizedTitleElement).toBeVisible();
	expect(
		await synchronizedTitleElement.evaluate((element) => ({
			whiteSpace: getComputedStyle(element).whiteSpace,
			lineClamp: getComputedStyle(element).webkitLineClamp
		}))
	).toEqual({ whiteSpace: 'pre-line', lineClamp: '3' });

	const structuralCard = structuralTitleElement.locator('..');
	const structuralCell = structuralCard.locator('..');
	const [cardBox, cellBox] = await Promise.all([
		structuralCard.boundingBox(),
		structuralCell.boundingBox()
	]);
	expect(cardBox).not.toBeNull();
	expect(cellBox).not.toBeNull();
	const edgeGaps = [
		cardBox!.x - cellBox!.x,
		cellBox!.x + cellBox!.width - cardBox!.x - cardBox!.width,
		cardBox!.y - cellBox!.y,
		cellBox!.y + cellBox!.height - cardBox!.y - cardBox!.height
	];
	expect(Math.max(...edgeGaps) - Math.min(...edgeGaps)).toBeLessThanOrEqual(2);

	await expect(page.getByText('CLUB', { exact: true })).toHaveCount(0);
	await expect(page.getByText('กิจกรรมรวม', { exact: true })).toHaveCount(0);
	await expect(page.getByText('กิจกรรมพร้อมกัน', { exact: true })).toHaveCount(0);
	await expect(page.getByText('ครูคณิตศาสตร์ A', { exact: true })).toHaveCount(0);
	await expect(page.getByText('ครูคณิตศาสตร์ B', { exact: true })).toHaveCount(0);

	await expect(page.getByText('ค21101', { exact: true })).toBeVisible();
	await expect(page.getByText('คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
});
