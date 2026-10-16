import { revalidatePage, expectSilentBackground } from './fixtures/page-revalidation';
import { expect, test, type Locator } from '@playwright/test';
import {
	mockStaffStudents,
	listPath,
	studentPath,
	firstStudent,
	secondStudent,
	year,
	nextYear
} from './fixtures/staff-student-route-data';
import { id } from './fixtures/staff-home-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
async function expectStudentName(region: Locator, name: string, editing: boolean) {
	if (editing) {
		const input = region.getByRole('textbox', { name: 'ชื่อ', exact: true });
		await expect(input).toHaveValue(name);
		await expect(input).toBeEditable();
	} else await expect(region).toContainText(name);
}
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
		await expectStudentName(region, 'นักเรียนแรก', route === 'edit');
		expect(api.reads).toHaveLength(2);
		expect(api.count('/api/auth/me')).toBe(1);
	});
	test(`${route} retains visible data during failed refresh`, async ({ page }) => {
		await mockStaffStudents(page, { fail: route === 'list' ? 'list' : 'profile', failAt: 2 });
		await page.goto(
			route === 'list' ? listPath() : studentPath(firstStudent, route === 'edit' ? '/edit' : '')
		);
		const region = page.getByTestId(route === 'list' ? 'student-list' : 'student-profile');
		await expectStudentName(region, 'นักเรียนแรก', route === 'edit');
		if (route === 'edit')
			await region.getByRole('textbox', { name: 'ชื่อ', exact: true }).fill('ชื่อร่างที่ต้องเก็บ');
		await revalidatePage(page);
		await expect(region).toContainText('region นักเรียนไม่พร้อม');
		await expectStudentName(
			region,
			route === 'edit' ? 'ชื่อร่างที่ต้องเก็บ' : 'นักเรียนแรก',
			route === 'edit'
		);
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
test('directory rejects a late old-filter response', async ({ page }) => {
	const api = await mockStaffStudents(page, { hold: 'list' });
	await page.goto(listPath());
	await expect(page.getByTestId('student-list').getByRole('status')).toBeVisible();
	await navigate(page, `${listPath()}?search=คำใหม่`);
	await expect(page.getByTestId('student-list')).toContainText('ค้นหา คำใหม่');
	api.release();
	await expect(page.getByTestId('student-list')).not.toContainText('นักเรียนแรก');
});
for (const suffix of ['', '/edit'])
	test(`selected ${suffix || 'detail'} rejects late old-person data`, async ({ page }) => {
		const api = await mockStaffStudents(page, { hold: 'profile' });
		await page.goto(studentPath(firstStudent, suffix));
		await expect(page.getByTestId('student-profile').getByRole('status')).toBeVisible();
		await navigate(page, studentPath(secondStudent, suffix));
		const region = page.getByTestId('student-profile');
		await expectStudentName(region, 'นักเรียนที่สอง', suffix === '/edit');
		const destination = page.url();
		api.release();
		await expectStudentName(region, 'นักเรียนที่สอง', suffix === '/edit');
		if (suffix === '/edit')
			await expect(region.getByRole('textbox', { name: 'ชื่อ', exact: true })).not.toHaveValue(
				'นักเรียนแรก'
			);
		else await expect(region).not.toContainText('นักเรียนแรก');
		await expect(page).toHaveURL(destination);
		expect(api.reads).toHaveLength(2);
		expect(api.writes).toHaveLength(0);
	});
test('selected profile changes year and preserves contextual edit link', async ({ page }) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '', year));
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
	await expect(page.getByText('อ่านได้อย่างเดียว', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มผู้ปกครอง' })).toHaveCount(0);
	await expect.poll(() => api.reads.length).toBe(1);
	await expect(page.getByRole('textbox', { name: 'ชื่อ', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'ย้อนกลับ', exact: true })).toHaveAttribute(
		'href',
		studentPath(firstStudent)
	);
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
	await page.locator('#student-first_name').fill('ชื่อที่แก้');
	await page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(page.getByTestId('student-profile')).toContainText('ชื่อที่แก้');
	expect(api.reads).toHaveLength(2);
	expect(api.writes).toEqual([{ method: 'PUT', path: `/api/students/${firstStudent}` }]);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/menu/user')).toBe(1);
});
test('late save cannot reset the next student or redirect', async ({ page }) => {
	const api = await mockStaffStudents(page, { hold: 'mutation' });
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.locator('#student-first_name').fill('ชื่อที่แก้');
	await page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, studentPath(secondStudent, '/edit'));
	const name = page.getByRole('textbox', { name: 'ชื่อ', exact: true });
	await expect(name).toHaveValue('นักเรียนที่สอง');
	const destination = page.url();
	api.release();
	await expect(name).toHaveValue('นักเรียนที่สอง');
	await expect(name).not.toHaveValue('ชื่อที่แก้');
	await expect(page).toHaveURL(destination);
	expect(api.writes).toEqual([{ method: 'PUT', path: `/api/students/${firstStudent}` }]);
	expect(api.reads).toHaveLength(2);
});
test('parent dialog is interaction-only and refreshes selected data after adding', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	await page.getByRole('button', { name: 'เพิ่มผู้ปกครอง' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	expect(api.reads).toHaveLength(1);
	await dialog.locator('#student-parent-first_name').fill('ผู้ปกครองใหม่');
	await dialog.locator('#student-parent-last_name').fill('ทดสอบ');
	await dialog.locator('#student-parent-phone').fill('0000000000');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(page.getByTestId('student-profile')).toContainText('ผู้ปกครองใหม่');
	expect(api.reads).toHaveLength(2);
	expect(api.writes[0].path).toBe(`/api/students/${firstStudent}/parents`);
});
test('parent removal patches the selected profile without refetching or resetting its draft', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(studentPath(firstStudent, '/edit'));
	const name = page.getByRole('textbox', { name: 'ชื่อ', exact: true });
	await expect(name).toHaveValue('นักเรียนแรก');
	await name.fill('ชื่อร่างที่ต้องเก็บ');
	await page.getByRole('button', { name: 'ลบผู้ปกครอง' }).click();
	const confirmation = page.getByRole('alertdialog');
	await expect(confirmation).toBeVisible();
	await confirmation.getByRole('button', { name: 'นำออก', exact: true }).click();
	await expect(confirmation).not.toBeVisible();
	await expect(page.getByTestId('student-profile')).not.toContainText('ผู้ปกครองเดิม');
	await expect(name).toHaveValue('ชื่อร่างที่ต้องเก็บ');
	expect(api.reads).toHaveLength(1);
	expect(api.writes).toEqual([
		{ method: 'DELETE', path: `/api/students/${firstStudent}/parents/${id(65)}` }
	]);
});
test('account deactivation keeps the permanent directory and refreshes only its region', async ({
	page
}) => {
	const api = await mockStaffStudents(page);
	await page.goto(listPath());
	await page.getByRole('button', { name: 'ปิดบัญชีนักเรียน', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toContainText('การเข้าร่วมปีการศึกษาที่ยังแก้ไขได้');
	await page
		.getByRole('alertdialog')
		.getByRole('button', { name: 'ปิดบัญชี', exact: true })
		.click();
	await expect(page.getByRole('alertdialog')).not.toBeVisible();
	await expect(page).toHaveURL(/\/staff\/students$/);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/students')).toBe(2);
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
test('same-person page revalidation keeps data and updates silently', async ({ page }) => {
	const api = await mockStaffStudents(page, { hold: 'profile', holdAt: 2 });
	await page.goto(studentPath());
	const region = page.getByTestId('student-profile');
	await expect(region).toContainText('นักเรียนแรก');
	await revalidatePage(page);
	await expectSilentBackground(page, region);
	await expect(region).toContainText('นักเรียนแรก');
	api.release();
	await expect(region.getByRole('status')).toHaveCount(0);
});
for (const width of [375, 1280])
	test(`placement history streams and retries independently at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockStaffStudents(page, {
			permissions: [
				'student.read.school',
				'student_academic_year.read.school',
				'homeroom.read.school'
			]
		});
		let release = () => {};
		const held = new Promise<void>((resolve) => (release = resolve));
		let years = 0,
			rooms = 0,
			placements = 0;
		await page.route(
			(url) => url.pathname.startsWith('/api/academic/'),
			async (route) => {
				const url = new URL(route.request().url());
				if (url.pathname === '/api/academic/context/options') return route.fallback();
				if (url.pathname === '/api/academic/student-years') {
					years += 1;
					expect(url.searchParams.get('studentId')).toBe(firstStudent);
					expect(url.searchParams.get('academicYearId')).toBe(year);
					if (years === 1) {
						await held;
						return route.fulfill({
							status: 503,
							contentType: 'application/json',
							body: JSON.stringify({ success: false, error: 'ประวัติยังไม่พร้อม' })
						});
					}
					return route.fulfill({
						contentType: 'application/json',
						body: JSON.stringify({
							success: true,
							data: [
								{
									id: '55000000-0000-4000-8000-000000000088',
									studentId: firstStudent,
									academicYearId: year,
									gradeLevelName: 'มัธยมศึกษาปีที่ 1',
									studyProgramName: 'แผนหลัก',
									status: 'active'
								}
							]
						})
					});
				}
				if (url.pathname === '/api/academic/homerooms') rooms += 1;
				else if (url.pathname.endsWith('/placements')) placements += 1;
				else return route.fallback();
				return route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({ success: true, data: [] })
				});
			}
		);
		await page.goto(studentPath(firstStudent, '', year));
		const history = page.getByTestId('student-placement-history');
		try {
			await expect(page.getByTestId('student-profile')).toContainText('นักเรียนแรก');
			await expect(history).toHaveAttribute('aria-busy', 'true');
			await expect(history.getByRole('table')).toBeVisible();
		} finally {
			release();
		}
		await expect(history).toContainText('ประวัติยังไม่พร้อม');
		await page.screenshot({
			path: `/tmp/schoolorbit-profile-history-${width}-error.png`,
			fullPage: true
		});
		await history.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(history).toContainText('มัธยมศึกษาปีที่ 1');
		await expect(history).toContainText('ยังไม่มีประวัติการจัดห้องในปีนี้');
		expect(api.count(`/api/students/${firstStudent}`)).toBe(1);
		expect({ years, rooms, placements }).toEqual({ years: 2, rooms: 2, placements: 1 });
		await page.screenshot({
			path: `/tmp/schoolorbit-profile-history-${width}-light.png`,
			fullPage: true
		});
		await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
		await page.screenshot({
			path: `/tmp/schoolorbit-profile-history-${width}-dark.png`,
			fullPage: true
		});
	});
