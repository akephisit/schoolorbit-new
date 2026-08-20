import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const academicYearId = '20000000-0000-4000-8000-000000000001';
const semesterId = '30000000-0000-4000-8000-000000000001';
const periodIds = {
	first: '70000000-0000-4000-8000-000000000001',
	second: '70000000-0000-4000-8000-000000000002',
	third: '70000000-0000-4000-8000-000000000003'
};

const configuredPeriods = [
	{
		id: periodIds.first,
		name: 'คาบ 1',
		start_time: '08:00:00',
		end_time: '08:50:00',
		order_index: 1
	},
	{
		id: periodIds.second,
		name: 'คาบ 2',
		start_time: '09:00:00',
		end_time: '09:50:00',
		order_index: 2
	},
	{
		id: periodIds.third,
		name: 'คาบ 3',
		start_time: '10:00:00',
		end_time: '10:50:00',
		order_index: 3
	}
];

const thirdPeriodEntry = {
	id: '40000000-0000-4000-8000-000000000001',
	academic_semester_id: semesterId,
	classroom_course_id: '50000000-0000-4000-8000-000000000001',
	classroom_id: '60000000-0000-4000-8000-000000000001',
	created_by: null,
	day_of_week: 'MON',
	end_time: '10:50:00',
	entry_type: 'COURSE',
	is_active: true,
	note: null,
	period_id: periodIds.third,
	period_name: 'คาบ 3',
	period_order_index: 3,
	room_code: 'MATH-1',
	room_id: null,
	start_time: '10:00:00',
	subject_code: 'ค21101',
	subject_name_th: 'คณิตศาสตร์',
	title: null,
	updated_by: null
};

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

			if (url.pathname === '/api/academic/structure') {
				await fulfillJson(route, {
					years: [
						{
							id: academicYearId,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							start_date: '2026-05-01',
							end_date: '2027-03-31',
							is_active: true,
							school_days: 'MON,SAT'
						}
					],
					semesters: [
						{
							id: semesterId,
							academic_year_id: academicYearId,
							name: 'ภาคเรียนที่ 1',
							term: '1',
							start_date: '2026-05-01',
							end_date: '2026-10-31',
							is_active: true
						}
					],
					levels: []
				});
				return;
			}

			if (url.pathname === '/api/me/timetable') {
				await fulfillJson(route, { current_seq: 1, items, periods: configuredPeriods });
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
	await expect(page.getByText('เสาร์', { exact: true })).toBeVisible();
	await expect(page.getByText('อาทิตย์', { exact: true })).toHaveCount(0);
}

test('shows configured periods that precede the teacher first lesson', async ({ page }) => {
	await mockStaffTimetableApis(page, [thirdPeriodEntry]);

	await page.goto('/staff/timetable');

	await expectConfiguredGrid(page);
	await expect(page.getByText('ค21101')).toBeVisible();
});

test('shows the complete configured grid when the teacher has no lessons', async ({ page }) => {
	await mockStaffTimetableApis(page, []);

	await page.goto('/staff/timetable');

	await expectConfiguredGrid(page);
	await expect(page.locator('tbody tr')).toHaveCount(2);
	await expect(page.getByText('ยังไม่มีตารางสอนของคุณในภาคเรียนนี้')).toHaveCount(0);
});
