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
	test(`centered dates and ${width < 640 ? 'compact markers' : 'all-day bars and timed dots'} at ${width}px`, async ({
		page
	}, testInfo) => {
		const mobile = width < 640;
		await page.setViewportSize({ width, height: mobile ? 900 : 1050 });
		await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
		await page.route('https://fonts.googleapis.com/**', (route) =>
			route.fulfill({ status: 200, contentType: 'text/css', body: '' })
		);
		const api = await mockCalendar(page);
		const base = makeApprovedCalendarEvent({
			title: 'ค่ายทั้งวัน',
			startDate: '2026-09-28',
			endDate: '2026-10-03',
			allDay: true,
			isPublic: true,
			notifyAudience: false,
			reminderOffsetsDays: [],
			targets: [{ audienceType: 'all' }],
			startTime: null,
			endTime: null
		});
		const records = [
			{ ...base, id: id(700), categoryColor: '#2563eb' },
			{
				...base,
				id: id(701),
				title: 'สอบช่วงเช้า',
				startDate: '2026-10-01',
				endDate: '2026-10-01',
				allDay: false,
				startTime: '08:30:00',
				endTime: '09:30:00',
				categoryColor: '#2563eb'
			},
			{
				...base,
				id: id(702),
				title: 'อบรมช่วงบ่าย',
				startDate: '2026-10-01',
				endDate: '2026-10-03',
				allDay: false,
				startTime: '13:00:00',
				endTime: '14:00:00',
				categoryColor: '#2563eb'
			}
		];
		await page.route(
			(url) => url.pathname === eventsPath,
			(route) =>
				route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ success: true, data: records })
				})
		);
		await page.goto(calendarPath());
		const grid = page.getByTestId('calendar-events');
		const agenda = page.getByRole('region', { name: 'รายละเอียดปฏิทิน', exact: true });
		const day = page.getByRole('button', { name: '1 ต.ค. 2569, 3 กิจกรรม', exact: true });
		await expect(day).toBeVisible();
		const centered = await day.evaluate((el) => {
			const cell = el.getBoundingClientRect(),
				number = el.querySelector('span')!.getBoundingClientRect();
			return Math.abs((cell.left + cell.right) / 2 - (number.left + number.right) / 2) < 1;
		});
		expect(centered).toBe(true);
		const bar = grid.locator('[title="ค่ายทั้งวัน"]');
		const timed = grid.locator('[title="08:30 สอบช่วงเช้า"]');
		const afternoon = grid.locator('[title="13:00 อบรมช่วงบ่าย"]');
		const marker = day.locator('[data-calendar-day-marker="event"]');
		if (mobile) {
			await expect(marker).toBeVisible();
			await expect(marker).toHaveCSS('background-color', 'rgb(37, 99, 235)');
			await expect(timed).toBeHidden();
			await expect(bar).toBeHidden();
			await expect(grid.getByRole('button', { name: /^ดูรายละเอียด / })).toHaveCount(0);
			await grid.locator('[data-calendar-date="2026-10-03"]').focus();
			await page.keyboard.press('Tab');
			await expect(grid.locator('[data-calendar-date="2026-10-04"]')).toBeFocused();
			await day.tap();
			await expect(agenda.getByRole('article')).toHaveCount(3);
		} else {
			await expect(marker).toBeHidden();
			await expect(timed).toBeVisible();
			await expect(afternoon).toHaveCount(3);
			const styles = await timed.evaluate((el) => ({
				background: getComputedStyle(el).backgroundColor,
				dot: getComputedStyle(el.firstElementChild!).backgroundColor,
				column: el.style.gridColumn
			}));
			expect(styles.background).toBe('rgba(0, 0, 0, 0)');
			expect(styles.dot).toBe('rgb(37, 99, 235)');
			expect(styles.column).toContain('span 1');
			await expect(bar).toHaveCSS('background-color', 'rgb(37, 99, 235)');
			expect(await bar.evaluate((el) => el.style.gridColumn)).toContain('span 6');
			await expect(timed).toHaveText('08:30 สอบช่วงเช้า', { useInnerText: true });
		}
		await page.addStyleTag({
			content: '*,*::before,*::after{transition:none!important;animation:none!important}'
		});
		for (const theme of ['light', 'dark']) {
			await page.evaluate(
				(value) => document.documentElement.classList.toggle('dark', value === 'dark'),
				theme
			);
			await page.screenshot({
				path: testInfo.outputPath(`calendar-timed-${width}-${theme}.png`),
				fullPage: true
			});
		}
		if (!mobile) {
			await timed.click();
			await expect(agenda.getByRole('article')).toHaveCount(1);
		}
		await expect(agenda).toContainText('08:30-09:30');
		await agenda.getByRole('button', { name: 'แก้ไข สอบช่วงเช้า', exact: true }).click();
		const editor = page.getByRole('dialog', { name: 'แก้ไขกิจกรรมปฏิทิน', exact: true });
		const allDay = editor.getByRole('checkbox', { name: 'ทั้งวัน', exact: true });
		const start = editor.getByLabel('เวลาเริ่มต้น *', { exact: true });
		await expect(allDay).not.toBeChecked();
		await expect(start).toHaveValue('08:30');
		await expect(editor.getByLabel('เวลาสิ้นสุด *', { exact: true })).toHaveValue('09:30');
		await editor.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await expect(editor).toHaveCount(0);
		if (!mobile) {
			await bar.click();
			await expect(agenda.getByRole('article')).toHaveCount(1);
		}
		await agenda.getByRole('button', { name: 'แก้ไข ค่ายทั้งวัน', exact: true }).click();
		await expect(allDay).toBeChecked();
		await expect(start).toHaveCount(0);
		await editor.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await expect(editor).toHaveCount(0);
		expect(api.writes).toHaveLength(0);
	});
}
