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
					registration_status: 'graduated',
					account_status: 'inactive'
				}
			});
			await page.goto(listPath());
			const table = page.getByRole('table', { name: 'รายชื่อนักเรียน' });
			await expect(table).toBeVisible();
			if (theme === 'dark') await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
			await expect(table.getByRole('link').first()).toContainText('เด็กหญิงชื่อทดสอบ');
			await expect(table).toContainText('สำเร็จการศึกษา');
			await expect(table).toContainText('ไม่ใช้งาน');
			await expect(table).not.toContainText('ม.1');
			for (const label of ['ดูข้อมูลนักเรียน', 'แก้ไขนักเรียน']) {
				const box = await table.getByRole('link', { name: label }).boundingBox();
				expect(box).not.toBeNull();
				expect(box!.x + box!.width).toBeLessThanOrEqual(width);
			}
			await expect(table.getByRole('button', { name: 'ปิดบัญชีนักเรียน' })).toHaveCount(0);
			expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
				width
			);
			expect(api.reads).toHaveLength(1);
			expect(api.reads[0].searchParams.has('academicYearId')).toBe(false);
			await page.screenshot({
				path: testInfo.outputPath(`students-${width}-${theme}.png`),
				fullPage: true
			});
		});
	}
}

test('registration and account filters are distinct and survive reload and history', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(`${listPath()}?page=2&academicYearId=legacy&status=active`);
	await expect(page.getByRole('table')).toBeVisible();
	await page.getByLabel('สถานะทะเบียน', { exact: true }).click();
	await page.getByRole('option', { name: 'สำเร็จการศึกษา', exact: true }).click();
	await expect(page).toHaveURL(/registrationStatus=graduated/);
	expect(new URL(page.url()).searchParams.has('page')).toBe(false);
	expect(new URL(page.url()).searchParams.has('academicYearId')).toBe(false);
	await page.getByLabel('สถานะบัญชี', { exact: true }).click();
	await page.getByRole('option', { name: 'ไม่ใช้งาน', exact: true }).click();
	await expect(page).toHaveURL(/accountStatus=inactive/);
	await page.reload();
	await expect(page.getByLabel('สถานะทะเบียน', { exact: true })).toContainText('สำเร็จการศึกษา');
	await expect(page.getByLabel('สถานะบัญชี', { exact: true })).toContainText('ไม่ใช้งาน');
	await page.goBack();
	await expect(page.getByLabel('สถานะบัญชี', { exact: true })).toContainText('ทั้งหมด');
	expect(
		api.reads.every(
			(url) => !url.searchParams.has('academicYearId') && !url.searchParams.has('status')
		)
	).toBe(true);
});

test('an empty later page can return to the previous results', async ({ page }) => {
	await mockStaffStudents(page, { emptyPage: 2 });
	await page.goto(`${listPath()}?page=2`);
	await expect(page.getByText('ไม่มีนักเรียนในหน้านี้', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: '← ก่อนหน้า' }).click();
	await expect(page.getByRole('table')).toContainText('นักเรียนแรก');
});

test('partial permanent records remain discoverable without code, date or login name', async ({
	page
}) => {
	const api = await mockStaffStudents(page, {
		listItem: {
			student_id: null,
			enrollment_date: null,
			username: null,
			registration_status: 'unrecorded'
		},
		profile: {
			student_id: null,
			enrollment_date: null,
			username: null,
			registration_status: 'unrecorded'
		}
	});
	await page.goto(listPath());
	await expect(page.getByRole('table')).toContainText('ยังไม่มีรหัส');
	await expect(page.getByRole('table')).toContainText('ยังไม่ระบุ');
	await page.getByRole('link', { name: 'ดูข้อมูลนักเรียน', exact: true }).click();
	await expect(page.getByTestId('student-summary')).toContainText('ยังไม่ระบุชื่อบัญชี');
	expect(api.reads.every((url) => !url.searchParams.has('academicYearId'))).toBe(true);
});

test('submitted filters drive empty guidance and reset clears both status categories', async ({
	page
}) => {
	await mockStaffStudents(page, { emptyPage: 1 });
	await page.goto(`${listPath()}?search=ไม่พบ&registrationStatus=withdrawn&accountStatus=inactive`);
	await page.getByLabel('ค้นหานักเรียน', { exact: true }).fill('');
	await expect(page.getByText('ไม่พบนักเรียนที่ตรงกับตัวกรอง', { exact: true })).toBeVisible();
	await page.getByTestId('student-list').getByRole('button', { name: 'ล้างตัวกรอง' }).click();
	await expect(page).toHaveURL(/\/staff\/students$/);
});

test('detail and immediate edit retain the full registry return context', async ({ page }) => {
	await mockStaffStudents(page);
	await page.goto(
		`${listPath()}?search=ชื่อ&registrationStatus=graduated&accountStatus=inactive&page=2`
	);
	const directoryURL = page.url();
	await page.getByRole('link', { name: 'ดูข้อมูลนักเรียน' }).click();
	await expect(page.getByTestId('student-profile')).toContainText('นักเรียนแรก');
	await expect(page.getByTestId('student-academic')).toHaveCount(0);
	await page.getByRole('link', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByRole('textbox', { name: 'ชื่อ', exact: true })).toBeEditable();
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
	await expect(page).toHaveURL(directoryURL);
});

test.describe('touch directory filters', () => {
	test.use({ viewport: { width: 375, height: 900 }, hasTouch: true, isMobile: true });
	test('registration status can be selected without writes', async ({ page }) => {
		const api = await mockStaffStudents(page);
		await page.goto(listPath());
		await page.getByLabel('สถานะทะเบียน', { exact: true }).tap();
		await page.getByRole('option', { name: 'ย้ายออก', exact: true }).tap();
		await expect(page.getByLabel('สถานะทะเบียน', { exact: true })).toContainText('ย้ายออก');
		expect(api.writes).toHaveLength(0);
	});
});
