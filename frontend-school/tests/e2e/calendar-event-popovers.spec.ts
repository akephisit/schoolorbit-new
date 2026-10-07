import { test, expect } from '@playwright/test';
import {
	mockCalendar,
	calendarPath,
	eventsPath,
	makeApprovedCalendarEvent
} from './fixtures/calendar-route-data';
import { id } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block' });
for (const width of [1280, 375]) {
	for (const manager of [true, false]) {
		test(`event details use an anchored popup at ${width}px for ${manager ? 'managers' : 'readers'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
			await page.route('https://fonts.googleapis.com/**', (route) =>
				route.fulfill({ status: 200, contentType: 'text/css', body: '' })
			);
			const api = await mockCalendar(page, {
				permissions: ['calendar.read.school', ...(manager ? ['calendar.manage.school'] : [])]
			});
			const event = {
				...makeApprovedCalendarEvent({
					title: 'สอบช่วงเช้า',
					description: 'รายละเอียดการสอบครบถ้วน',
					location: 'ห้องประชุม',
					startDate: '2026-10-01',
					endDate: '2026-10-01',
					startTime: '08:30',
					endTime: '09:30',
					allDay: false,
					isPublic: true,
					notifyAudience: false,
					reminderOffsetsDays: [],
					targets: [{ audienceType: 'all' }]
				}),
				id: id(710),
				categoryColor: '#2563eb'
			};
			await page.route(
				(url) => url.pathname === eventsPath,
				(route) =>
					route.fulfill({
						status: 200,
						contentType: 'application/json',
						body: JSON.stringify({
							success: true,
							data: [event, { ...event, id: id(711), title: 'กิจกรรมอื่นในวันเดียวกัน' }]
						})
					})
			);
			await page.goto(calendarPath());
			const entry = page.getByRole('button', { name: /^ดูรายละเอียด สอบช่วงเช้า,/ });
			await expect(entry).toBeVisible();
			await expect(
				page.getByRole('button', { name: 'แก้ไข สอบช่วงเช้า', exact: true })
			).toHaveCount(0);
			const grid = page.getByTestId('calendar-events');
			const day = page.locator('[data-calendar-date="2026-10-01"]');
			const gridBox = (await grid.boundingBox())!;
			const dayBox = (await day.boundingBox())!;
			expect(dayBox.width * 7).toBeGreaterThan(gridBox.width - 4);
			await entry.focus();
			await page.keyboard.press('Enter');
			const popup = page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true });
			await expect(popup).toContainText('รายละเอียดการสอบครบถ้วน');
			await expect(popup).not.toContainText('กิจกรรมอื่นในวันเดียวกัน');
			await expect(popup).toContainText('08:30-09:30');
			await expect(popup).toContainText('ห้องประชุม');
			await expect(
				popup.getByRole('button', { name: 'แก้ไข สอบช่วงเช้า', exact: true })
			).toHaveCount(manager ? 1 : 0);
			await expect(popup.getByRole('button', { name: 'ลบ สอบช่วงเช้า', exact: true })).toHaveCount(
				manager ? 1 : 0
			);
			const popupBox = (await popup.boundingBox())!;
			expect(popupBox.x).toBeGreaterThanOrEqual(0);
			expect(popupBox.x + popupBox.width).toBeLessThanOrEqual(width);
			await page.addStyleTag({
				content: '*,*::before,*::after{transition:none!important;animation:none!important}'
			});
			for (const theme of ['light', 'dark']) {
				await page.evaluate(
					(value) => document.documentElement.classList.toggle('dark', value === 'dark'),
					theme
				);
				await page.screenshot({
					path: testInfo.outputPath(`calendar-popup-${width}-${theme}.png`)
				});
			}
			await page.keyboard.press('Escape');
			await expect(popup).toHaveCount(0);
			await expect(entry).toBeFocused();
			await entry.click();
			if (manager) {
				await popup.getByRole('button', { name: 'ลบ สอบช่วงเช้า', exact: true }).click();
				await expect(popup).toHaveCount(0);
				await expect(page.getByRole('alertdialog')).toContainText('สอบช่วงเช้า');
				await page
					.getByRole('alertdialog')
					.getByRole('button', { name: 'ยกเลิก', exact: true })
					.click();
				await entry.click();
			}
			await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
			await expect(popup).toHaveCount(0);
			expect(api.writes).toHaveLength(0);
			expect(api.count('/api/calendar/target-options')).toBe(0);
		});
	}
}

test('a day popup exposes activities hidden behind the overflow count', async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await mockCalendar(page);
	const base = makeApprovedCalendarEvent({
		title: 'Activity',
		startDate: '2026-10-01',
		endDate: '2026-10-01',
		allDay: true,
		isPublic: true,
		notifyAudience: false,
		reminderOffsetsDays: [],
		targets: [{ audienceType: 'all' }],
		startTime: null,
		endTime: null
	});
	await page.route(
		(url) => url.pathname === eventsPath,
		(route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					success: true,
					data: Array.from({ length: 6 }, (_, index) => ({
						...base,
						id: id(720 + index),
						title: `กิจกรรมลำดับ ${index + 1}`
					}))
				})
			})
	);
	await page.goto(calendarPath());
	const day = page.getByRole('button', { name: '1 ต.ค. 2569, 6 กิจกรรม', exact: true });
	await expect(day).toContainText('+3');
	await day.click({ position: { x: 5, y: 5 } });
	const popup = page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true });
	await expect(popup.getByRole('article')).toHaveCount(6);
	await expect(popup).toContainText('กิจกรรมลำดับ 6');
	await popup.getByRole('button', { name: 'ปิดรายละเอียดกิจกรรม', exact: true }).click();
	await expect(popup).toHaveCount(0);
	await expect(day).toBeFocused();
});
