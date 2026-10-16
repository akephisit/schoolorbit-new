import { expect, test } from '@playwright/test';
import { listPath, mockStaffStudents } from './fixtures/staff-student-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});

for (const width of [375, 1440]) {
	for (const theme of ['light', 'dark']) {
		test(`directory remains readable at ${width}px in ${theme}`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 950 });
			await page.emulateMedia({ colorScheme: 'light' });
			const api = await mockStaffStudents(page, {
				listItem: {
					title: 'เด็กหญิง',
					first_name: 'ชื่อทดสอบที่ยาวสำหรับตรวจสอบการตัดบรรทัด',
					last_name: 'นามสกุลทดสอบที่ยาวเป็นพิเศษ',
					homeroom: null
				}
			});
			await page.goto(listPath());
			const table = page.getByRole('table', { name: 'รายชื่อนักเรียน' });
			await expect(table).toBeVisible();
			if (theme === 'dark') await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
			await expect(table.getByRole('link').first()).toContainText('เด็กหญิงชื่อทดสอบ');
			await expect(
				table.getByText('ม.1', { exact: false }).filter({ visible: true })
			).toBeVisible();
			for (const label of ['ดูข้อมูลนักเรียน', 'แก้ไขนักเรียน', 'ลบนักเรียน']) {
				const box = await table
					.getByRole(label === 'ลบนักเรียน' ? 'button' : 'link', { name: label })
					.boundingBox();
				expect(box).not.toBeNull();
				expect(box!.x + box!.width).toBeLessThanOrEqual(width);
			}
			const tableBox = await table.boundingBox();
			expect(tableBox!.x + tableBox!.width).toBeLessThanOrEqual(width);
			expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
				width
			);
			expect(api.reads).toHaveLength(1);
			await page.screenshot({
				path: testInfo.outputPath(`students-${width}-${theme}.png`),
				fullPage: true
			});
		});
	}
}

test('status selection preserves selection, URL and browser history', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(`${listPath()}&page=2`);
	await expect(page.getByRole('table')).toBeVisible();
	const group = page.getByRole('radiogroup', { name: 'สถานะนักเรียน' });
	await group.getByRole('radio', { name: 'ไม่ใช้งาน', exact: true }).click();
	await expect(page).toHaveURL(/status=inactive/);
	expect(new URL(page.url()).searchParams.has('page')).toBe(false);
	await expect(group.getByRole('radio', { name: 'ไม่ใช้งาน', exact: true })).toHaveAttribute(
		'aria-checked',
		'true'
	);
	const count = api.reads.length;
	await group.getByRole('radio', { name: 'ไม่ใช้งาน', exact: true }).click();
	await expect(group.getByRole('radio', { name: 'ไม่ใช้งาน', exact: true })).toHaveAttribute(
		'aria-checked',
		'true'
	);
	expect(api.reads).toHaveLength(count);
	await page.reload();
	await expect(group.getByRole('radio', { name: 'ไม่ใช้งาน', exact: true })).toHaveAttribute(
		'aria-checked',
		'true'
	);
	await group.getByRole('radio', { name: 'ทั้งหมด', exact: true }).focus();
	await page.keyboard.press('Space');
	await expect(page).toHaveURL(/status=all/);
	await page.goBack();
	await expect(group.getByRole('radio', { name: 'ไม่ใช้งาน', exact: true })).toHaveAttribute(
		'aria-checked',
		'true'
	);
});

test('an empty later page can return to the previous results', async ({ page }) => {
	await mockStaffStudents(page, { emptyPage: 2 });
	await page.goto(`${listPath()}&page=2`);
	await expect(page.getByText('ไม่มีนักเรียนในหน้านี้', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: '← ก่อนหน้า' }).click();
	await expect(page.getByRole('table')).toContainText('นักเรียนแรก');
});

test('search uses submitted filters for empty guidance and supports reset', async ({ page }) => {
	await mockStaffStudents(page, { emptyPage: 1 });
	await page.goto(`${listPath()}&search=ไม่พบ&status=inactive`);
	await page.getByLabel('ค้นหานักเรียน', { exact: true }).fill('');
	await expect(page.getByText('ไม่พบนักเรียนที่ตรงกับตัวกรอง', { exact: true })).toBeVisible();
	await page.getByTestId('student-list').getByRole('button', { name: 'ล้างตัวกรอง' }).click();
	await expect(page).toHaveURL(/status=active/);
	expect(new URL(page.url()).searchParams.has('search')).toBe(false);
});

test('detail return link restores search, status and page', async ({ page }) => {
	await mockStaffStudents(page);
	await page.goto(`${listPath()}&search=ชื่อ&status=all&page=2`);
	const directoryURL = page.url();
	await page.getByRole('link', { name: 'ดูข้อมูลนักเรียน' }).click();
	await expect(page.getByTestId('student-profile')).toContainText('นักเรียนแรก');
	await page.getByRole('link', { name: 'ย้อนกลับ' }).click();
	await expect(page).toHaveURL(directoryURL);
	await expect(page.getByLabel('ค้นหานักเรียน', { exact: true })).toHaveValue('ชื่อ');
});
