import { test, expect } from '@playwright/test';
import { mockCalendar, calendarPath, eventsPath } from './fixtures/calendar-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await page.route('https://fonts.**', (route) => route.abort());
});

for (const width of [1280, 375]) {
	for (const scope of ['manager', 'requester', 'both', 'reader']) {
		test(`calendar menus honor ${scope} permissions at ${width}px`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			const canCreate = scope === 'manager' || scope === 'both';
			const canRequest = scope === 'requester' || scope === 'both';
			const api = await mockCalendar(page, {
				permissions: [
					'calendar.read.school',
					...(canCreate ? ['calendar.manage.school'] : []),
					...(canRequest ? ['calendar.request.own'] : [])
				]
			});
			await page.goto(calendarPath());
			await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
			const plus = page.getByRole('button', { name: 'เพิ่มรายการปฏิทิน', exact: true });
			await expect(plus).toHaveCount(canCreate || canRequest ? 1 : 0);
			if (canCreate || canRequest) {
				await plus.click();
				await expect(page.getByRole('menuitem', { name: 'เพิ่มกิจกรรม', exact: true })).toHaveCount(
					canCreate ? 1 : 0
				);
				await expect(
					page.getByRole('menuitem', { name: 'คำร้องขอเพิ่มกิจกรรม', exact: true })
				).toHaveCount(canRequest ? 1 : 0);
				await page.screenshot({ path: testInfo.outputPath(`create-${scope}-${width}.png`) });
				await page.keyboard.press('Escape');
				await expect(plus).toBeFocused();
			}
			await page.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
			await expect(
				page.getByRole('menuitem', { name: 'คัดลอกลิงก์สาธารณะ', exact: true })
			).toBeVisible();
			await expect(
				page.getByRole('menuitem', { name: 'ฝังในเว็บไซต์', exact: true })
			).toBeVisible();
			await expect(
				page.getByRole('menuitem', { name: 'หมวดหมู่และแท็ก', exact: true })
			).toHaveCount(canCreate ? 1 : 0);
			await page.addStyleTag({
				content: '*,*::before,*::after {transition:none!important;animation:none!important}'
			});
			await page.screenshot({ path: testInfo.outputPath(`settings-${scope}-${width}-light.png`) });
			await page.evaluate(() => document.documentElement.classList.add('dark'));
			await page.screenshot({ path: testInfo.outputPath(`settings-${scope}-${width}-dark.png`) });
			await page.keyboard.press('Escape');
			await page.locator('[data-calendar-date="2026-10-12"]').click();
			const popup = page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true });
			await expect(popup).toBeVisible();
			const dayPlus = popup.getByRole('button', { name: 'เพิ่มรายการในวันที่เลือก', exact: true });
			await expect(dayPlus).toHaveCount(canCreate || canRequest ? 1 : 0);
			if (canCreate || canRequest) {
				await dayPlus.click();
				await page
					.getByRole('menuitem', {
						name: canRequest ? 'คำร้องขอเพิ่มกิจกรรม' : 'เพิ่มกิจกรรม',
						exact: true
					})
					.click();
				await expect(popup).toHaveCount(0);
				const form = page.getByRole('dialog');
				await expect(form).toBeVisible();
				if (canRequest) {
					await expect(
						form.getByRole('button', { name: 'วันที่เริ่มกิจกรรม', exact: true })
					).toContainText('12');
					await expect(
						form.getByRole('checkbox', { name: 'ทั้งวัน', exact: true })
					).not.toBeChecked();
					await expect(form.getByLabel('เวลาเริ่ม *', { exact: true })).toHaveValue('');
					await expect(form.getByLabel('เวลาเริ่ม *', { exact: true })).toHaveAttribute(
						'required',
						''
					);
				} else {
					await expect
						.poll(() =>
							api.reads
								.filter((url) => url.pathname === '/api/calendar/target-options')
								.at(-1)
								?.searchParams.get('date')
						)
						.toBe('2026-10-12');
				}
			}
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
		});
	}
}

test('catalog failure disables direct creation and management while requests remain available', async ({
	page
}) => {
	await mockCalendar(page, {
		fail: 'categories',
		permissions: ['calendar.read.school', 'calendar.manage.school', 'calendar.request.own']
	});
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'เพิ่มรายการปฏิทิน', exact: true }).click();
	await expect(page.getByRole('menuitem', { name: 'เพิ่มกิจกรรม', exact: true })).toHaveAttribute(
		'aria-disabled',
		'true'
	);
	await page.getByRole('menuitem', { name: 'คำร้องขอเพิ่มกิจกรรม', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText(
		'กิจกรรมจะขึ้นปฏิทินหลังผู้ดูแลตรวจและอนุมัติ'
	);
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect(
		page.getByRole('menuitem', { name: 'หมวดหมู่และแท็ก', exact: true })
	).toHaveAttribute('aria-disabled', 'true');
});

test('empty month keeps the calendar without a second empty box below it', async ({ page }) => {
	await mockCalendar(page);
	await page.route(
		(url) => url.pathname === eventsPath,
		(route) => route.fulfill({ json: { success: true, data: [] } })
	);
	await page.goto(calendarPath());
	await expect(page.locator('[data-calendar-date]')).toHaveCount(42);
	await expect(page.getByRole('button', { name: /^ดูรายละเอียด / })).toHaveCount(0);
	await expect(page.getByText('ยังไม่มีกิจกรรม', { exact: true })).toHaveCount(0);
	await page.locator('[data-calendar-date="2026-10-12"]').click();
	await expect(page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true })).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'เพิ่มรายการในวันที่เลือก', exact: true })
	).toBeVisible();
});

test('settings copy the public link and open the embed tool', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	await mockCalendar(page, { permissions: ['calendar.read.school'] });
	await page.goto(calendarPath());
	await page.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await page.getByRole('menuitem', { name: 'คัดลอกลิงก์สาธารณะ', exact: true }).click();
	await expect(page.getByText('คัดลอกลิงก์ปฏิทินสาธารณะแล้ว', { exact: true })).toBeVisible();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toMatch(/\/calendar(?:\?|$)/);
	await page.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ฝังในเว็บไซต์', exact: true }).click();
	await expect(page.locator('#calendar-embed-code')).toHaveValue(/<iframe/);
	await expect(page.getByRole('dialog')).toContainText('WordPress');
});
