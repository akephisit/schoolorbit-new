import { expect, test } from '@playwright/test';
import { mockYearManagement, id, yearUrl } from './fixtures/year-management';
test.use({ serviceWorkers: 'block' });
test('policy administration retires into academic year management', async ({ page }) => {
	const observed = await mockYearManagement(page, { reader: true });
	await page.goto(`/staff/academic/promotion/policies?academicYearId=${id(1)}`);
	await expect(page).toHaveURL(yearUrl);
	expect(observed.reads.some((path) => path.includes('promotion-policies'))).toBe(false);
	await expect(page.getByRole('button', { name: /ยืนยันเกณฑ์|เพิ่มเกณฑ์/ })).toHaveCount(0);
});
