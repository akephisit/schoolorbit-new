import { expect, test, type Route } from '@playwright/test';
import { readFileSync } from 'node:fs';

test.use({ serviceWorkers: 'block' });

function fulfillJson(route: Route, data: unknown) {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

test('downloads the loaded staff timetable from the PageShell action', async ({ page }) => {
	const logoFileId = '80000000-0000-4000-8000-000000000001';
	const logoPng = readFileSync(new URL('../../static/notification-badge.png', import.meta.url));
	let logoDeliveryRequestCount = 0;
	let logoBlobRequestCount = 0;
	let releaseTimetable: ((route: Route) => void) | undefined;
	const timetableRequest = new Promise<Route>((resolve) => {
		releaseTimetable = resolve;
	});

	await page.route('https://public-files.example.test/logo.png', async (route) => {
		logoBlobRequestCount += 1;
		await route.fulfill({
			status: 200,
			contentType: 'image/png',
			headers: { 'access-control-allow-origin': '*' },
			body: logoPng
		});
	});

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/school/public') {
				await fulfillJson(route, {
					logoFileId,
					schoolName: 'ซับน้อยเหนือวิทยาคม'
				});
				return;
			}

			if (url.pathname === `/api/public/files/${logoFileId}/delivery`) {
				logoDeliveryRequestCount += 1;
				await fulfillJson(route, { url: 'https://public-files.example.test/logo.png' });
				return;
			}

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
							id: '20000000-0000-4000-8000-000000000001',
							name: 'ปีการศึกษา 2569',
							year: 2569,
							start_date: '2026-05-01',
							end_date: '2027-03-31',
							is_active: true,
							school_days: 'MON,TUE,WED,THU,FRI,SAT',
							created_at: '2026-05-01T00:00:00Z',
							updated_at: '2026-05-01T00:00:00Z'
						}
					],
					semesters: [
						{
							id: '30000000-0000-4000-8000-000000000001',
							academic_year_id: '20000000-0000-4000-8000-000000000001',
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
				releaseTimetable?.(route);
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

			await fulfillJson(route, {});
		}
	);

	await page.goto('/staff/timetable');
	const downloadButton = page.getByRole('button', { name: 'ดาวน์โหลด PDF' });
	const pendingTimetableRoute = await timetableRequest;
	await expect(downloadButton).toBeDisabled();

	await fulfillJson(pendingTimetableRoute, {
		current_seq: 1,
		periods: [
			{
				id: '70000000-0000-4000-8000-000000000001',
				name: 'คาบ 1',
				start_time: '08:30:00',
				end_time: '09:20:00',
				order_index: 1
			}
		],
		items: [
			{
				id: '40000000-0000-4000-8000-000000000001',
				academic_semester_id: '30000000-0000-4000-8000-000000000001',
				classroom_course_id: '50000000-0000-4000-8000-000000000001',
				classroom_id: '60000000-0000-4000-8000-000000000001',
				created_by: null,
				day_of_week: 'SAT',
				end_time: '09:20:00',
				entry_type: 'COURSE',
				is_active: true,
				note: null,
				period_id: '70000000-0000-4000-8000-000000000001',
				period_name: 'คาบ 1',
				period_order_index: 1,
				room_code: 'MATH-1',
				room_id: null,
				start_time: '08:30:00',
				subject_code: 'ค21101',
				subject_name_th: 'คณิตศาสตร์',
				title: null,
				updated_by: null
			}
		]
	});

	await expect(downloadButton).toBeEnabled();
	await expect(page.getByText('ค21101')).toBeVisible();

	const downloadPromise = page.waitForEvent('download');
	await downloadButton.click();
	const download = await downloadPromise;

	expect(download.suggestedFilename()).toBe(
		'ตารางสอน ครูสายใจ วิทยา ภาคเรียนที่ 1 ปีการศึกษา 2569.pdf'
	);
	await expect(page.getByText('ดาวน์โหลดตารางสอนแล้ว')).toBeVisible();
	await expect(downloadButton).toBeEnabled();
	expect(logoDeliveryRequestCount).toBe(1);
	expect(logoBlobRequestCount).toBe(1);
});
