import { sessionState } from './fixtures/session-state';
import { expect, test, type Route } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import {
	expectRoundedTimetablePdf,
	expectTimetablePdfPresentation,
	expectTimetablePdfTextColors
} from './timetable-pdf-assertions';

import {
	makeTimetableBlock,
	makeStructuralTimetableBlock,
	timetableIds
} from './timetable-test-harness';

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
	let brandingRequestCount = 0;
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
				brandingRequestCount++;
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
					displayName: 'นายสายใจ วิทยา',
					lastName: 'วิทยา',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
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
							isVisible: true,
							id: '20000000-0000-4000-8000-000000000001',
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: '30000000-0000-4000-8000-000000000001',
							academicYearId: '20000000-0000-4000-8000-000000000001',
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					],
					defaultAcademicYearId: '20000000-0000-4000-8000-000000000001',
					defaultAcademicTermId: '30000000-0000-4000-8000-000000000001'
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

	const missingRoomName = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
		code: 'ว21101',
		name: 'วิทยาศาสตร์วันจันทร์'
	});
	missingRoomName.groups[0].roomName = null;
	const otherRoom = makeStructuralTimetableBlock('other-room-target', timetableIds.period1, 'อื่น')
		.homerooms[0];
	missingRoomName.homerooms = [
		{
			...otherRoom,
			roomId: timetableIds.roomB,
			roomName: 'ห้องอื่นที่ไม่ควรแสดง',
			roomCode: 'LAB-2'
		}
	];
	await fulfillJson(pendingTimetableRoute, [
		{
			...missingRoomName,
			academicTermId: '30000000-0000-4000-8000-000000000001',
			academicYearId: '20000000-0000-4000-8000-000000000001',
			dayOfWeek: 'MON'
		},
		{
			...makeTimetableBlock(timetableIds.blockA, timetableIds.period1),
			academicTermId: '30000000-0000-4000-8000-000000000001',
			academicYearId: '20000000-0000-4000-8000-000000000001',
			dayOfWeek: 'SAT'
		}
	]);

	await expect(downloadButton).toBeEnabled();
	await expect(page.getByTestId('own-timetable-week').getByText('ค21101')).toBeVisible();
	await page
		.getByRole('radiogroup', { name: 'มุมมองตารางสอน' })
		.filter({ visible: true })
		.getByRole('radio', { name: 'รายวัน', exact: true })
		.click();
	await page
		.getByRole('radiogroup', { name: 'วันในตารางสอน' })
		.getByRole('radio', { name: 'วันเสาร์', exact: true })
		.click();
	await expect(page.getByTestId('own-timetable-day').getByText('ค21101')).toBeVisible();

	const downloadPromise = page.waitForEvent('download');
	await downloadButton.click();
	const download = await downloadPromise;
	await download.saveAs('/tmp/staff-own-round.pdf');
	await expectRoundedTimetablePdf(download, [1]);
	await expectTimetablePdfPresentation(download);
	await expectTimetablePdfTextColors(download, {
		ค21101: '#1e3a8a',
		คณิตศาสตร์พื้นฐาน: '#000000',
		'ม.1/1': '#d97706',
		ห้องคณิตศาสตร์: '#1f2937',
		สแกนดูตาราง: '#6b7280',
		'SchoolOrbit TimeTable': '#9ca3af'
	});

	expect(download.suggestedFilename()).toBe(
		'ตารางสอน ครูสายใจ วิทยา ภาคเรียนที่ 1 ปีการศึกษา 2569.pdf'
	);
	const text = execFileSync('pdftotext', ['-raw', (await download.path())!, '-'], {
		encoding: 'utf8'
	}).replace(/\s/g, '');
	expect(text).toContain(
		'โรงเรียนซับน้อยเหนือวิทยาคมตารางสอนภาคเรียนที่1ปีการศึกษา2569นายสายใจวิทยา'
	);
	expect(text).toContain('ม.1/1');
	expect(text).toContain('จันทร์');
	expect(text).toContain('เสาร์');
	expect(text).toContain('วิทยาศาสตร์วันจันทร์');
	expect(text).toContain('คณิตศาสตร์พื้นฐาน');
	expect(text).toContain('ห้องคณิตศาสตร์');
	expect(text).toContain('ไม่พบชื่อห้อง');
	expect(text).not.toMatch(/MATH-1|LAB-2|ห้องอื่นที่ไม่ควรแสดง/);
	expect(text).not.toContain('ม.1/1คณิตศาสตร์');
	await expect(page.getByText('ดาวน์โหลดตารางสอนแล้ว')).toBeVisible();
	await expect(downloadButton).toBeEnabled();
	expect(brandingRequestCount).toBe(1);
	expect(logoDeliveryRequestCount).toBe(1);
	expect(logoBlobRequestCount).toBe(1);
});
