import { expect, test } from '@playwright/test';
import {
	mockCalendar,
	calendarPath,
	makeApprovedCalendarEvent,
	eventsPath
} from './fixtures/calendar-route-data';
import { publicYears } from './fixtures/public-calendar-server';

test.use({ serviceWorkers: 'block' });
test('staff partial search finds another month and opens that date without reloading catalogs', async ({
	page
}) => {
	const api = await mockCalendar(page, { permissions: ['calendar.read.school'] });
	await page.route('**/api/public/academic-context/options', (route) =>
		route.fulfill({ json: { success: true, data: publicYears } })
	);
	await page.route(
		(url) => url.pathname === eventsPath && url.searchParams.get('search') === 'true',
		(route) =>
			route.fulfill({
				json: {
					success: true,
					data: [
						makeApprovedCalendarEvent({
							title: 'ทัศนศึกษาพฤศจิกายน',
							startDate: '2026-11-05',
							endDate: '2026-11-05',
							allDay: true,
							isPublic: true,
							targets: [{ audienceType: 'all' }],
							tagIds: [],
							reminderOffsetsDays: [],
							notifyAudience: false
						})
					]
				}
			})
	);
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'ค้นหากิจกรรม', exact: true }).click();
	await page.getByLabel('คำค้นหากิจกรรม').fill('ทัศน');
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: /ทัศนศึกษาพฤศจิกายน/ })).toBeVisible();
	await dialog.getByRole('button', { name: /ทัศนศึกษาพฤศจิกายน/ }).click();
	await expect(page).toHaveURL(/month=2026-11/);
	await expect(dialog).toHaveCount(0);
	expect(api.count('/api/calendar/categories')).toBe(1);
	expect(api.count('/api/calendar/tags')).toBe(1);
	expect(api.writes).toHaveLength(0);
});

test('profile logout completes on the school homepage', async ({ page }) => {
	await mockCalendar(page);
	let loggedOut = false;
	await page.route('**/api/auth/logout', (route) => {
		loggedOut = true;
		return route.fulfill({ json: { success: true, data: {} } });
	});
	await page.route('**/api/auth/me', (route) =>
		loggedOut
			? route.fulfill({ status: 401, json: { success: false, error: 'Unauthenticated' } })
			: route.fallback()
	);
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'เปิดเมนูบัญชี', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ออกจากระบบ', exact: true }).click();
	await expect(page).toHaveURL(/\/$/);
	expect(loggedOut).toBe(true);
});
