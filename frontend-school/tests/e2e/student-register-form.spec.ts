import { expect, test, type Page } from '@playwright/test';
import {
	mockStaffStudents,
	firstStudent,
	studentPath,
	year
} from './fixtures/staff-student-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});
async function fillCreate(page: Page) {
	await page.getByLabel('รหัสนักเรียน', { exact: true }).fill('00001');
	await page.getByLabel('ชื่อ', { exact: true }).fill('นักเรียนใหม่');
	await page.getByLabel('นามสกุล', { exact: true }).fill('ทดสอบ');
	await page.getByLabel('รหัสผ่าน', { exact: true }).fill('password123');
	await page.getByLabel('ยืนยันรหัสผ่าน', { exact: true }).fill('password123');
}

test('creation without a year ignores hidden guardian drafts and opens permanent detail', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto('/staff/students/new');
	await fillCreate(page);
	await page.getByRole('switch', { name: 'เพิ่มผู้ปกครองพร้อมนักเรียน' }).click();
	await expect(page.getByRole('region', { name: 'ผู้ปกครองคนที่ 1' })).toBeVisible();
	await page.getByRole('switch', { name: 'เพิ่มผู้ปกครองพร้อมนักเรียน' }).click();
	await page.getByRole('button', { name: 'เพิ่มนักเรียน', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/staff/students/${firstStudent}$`));
	expect(api.payloads[0].data?.student_id).toBe('00001');
	for (const field of [
		'parents',
		'parentsEnabled',
		'confirmPassword',
		'student_number',
		'grade_level',
		'homeroom'
	])
		expect(api.payloads[0].data).not.toHaveProperty(field);
	expect(api.reads).toHaveLength(1);
	expect(api.reads[0].searchParams.has('academicYearId')).toBe(false);
	await expect(page.getByTestId('student-academic')).toHaveCount(0);
});

test('creation refuses short passwords and whitespace names before sending a request', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto('/staff/students/new');
	await fillCreate(page);
	await page.getByLabel('ชื่อ', { exact: true }).fill('   ');
	await page.getByLabel('รหัสผ่าน', { exact: true }).fill('short');
	await page.getByRole('button', { name: 'เพิ่มนักเรียน', exact: true }).click();
	await expect(page.getByText('กรุณากรอกชื่อ', { exact: true })).toBeVisible();
	await expect(page.getByText('รหัสผ่านต้องมี 8–128 ตัวอักษร', { exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(0);
});

test('pending create prevents duplicate submission and ignores a completion after leaving', async ({
	page
}) => {
	const api = await mockStaffStudents(page, { hold: 'mutation' });
	await page.goto('/staff/students/new');
	await fillCreate(page);
	await page.getByRole('form', { name: 'เพิ่มทะเบียนนักเรียน' }).evaluate((element) => {
		(element as HTMLFormElement).requestSubmit();
		(element as HTMLFormElement).requestSubmit();
	});
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, '/staff/students');
	await expect(page.getByTestId('student-list')).toContainText('นักเรียนแรก');
	api.release();
	await expect(page).toHaveURL(/\/staff\/students$/);
	expect(api.writes).toHaveLength(1);
});

test('failed create retains the entered draft and exposes a durable error', async ({ page }) => {
	await mockStaffStudents(page, { failMutation: true });
	await page.goto('/staff/students/new');
	await fillCreate(page);
	await page.getByRole('button', { name: 'เพิ่มนักเรียน', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('บันทึกตัวอย่างไม่สำเร็จ');
	await expect(page.getByLabel('ชื่อ', { exact: true })).toHaveValue('นักเรียนใหม่');
	await expect(page.getByLabel('รหัสผ่าน', { exact: true })).toHaveValue('password123');
});

test('immediate permanent edit sends explicit clears with no annual or immutable fields', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	await expect(page.getByLabel('ชื่อ', { exact: true })).toBeEditable();
	await expect(page.getByRole('button', { name: 'แก้ไข', exact: true })).toHaveCount(0);
	await expect(page.getByRole('textbox', { name: 'รหัสนักเรียน', exact: true })).toHaveCount(0);
	await expect(page.getByLabel('ระดับชั้น', { exact: true })).toHaveCount(0);
	await expect(page.getByLabel('เลขที่', { exact: true })).toHaveCount(0);
	await page.getByLabel('ชื่อ', { exact: true }).fill('ชื่อใหม่');
	await page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/staff/students/${firstStudent}$`));
	const payload = api.payloads[0].data;
	for (const field of [
		'student_id',
		'student_number',
		'grade_level',
		'homeroom',
		'national_id',
		'password'
	])
		expect(payload).not.toHaveProperty(field);
	expect(payload).toMatchObject({
		first_name: 'ชื่อใหม่',
		title: null,
		date_of_birth: null,
		gender: null,
		email: null,
		phone: null,
		address: null,
		registration_status: 'studying'
	});
	expect(api.reads).toHaveLength(2);
});

test('failed refresh and failed save preserve the edit draft', async ({ page }) => {
	await mockStaffStudents(page, { fail: 'profile', failAt: 2, failMutation: true });
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.getByLabel('ชื่อ', { exact: true }).fill('ชื่อร่าง');
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByText('region นักเรียนไม่พร้อม', { exact: true })).toBeVisible();
	await expect(page.getByLabel('ชื่อ', { exact: true })).toHaveValue('ชื่อร่าง');
	await page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('บันทึกตัวอย่างไม่สำเร็จ');
	await expect(page.getByLabel('ชื่อ', { exact: true })).toHaveValue('ชื่อร่าง');
});

test('guardian workflow hides national ID when PII read permission is absent', async ({ page }) => {
	await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.getByRole('button', { name: 'เพิ่มผู้ปกครอง', exact: true }).click();
	await expect(page.getByRole('dialog').getByLabel('เลขบัตรประชาชน', { exact: true })).toHaveCount(
		0
	);
});

test('annual source survives permanent edit while malformed years cannot trigger annual reads', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '', year));
	await expect(page.getByTestId('student-academic')).toContainText('ห้องปีเดิม');
	await page.getByRole('link', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByLabel('ชื่อ', { exact: true })).toBeEditable();
	expect(api.reads.at(-1)?.searchParams.has('academicYearId')).toBe(false);
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await expect(page.getByTestId('student-academic')).toContainText('ห้องปีเดิม');
	await expect(page.getByRole('link', { name: 'ย้อนกลับ', exact: true })).toHaveAttribute(
		'href',
		`/staff/academic/student-years?academicYearId=${year}`
	);
	const malformed = new URLSearchParams({
		academicYearId: 'malformed',
		returnTo: '/staff/academic/student-years?academicYearId=malformed'
	});
	await navigate(page, `${studentPath()}?${malformed}`);
	await expect(page.getByTestId('student-academic')).toHaveCount(0);
	await expect(page.getByTestId('student-profile')).toContainText('นักเรียนแรก');
	expect(api.reads.at(-1)?.searchParams.has('academicYearId')).toBe(false);
});

for (const width of [375, 1280])
	for (const mode of ['create', 'edit'] as const) {
		test(`${mode} form fits ${width}px in both themes`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 950 });
			await mockStaffStudents(page);
			await page.goto(
				mode === 'create' ? '/staff/students/new' : studentPath(firstStudent, '/edit')
			);
			await expect(page.getByLabel('ชื่อ', { exact: true })).toBeEditable();
			for (const theme of ['light', 'dark']) {
				if (theme === 'dark') await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
					width
				);
				await page.screenshot({
					path: testInfo.outputPath(`student-${mode}-${width}-${theme}.png`),
					fullPage: true
				});
			}
		});
	}
