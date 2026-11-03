import { expect, test } from '@playwright/test';
import { mockYearManagement, yearUrl, openRoster } from './fixtures/year-management';
test.use({ serviceWorkers: 'block' });
test('readers inspect the year and roster without manage-only options or commands', async ({
	page
}) => {
	const observed = await mockYearManagement(page, { reader: true });
	await page.goto(yearUrl);
	await expect(page.getByRole('heading', { name: 'จัดการปีการศึกษา', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ขึ้นปีการศึกษาใหม่', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true })).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: 'ดูนโยบายและขอบเขตข้อมูล', exact: true })
	).toHaveCount(0);
	expect(observed.reads).not.toContain('/api/academic/setup/workspace');
	expect(
		observed.reads.some(
			(path) =>
				path.endsWith('/retention') ||
				(path.endsWith('/options') && path.includes('year-rollovers'))
		)
	).toBe(false);
	await openRoster(page);
	await expect(
		page.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true })
	).toHaveCount(0);
});
test('retention loads on demand and saves a future-only policy without a purge request', async ({
	page
}) => {
	const observed = await mockYearManagement(page);
	await page.goto(yearUrl);
	await expect(
		page.getByRole('button', { name: 'ดูนโยบายและขอบเขตข้อมูล', exact: true })
	).toBeVisible();
	expect(observed.reads.some((path) => path.endsWith('/retention'))).toBe(false);
	await page.getByRole('button', { name: 'ดูนโยบายและขอบเขตข้อมูล', exact: true }).click();
	await expect(
		page.getByText('รายละเอียดถูกเก็บกวาดทั้งภาคเรียนพร้อมกัน', { exact: false })
	).toBeVisible();
	await expect(
		page.getByText('ภาคเรียนเดิมอยู่นอกช่วงที่เปิดใช้นโยบาย', { exact: true })
	).toBeVisible();
	await page.getByRole('switch', { name: 'เปิดเก็บกวาดรายละเอียดอัตโนมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกระยะเวลาเก็บรายละเอียด', exact: true }).click();
	await expect(
		page.getByText('บันทึกนโยบายแล้ว การบันทึกนี้ไม่ลบรายละเอียด', { exact: true })
	).toBeVisible();
	expect(observed.policies).toEqual([
		{
			expectedRowVersion: 1,
			enabled: true,
			primaryBoundary: 'year',
			secondaryBoundary: 'term',
			graceDays: 90
		}
	]);
	expect(observed.confirms).toEqual([]);
});
test('independent setup remains visible while year summary is pending', async ({ page }) => {
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	await mockYearManagement(page, { delayWorkspace: gate });
	try {
		await page.goto(yearUrl);
		await expect(page.locator('[data-testid="academic-setup-ready"]')).toBeVisible();
		await expect(
			page.getByRole('region', { name: 'จัดการรอบการศึกษา' }).locator('[data-slot="skeleton"]')
		).not.toHaveCount(0);
	} finally {
		release();
	}
});
