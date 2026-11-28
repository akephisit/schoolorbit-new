import { test, expect } from '@playwright/test';
import {
	mockCalendar,
	calendarPath,
	eventsPath,
	makeApprovedCalendarEvent
} from './fixtures/calendar-route-data';
import { id } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block', hasTouch: true });
for (const width of [1280, 375]) {
	for (const manager of [true, false]) {
		test(`the agenda shows complete event details at ${width}px for ${manager ? 'managers' : 'readers'}`, async ({
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
			const description = [
				'รายละเอียดการสอบครบถ้วน',
				'เตรียมบัตรประจำตัวนักเรียน',
				'เข้าห้องสอบก่อนเวลา',
				'นำอุปกรณ์ที่จำเป็นมาด้วย',
				'ติดตามผลสอบหลังประกาศ'
			].join('\n');
			const event = {
				...makeApprovedCalendarEvent({
					title: 'สอบช่วงเช้า',
					description,
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
							data: [
								event,
								{
									...event,
									id: id(711),
									title: 'กิจกรรมอื่นในวันเดียวกัน',
									description: null,
									startTime: '09:45',
									endTime: '10:15'
								}
							]
						})
					})
			);
			await page.goto(calendarPath());
			const grid = page.getByTestId('calendar-events');
			const day = page.locator('[data-calendar-date="2026-10-01"]');
			const agenda = page.getByRole('region', { name: 'รายละเอียดปฏิทิน', exact: true });
			const entry = grid.getByRole('button', { name: /^ดูรายละเอียด สอบช่วงเช้า,/ });
			const otherEvent = agenda.getByRole('heading', { name: 'กิจกรรมอื่นในวันเดียวกัน' });
			const wholeDay = agenda.getByRole('button', { name: 'ดูทั้งวัน', exact: true });
			await expect(agenda.getByRole('heading', { name: '1 ต.ค. 2569', exact: true })).toBeVisible();
			await expect(otherEvent).toBeVisible();
			if (width < 640) {
				await expect(entry).toHaveCount(0);
				await expect(day.locator('[data-calendar-day-marker="event"]')).toBeVisible();
				await day.tap();
				await expect(otherEvent).toBeVisible();
				await expect(wholeDay).toHaveCount(0);
			} else {
				await entry.focus();
				await page.keyboard.press('Enter');
				await expect(wholeDay).toBeVisible();
				await expect(otherEvent).toHaveCount(0);
			}
			const primaryEvent = agenda.getByRole('article').filter({
				has: page.getByRole('heading', { name: 'สอบช่วงเช้า', exact: true })
			});
			const descriptionText = primaryEvent.getByText(description, { exact: true });
			await expect(descriptionText).toBeVisible();
			await expect
				.poll(() => descriptionText.evaluate((text) => text.scrollHeight - text.clientHeight))
				.toBeLessThanOrEqual(1);
			await expect(primaryEvent.getByText('08:30-09:30', { exact: true })).toBeVisible();
			await expect(primaryEvent.getByText('ห้องประชุม', { exact: true })).toBeVisible();
			await expect(
				agenda.getByRole('button', { name: 'แก้ไข สอบช่วงเช้า', exact: true })
			).toHaveCount(manager ? 1 : 0);
			const remove = agenda.getByRole('button', { name: 'ลบ สอบช่วงเช้า', exact: true });
			await expect(remove).toHaveCount(manager ? 1 : 0);
			const agendaBox = (await agenda.boundingBox())!;
			expect(agendaBox.x).toBeGreaterThanOrEqual(0);
			expect(agendaBox.x + agendaBox.width).toBeLessThanOrEqual(width);
			await page.addStyleTag({
				content: '*,*::before,*::after{transition:none!important;animation:none!important}'
			});
			for (const theme of ['light', 'dark']) {
				await page.evaluate(
					(value) => document.documentElement.classList.toggle('dark', value === 'dark'),
					theme
				);
				if (width < 640) await descriptionText.scrollIntoViewIfNeeded();
				await page.screenshot({
					path: testInfo.outputPath(`calendar-agenda-${width}-${theme}.png`)
				});
			}
			await page.keyboard.press('Escape');
			await expect(agenda).toBeVisible();
			if (width >= 640) {
				await expect(otherEvent).toHaveCount(0);
				await wholeDay.press('Enter');
				await expect(otherEvent).toBeVisible();
				await expect(entry).toBeFocused();
			}
			if (manager) {
				await remove.click();
				const confirmation = page.getByRole('alertdialog');
				await expect(confirmation).toContainText('สอบช่วงเช้า');
				await confirmation.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
				await expect(confirmation).toHaveCount(0);
				await expect(remove).toBeVisible();
			}
			const otherDay = page.locator(
				`[data-calendar-date="${width < 640 ? '2026-09-27' : '2026-10-20'}"]`
			);
			const nextDay = page.locator(
				`[data-calendar-date="${width < 640 ? '2026-09-28' : '2026-10-21'}"]`
			);
			await otherDay.click({ position: { x: 5, y: 5 } });
			await expect(agenda.getByRole('heading', { level: 2 })).toHaveText(
				width < 640 ? '27 ก.ย. 2569' : '20 ต.ค. 2569'
			);
			await nextDay.tap({ position: { x: 5, y: 5 } });
			await expect(agenda.getByRole('heading', { level: 2 })).toHaveText(
				width < 640 ? '28 ก.ย. 2569' : '21 ต.ค. 2569'
			);
			await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
			await expect(agenda.getByRole('heading', { name: '1 พ.ย. 2569', exact: true })).toBeVisible();
			expect(api.writes).toHaveLength(0);
			expect(api.count('/api/calendar/target-options')).toBe(0);
		});
	}
}

test('the day agenda includes activities hidden behind the overflow count', async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 900 });
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
	const visibleEntries = page.getByRole('button', { name: /^ดูรายละเอียด กิจกรรมลำดับ/ });
	await expect
		.poll(async () =>
			day.getByText(`+${6 - (await visibleEntries.count())}`, { exact: true }).count()
		)
		.toBe(1);
	const agenda = page.getByRole('region', { name: 'รายละเอียดปฏิทิน', exact: true });
	const entry = visibleEntries.first();
	await entry.click();
	await expect(agenda.getByRole('article')).toHaveCount(1);
	await agenda.getByRole('button', { name: 'ดูทั้งวัน', exact: true }).click();
	await expect(agenda.getByRole('article')).toHaveCount(6);
	await expect(agenda).toContainText('กิจกรรมลำดับ 6');
	await expect(entry).toBeFocused();
});
