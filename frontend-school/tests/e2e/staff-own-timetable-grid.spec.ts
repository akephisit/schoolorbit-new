import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const academicYearId = '20000000-0000-4000-8000-000000000001';
const academicTermId = '30000000-0000-4000-8000-000000000001';
const bellScheduleId = '71000000-0000-4000-8000-000000000001';
const periodIds = {
	first: '70000000-0000-4000-8000-000000000001',
	second: '70000000-0000-4000-8000-000000000002',
	third: '70000000-0000-4000-8000-000000000003'
};

const configuredPeriods = [
	{
		id: periodIds.first,
		name: 'คาบ 1',
		startTime: '08:00:00',
		endTime: '08:50:00'
	},
	{
		id: periodIds.second,
		name: 'คาบ 2',
		startTime: '09:00:00',
		endTime: '09:50:00'
	},
	{
		id: periodIds.third,
		name: 'คาบ 3',
		startTime: '10:00:00',
		endTime: '10:50:00'
	}
];

const thirdPeriodEntry = {
	id: '40000000-0000-4000-8000-000000000001',
	academicTermId,
	academicYearId,
	bellScheduleId,
	bellSchedulePeriodId: periodIds.third,
	createdAt: '2026-08-07T00:00:00Z',
	dayOfWeek: 'MON',
	endTime: '10:50:00',
	entryType: 'COURSE',
	instructors: [],
	isActive: true,
	learningGroupName: 'ม.1/1',
	note: null,
	offeringCode: 'ค21101',
	offeringName: 'คณิตศาสตร์',
	periodName: 'คาบ 3',
	roomCode: 'MATH-1',
	rowVersion: 1,
	startTime: '10:00:00',
	title: null,
	updatedAt: '2026-08-07T00:00:00Z'
};

const configuredPeriodEntries = configuredPeriods.map((period, index) => ({
	...thirdPeriodEntry,
	id: `40000000-0000-4000-8000-00000000000${index + 1}`,
	bellSchedulePeriodId: period.id,
	periodName: period.name,
	startTime: period.startTime,
	endTime: period.endTime,
	offeringCode: index === 2 ? 'ค21101' : `พัก-${index + 1}`,
	offeringName: index === 2 ? 'คณิตศาสตร์' : 'ช่วงเตรียมการ',
	entryType: index === 2 ? 'COURSE' : 'BREAK'
}));

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
