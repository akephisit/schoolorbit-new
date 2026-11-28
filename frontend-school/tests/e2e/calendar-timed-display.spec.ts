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
	test(`centered dates, all-day bars, and timed dots at ${width}px`, async ({ page }, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
		await page.route('https://fonts.googleapis.com/**', (route) =>
			route.fulfill({ status: 200, contentType: 'text/css', body: '' })
		);
		await mockCalendar(page);
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
		const day = page.getByRole('button', { name: '1 ต.ค. 2569, 3 กิจกรรม', exact: true });
		await day.click({ position: { x: 5, y: 5 } });
		const centered = await day.evaluate((el) => {
			const cell = el.getBoundingClientRect(),
				number = el.querySelector('span')!.getBoundingClientRect();
			return Math.abs((cell.left + cell.right) / 2 - (number.left + number.right) / 2) < 1;
		});
		expect(centered).toBe(true);
		await page.keyboard.press('Escape');
		const bar = page.locator('[title="ค่ายทั้งวัน"]');
		const timed = page.locator('[title="08:30 สอบช่วงเช้า"]');
		await expect(timed).toHaveCount(1);
		await expect(page.locator('[title="13:00 อบรมช่วงบ่าย"]')).toHaveCount(3);
		const styles = await timed.evaluate((el) => ({
			background: getComputedStyle(el).backgroundColor,
			dot: getComputedStyle(el.firstElementChild!).backgroundColor,
			column: el.style.gridColumn
		}));
		expect(styles.background).toBe('rgba(0, 0, 0, 0)');
		expect(styles.dot).toBe('rgb(37, 99, 235)');
		expect(styles.column).toContain('span 1');
		expect(await bar.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
			'rgb(37, 99, 235)'
		);
		expect(await timed.innerText()).toBe(width < 640 ? 'สอบช่วงเช้า' : '08:30 สอบช่วงเช้า');
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
		await timed.click();
		await expect(page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true })).toContainText(
			'08:30-09:30'
		);
		await page.getByRole('button', { name: 'แก้ไข สอบช่วงเช้า', exact: true }).click();
		await expect(page.getByRole('checkbox', { name: 'ทั้งวัน', exact: true })).not.toBeChecked();
		await expect(page.getByLabel('เวลาเริ่มต้น *', { exact: true })).toHaveValue('08:30');
		await expect(page.getByLabel('เวลาสิ้นสุด *', { exact: true })).toHaveValue('09:30');
		await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await bar.click();
		await page.getByRole('button', { name: 'แก้ไข ค่ายทั้งวัน', exact: true }).click();
		await expect(page.getByRole('checkbox', { name: 'ทั้งวัน', exact: true })).toBeChecked();
		await expect(page.getByLabel('เวลาเริ่มต้น *', { exact: true })).toHaveCount(0);
	});
}
