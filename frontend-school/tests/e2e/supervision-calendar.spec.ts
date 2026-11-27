import { expect, test } from '@playwright/test';
import type { SupervisionCalendarEntry } from '../../src/lib/api/supervision';
import {
	firstCycle,
	id,
	mock,
	path,
	supervisionRoot,
	year
} from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });

const calendarApiPath = '/api/supervision/calendar';
const term = id(8);
const observedDate = '2026-10-01';

function booking(
	value: number,
	status: SupervisionCalendarEntry['status'],
	observedDisplayName: string
): SupervisionCalendarEntry {
	return {
		id: id(value),
		cycleId: firstCycle,
		cycleTitle: 'รอบสังเกตการสอน',
		observedDisplayName,
		status,
		evaluatorDisplayNames: status === 'confirmed' ? ['ครู ผู้ประเมิน'] : [],
		periods: [
			{
				id: id(value + 10),
				position: 1,
				observedAt: '2026-10-01T01:30:00Z',
				endsAt: '2026-10-01T02:20:00Z',
				periodLabel: 'คาบ 1'
			}
		]
	};
}

test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});

test('staff without grants can discover the calendar and read only the selected academic schedule', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: [],
		term: true,
		denyStaffContext: true,
		menuGroups: [
			{
				code: 'academic_supervision',
				name: 'นิเทศการสอน',
				icon: 'ClipboardCheck',
				workspaceCode: 'academic',
				workspaceName: 'วิชาการ',
				workspaceOrder: 1,
				displayOrder: 1,
				items: [
					{
						id: 'supervision',
						code: 'supervision',
						name: 'นิเทศการสอน',
						path: supervisionRoot,
						icon: 'ClipboardCheck'
					}
				]
			}
		]
	});
	const reads: URL[] = [];
	await page.route(
		(url) => url.pathname === calendarApiPath,
		async (route) => {
			expect(route.request().method()).toBe('GET');
			const url = new URL(route.request().url());
			reads.push(url);
			const items = url.searchParams.has('academicTermId')
				? [booking(31, 'confirmed', 'ครู เฉพาะภาคเรียน')]
				: [booking(32, 'confirmed', 'ครู ยืนยันนัด'), booking(33, 'requested', 'ครู รออนุมัติ')];
			await route.fulfill({ json: { success: true, data: { items } } });
		}
	);

	await page.goto('/staff/profile');
	const hubLink = page.locator('aside').getByRole('link', { name: 'นิเทศการสอน', exact: true });
	await expect(hubLink).toBeVisible();
	await hubLink.click();
	const hub = page.getByTestId('supervision-hub');
	const calendarLink = hub.getByRole('link', { name: 'ปฏิทินนิเทศ', exact: true });
	await expect(hub.getByRole('link')).toHaveCount(1);
	await expect(calendarLink).toHaveAttribute('href', path('calendar'));
	await expect(calendarLink).toHaveAttribute('data-sveltekit-preload-data', 'false');
	await calendarLink.hover();
	expect(reads).toHaveLength(0);
	await page.setViewportSize({ width: 390, height: 900 });
	await calendarLink.click();

	const body = page.getByTestId('supervision-calendar-schedule');
	const day = body.getByRole('region');
	const nav = page.getByTestId('supervision-workflow-nav');
	await expect(page.getByRole('heading', { name: 'ปฏิทินนิเทศ', exact: true })).toBeVisible();
	await expect(nav.getByRole('link')).toHaveCount(1);
	await expect(nav.getByRole('link', { name: 'หน้าหลักนิเทศการสอน' })).toHaveAttribute(
		'href',
		path('')
	);
	await expect(day.getByText('ครู ยืนยันนัด', { exact: true })).toBeVisible();
	await expect(day.getByText('ครู รออนุมัติ', { exact: true })).toBeVisible();
	await expect(day.getByText('08:30–09:20 น.').first()).toBeVisible();
	await expect(day.getByText('ครู ผู้ประเมิน', { exact: true })).toBeVisible();
	await expect(day.getByText('รอบสังเกตการสอน', { exact: true }).first()).toBeVisible();
	await expect(day.getByText('ยืนยันแล้ว', { exact: true })).toBeVisible();
	await expect(day.getByText('รออนุมัติคำขอ', { exact: true })).toBeVisible();
	await expect(day.getByRole('link')).toHaveCount(0);
	await expect(day.getByRole('button')).toHaveCount(0);
	expect(reads).toHaveLength(1);
	expect(Object.fromEntries(reads[0].searchParams)).toEqual({
		academicYearId: year,
		fromDate: '2026-09-27',
		toDate: '2026-11-07'
	});
	expect(api.reads.filter((read) => read === '/api/public/academic-context/options')).toHaveLength(
		1
	);

	await body.locator('[data-calendar-date="2026-10-02"]').click();
	await expect(day.getByText('ไม่มีคาบนิเทศในวันที่เลือก', { exact: true })).toBeVisible();
	await body.locator(`[data-calendar-date="${observedDate}"]`).click();
	await expect(day.getByText('ครู ยืนยันนัด', { exact: true })).toBeVisible();
	expect(reads).toHaveLength(1);

	await page.getByTestId('academic-context-mobile-trigger').click();
	await page.getByLabel('เลือกภาคเรียน (มือถือ)', { exact: true }).click();
	await page.getByRole('option', { name: 'ภาคเรียนที่ 1', exact: true }).click();
	await expect(page).toHaveURL(`${path('calendar')}&academicTermId=${term}`);
	await expect(day.getByText('ครู เฉพาะภาคเรียน', { exact: true })).toBeVisible();
	await expect(day.getByText('ครู ยืนยันนัด', { exact: true })).toHaveCount(0);
	expect(reads).toHaveLength(2);
	expect(Object.fromEntries(reads[1].searchParams)).toEqual({
		academicYearId: year,
		academicTermId: term,
		fromDate: '2026-09-27',
		toDate: '2026-11-07'
	});
	await expect(nav.getByRole('link', { name: 'หน้าหลักนิเทศการสอน' })).toHaveAttribute(
		'href',
		`${path('')}&academicTermId=${term}`
	);

	await page.goto(path(id(31)));
	await expect(page).toHaveURL(/\/403\?/);
	expect(api.reads.filter((read) => read.startsWith('/api/supervision/'))).toEqual([]);
	expect(api.writes.filter((write) => write.includes('/api/supervision/'))).toEqual([]);
});

test('month changes supersede a delayed read and a failed month can retry in place', async ({
	page
}) => {
	await mock(page, { permissions: [], term: true });
	const november = Promise.withResolvers<void>();
	const served: string[] = [];
	let decemberFailed = false;
	await page.route(
		(url) => url.pathname === calendarApiPath,
		async (route) => {
			const from = new URL(route.request().url()).searchParams.get('fromDate') ?? '';
			served.push(from);
			if (from === '2026-11-01') await november.promise;
			if (from === '2026-11-29' && !decemberFailed) {
				decemberFailed = true;
				await route.fulfill({
					status: 503,
					json: { success: false, error: 'calendar unavailable' }
				});
				return;
			}
			const items =
				from === '2026-11-01'
					? [
							{
								...booking(34, 'confirmed', 'ครู เดือนพฤศจิกายน'),
								periods: [
									{
										id: id(44),
										position: 1,
										observedAt: '2026-11-01T01:30:00Z',
										endsAt: null,
										periodLabel: null
									}
								]
							}
						]
					: from === '2026-09-27'
						? [booking(31, 'confirmed', 'ครู เดือนตุลาคม')]
						: [];
			await route.fulfill({ json: { success: true, data: { items } } });
		}
	);

	const schedule = page.getByTestId('supervision-calendar-schedule');
	const next = page.getByRole('button', { name: 'เดือนถัดไป' });
	await page.goto(path('calendar'));
	await expect(
		schedule.getByRole('region').getByText('ครู เดือนตุลาคม', { exact: true })
	).toBeVisible();
	try {
		await next.click();
		await expect(page).toHaveURL(/month=2026-11/);
		await expect(schedule).toHaveAttribute('aria-busy', 'true');
		await expect(schedule.getByRole('status', { name: 'กำลังโหลดปฏิทินนิเทศ' })).toBeVisible();
		await expect(schedule.getByText('ครู เดือนตุลาคม', { exact: true })).toHaveCount(0);
		await next.click();
		await expect(page).toHaveURL(/month=2026-12/);
		await expect(page.getByText('โหลดปฏิทินนิเทศไม่สำเร็จ', { exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'ลองโหลดปฏิทินอีกครั้ง', exact: true }).click();
		await expect(schedule).toHaveAttribute('aria-busy', 'false');
		await expect(schedule.getByText('ไม่มีคาบนิเทศในวันที่เลือก', { exact: true })).toBeVisible();
	} finally {
		november.resolve();
	}
	await expect(schedule.getByText('ครู เดือนพฤศจิกายน', { exact: true })).toHaveCount(0);
	expect(served).toEqual(['2026-09-27', '2026-11-01', '2026-11-29', '2026-11-29']);
	await page.goBack();
	await expect(page).toHaveURL(/month=2026-11/);
	await expect(
		schedule.getByRole('region').getByText('ครู เดือนพฤศจิกายน', { exact: true })
	).toBeVisible();
	expect(served).toHaveLength(5);
});

test('a non-staff session cannot load either supervision entry point', async ({ page }) => {
	const api = await mock(page, { userType: 'student', permissions: ['*'] });
	await page.goto(path('calendar'));
	await expect(page).toHaveURL(/\/403\?/);
	await page.goto(path(''));
	await expect(page).toHaveURL(/\/403\?/);
	expect(api.count(calendarApiPath)).toBe(0);
});
