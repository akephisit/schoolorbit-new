import { expect, test } from '@playwright/test';
import {
	mockStaffStudents,
	listPath,
	studentPath,
	firstStudent,
	secondStudent,
	year,
	nextYear
} from './fixtures/staff-student-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
test('directory has one route-owned read and first skeleton without optional reads', async ({
	page
}) => {
	const api = await mockStaffStudents(page, { hold: 'list' });
	await page.goto(listPath());
	await expect(page.getByTestId('student-list').getByRole('status')).toBeVisible();
	api.release();
	await expect(page.getByText('นักเรียนแรก ทดสอบ', { exact: true })).toBeVisible();
	expect(api.count('/api/students')).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.reads).toHaveLength(1);
});
for (const route of ['list', 'profile', 'edit'] as const) {
	test(`${route} has focused first error and retry`, async ({ page }) => {
		const api = await mockStaffStudents(page, { fail: route === 'list' ? 'list' : 'profile' });
		await page.goto(
			route === 'list' ? listPath() : studentPath(firstStudent, route === 'edit' ? '/edit' : '')
		);
		const region = page.getByTestId(route === 'list' ? 'student-list' : 'student-profile');
		await expect(region).toContainText('region นักเรียนไม่พร้อม');
		await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(region).toContainText('นักเรียนแรก');
		expect(api.reads).toHaveLength(2);
		expect(api.count('/api/auth/me')).toBe(1);
	});
	test(`${route} retains visible data during failed refresh`, async ({ page }) => {
		await mockStaffStudents(page, { fail: route === 'list' ? 'list' : 'profile', failAt: 2 });
		await page.goto(
			route === 'list' ? listPath() : studentPath(firstStudent, route === 'edit' ? '/edit' : '')
		);
		const region = page.getByTestId(route === 'list' ? 'student-list' : 'student-profile');
		await expect(region).toContainText('นักเรียนแรก');
		await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
		await expect(region).toContainText('region นักเรียนไม่พร้อม');
		await expect(region).toContainText('นักเรียนแรก');
	});
}
test('directory paging uses URL and back restores the first page', async ({ page }) => {
	const api = await mockStaffStudents(page, { fullPage: true });
	await page.goto(listPath());
	await page.getByRole('button', { name: 'ถัดไป →' }).click();
	await expect(page).toHaveURL(/page=2/);
	await expect(page.getByTestId('student-list')).toContainText('นักเรียนหน้าสอง');
	await page.goBack();
	await expect(page.getByTestId('student-list')).toContainText('นักเรียนแรก');
	expect(api.reads.every((url) => url.searchParams.get('pageSize') === '20')).toBe(true);
});
test('directory search deep link and history keep context', async ({ page }) => {
	await mockStaffStudents(page);
	await page.goto(listPath());
	await page.getByPlaceholder('ค้นหาชื่อ หรือรหัสนักเรียน...').fill('คำใหม่');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page).toHaveURL(/search=/);
	await expect(page.getByTestId('student-list')).toContainText('ค้นหา คำใหม่');
	await page.goBack();
	await expect(page.getByTestId('student-list')).toContainText('นักเรียนแรก');
	await page.goForward();
	await expect(page.getByPlaceholder('ค้นหาชื่อ หรือรหัสนักเรียน...')).toHaveValue('คำใหม่');
});
test('directory rejects a late old-year response', async ({ page }) => {
	const api = await mockStaffStudents(page, { hold: 'list' });
	await page.goto(listPath());
	await expect(page.getByTestId('student-list').getByRole('status')).toBeVisible();
	await navigate(page, listPath(nextYear));
	await expect(page.getByTestId('student-list')).toContainText('นักเรียนปีถัดไป');
	api.release();
	await expect(page.getByTestId('student-list')).not.toContainText('นักเรียนแรก');
});
for (const suffix of ['', '/edit'])
	test(`selected ${suffix || 'detail'} rejects late old-person data`, async ({ page }) => {
		const api = await mockStaffStudents(page, { hold: 'profile' });
		await page.goto(studentPath(firstStudent, suffix));
		await expect(page.getByTestId('student-profile').getByRole('status')).toBeVisible();
		await navigate(page, studentPath(secondStudent, suffix));
		await expect(page.getByTestId('student-profile')).toContainText('นักเรียนที่สอง');
		api.release();
		await expect(page.getByTestId('student-profile')).not.toContainText('นักเรียนแรก');
	});
test('selected profile changes year and preserves contextual edit link', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath());
	await expect(page.getByTestId('student-profile')).toContainText('ห้องปีเดิม');
	await navigate(page, studentPath(firstStudent, '', nextYear));
	await expect(page.getByTestId('student-profile')).toContainText('ห้องปีถัดไป');
	await expect(page.getByRole('link', { name: 'แก้ไข', exact: true })).toHaveAttribute(
		'href',
		studentPath(firstStudent, '/edit', nextYear)
	);
	expect(api.reads).toHaveLength(2);
});
test('read-only editor loads one profile and no optional editor catalog', async ({ page }) => {
	const api = await mockStaffStudents(page, { permissions: ['student.read.assigned'] });
	await page.goto(studentPath(firstStudent, '/edit'));
	await expect(page.getByTestId('student-profile')).toContainText('นักเรียนแรก');
	await expect(page.getByText('อ่านได้อย่างเดียว', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มผู้ปกครอง' })).toHaveCount(0);
	expect(api.reads).toHaveLength(1);
});
test('write-only actor does not request a denied profile', async ({ page }) => {
	const api = await mockStaffStudents(page, { permissions: ['student.update.all'] });
	await page.goto(studentPath(firstStudent, '/edit'));
	await expect(page.getByText('ไม่มีสิทธิ์ดูข้อมูลนักเรียน', { exact: true })).toBeVisible();
	expect(api.reads).toHaveLength(0);
});
test('saving reloads only the selected profile after an empty mutation reply', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.locator('#first_name').fill('ชื่อที่แก้');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByTestId('student-profile')).toContainText('ชื่อที่แก้');
	expect(api.reads).toHaveLength(2);
	expect(api.writes).toEqual([{ method: 'PUT', path: `/api/students/${firstStudent}` }]);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/menu/user')).toBe(1);
});
test('late save cannot reset the next student or redirect', async ({ page }) => {
	const api = await mockStaffStudents(page, { hold: 'mutation' });
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.locator('#first_name').fill('ชื่อที่แก้');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, studentPath(secondStudent, '/edit'));
	await expect(page.getByTestId('student-profile')).toContainText('นักเรียนที่สอง');
	api.release();
	await expect(page.getByTestId('student-profile')).not.toContainText('ชื่อที่แก้');
	expect(api.reads).toHaveLength(2);
});
test('parent dialog is interaction-only and refreshes selected data after adding', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มผู้ปกครอง' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	expect(api.reads).toHaveLength(1);
	await dialog.locator('#p_first_name').fill('ผู้ปกครองใหม่');
	await dialog.locator('#p_last_name').fill('ทดสอบ');
	await dialog.locator('#p_phone').fill('0000000000');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(page.getByTestId('student-profile')).toContainText('ผู้ปกครองใหม่');
	expect(api.reads).toHaveLength(2);
	expect(api.writes[0].path).toBe(`/api/students/${firstStudent}/parents`);
});
test('parent removal refreshes only the selected profile', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	page.on('dialog', (dialog) => dialog.accept());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('button', { name: 'ลบผู้ปกครอง' }).click();
	await expect(page.getByTestId('student-profile')).not.toContainText('ผู้ปกครองเดิม');
	expect(api.reads).toHaveLength(2);
});
test('deleting navigates to the selected-year list without identity refresh', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	page.on('dialog', (dialog) => dialog.accept());
	await page.getByRole('button', { name: 'ลบ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/staff/students\\?academicYearId=${year}$`));
	await expect(page.getByTestId('student-list')).toBeVisible();
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/students')).toBe(1);
});
test('person links wait for tap rather than reading private details on hover', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(listPath());
	const link = page.getByRole('link', { name: 'ดูข้อมูลนักเรียน', exact: true });
	await link.hover();
	expect(api.count(`/api/students/${firstStudent}`)).toBe(0);
	await link.click();
	await expect(page.getByTestId('student-profile')).toContainText('นักเรียนแรก');
	expect(api.count(`/api/students/${firstStudent}`)).toBe(1);
});
test('same-person refresh keeps data and exposes updating status while pending', async ({
	page
}) => {
	const api = await mockStaffStudents(page, { hold: 'profile', holdAt: 2 });
	await page.goto(studentPath());
	const region = page.getByTestId('student-profile');
	await expect(region).toContainText('นักเรียนแรก');
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(region.getByRole('status')).toContainText('กำลังอัปเดต');
	await expect(region).toContainText('นักเรียนแรก');
	api.release();
	await expect(region.getByRole('status')).toHaveCount(0);
});
