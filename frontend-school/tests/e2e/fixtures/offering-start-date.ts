import { expect, type Page } from '@playwright/test';

export const offeringStartsOn = '2026-10-15';

/** Choose the actual offering date explicitly in each preparation workflow. */
export async function selectOfferingStartDate(page: Page) {
	const trigger = page.getByRole('button', {
		name: 'วันที่เริ่มเปิดสอนจริงสำหรับรายการจากหลักสูตร',
		exact: true
	});
	await expect(trigger).toBeVisible();
	if (!(await trigger.textContent())?.includes('เลือกวันที่')) return;
	await page.clock.setFixedTime(new Date('2026-10-15T05:00:00Z'));
	await trigger.click();
	await page
		.locator('[data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: /^15$/ })
		.click();
}
