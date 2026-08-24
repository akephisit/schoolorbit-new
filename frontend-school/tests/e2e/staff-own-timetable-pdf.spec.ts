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

			if (url.pathname === '/api/academic/context/options') {
				await fulfillJson(route, {
					years: [
						{
							id: '20000000-0000-4000-8000-000000000001',
							name: 'ปีการศึกษา 2569',
							year: 2569,
							startDate: '2026-05-01',
							endDate: '2027-03-31',
							status: 'active'
						}
					],
					terms: [
						{
							id: '30000000-0000-4000-8000-000000000001',
							academicYearId: '20000000-0000-4000-8000-000000000001',
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
					activeAcademicYearId: '20000000-0000-4000-8000-000000000001',
					activeAcademicTermId: '30000000-0000-4000-8000-000000000001'
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

	await fulfillJson(pendingTimetableRoute, [
		{
			id: '40000000-0000-4000-8000-000000000001',
			academicTermId: '30000000-0000-4000-8000-000000000001',
			academicYearId: '20000000-0000-4000-8000-000000000001',
			bellScheduleId: '71000000-0000-4000-8000-000000000001',
			bellSchedulePeriodId: '70000000-0000-4000-8000-000000000001',
			createdAt: '2026-08-07T00:00:00Z',
			dayOfWeek: 'SAT',
			endTime: '09:20:00',
			entryType: 'COURSE',
			instructors: [],
			isActive: true,
			learningGroupName: 'ม.1/1',
			note: null,
			offeringCode: 'ค21101',
			offeringName: 'คณิตศาสตร์',
			periodName: 'คาบ 1',
			roomCode: 'MATH-1',
			rowVersion: 1,
			startTime: '08:30:00',
			title: null,
			updatedAt: '2026-08-07T00:00:00Z'
		}
	]);

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
