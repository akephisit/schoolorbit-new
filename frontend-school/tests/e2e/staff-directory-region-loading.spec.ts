import { expect, test, type Page } from '@playwright/test';
import {
	mockStaffDirectory,
	directoryPath,
	staffPath,
	firstStaff,
	secondStaff,
	roleId,
	organizationId
} from './fixtures/staff-directory-route-data';
import { navigate } from './fixtures/supervision-route-data';
import { actor } from './fixtures/staff-home-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const profileEndpoint = `/api/staff/${firstStaff}`;
const rolesEndpoint = `/api/users/${firstStaff}/roles`;
const permissionsEndpoint = `/api/users/${firstStaff}/permissions`;
async function fillCreate(page: Page) {
	await page.getByPlaceholder('ชื่อ', { exact: true }).fill('บุคลากรใหม่');
	await page.getByPlaceholder('นามสกุล', { exact: true }).fill('ทดสอบ');
	await page.locator('input[type=password]').nth(0).fill('synthetic-passphrase');
	await page.locator('input[type=password]').nth(1).fill('synthetic-passphrase');
}
async function next(page: Page, count = 1) {
	for (let index = 0; index < count; index++)
		await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
}
test('staff list starts once without editor options', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'list' });
	await page.goto(directoryPath);
	await expect(page.getByTestId('staff-directory').getByRole('status')).toBeVisible();
	api.release();
	await expect(page.getByTestId('staff-directory')).toContainText('บุคลากรแรก');
	expect(api.reads).toHaveLength(1);
	expect(api.count('/api/auth/me')).toBe(1);
});
test('list focused retry and retained refresh error', async ({ page }) => {
	const api = await mockStaffDirectory(page, { fail: 'list', failAt: 2 });
	await page.goto(directoryPath);
	await expect(page.getByTestId('staff-directory')).toContainText('บุคลากรแรก');
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('staff-directory')).toContainText('region list ไม่พร้อม');
	await expect(page.getByTestId('staff-directory')).toContainText('บุคลากรแรก');
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	expect(api.count('/api/staff')).toBe(3);
});
test('list search and paging follow URL history', async ({ page }) => {
	await mockStaffDirectory(page, { fullPage: true });
	await page.goto(directoryPath);
	await page.getByPlaceholder('ค้นหาชื่อ, นามสกุล...').fill('คำใหม่');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect(page).toHaveURL(/search=/);
	await expect(page.getByTestId('staff-directory')).toContainText('ค้นหา คำใหม่');
	await page.goBack();
	await expect(page.getByTestId('staff-directory')).toContainText('บุคลากรแรก');
	await page.getByRole('button', { name: /ถัดไป/ }).click();
	await expect(page).toHaveURL(/page=2/);
	await expect(page.getByTestId('staff-directory')).toContainText('บุคลากรหน้าสอง');
});
for (const kind of ['profile', 'achievements'] as const) {
	test(`selected ${kind} can be slow without blocking its sibling`, async ({ page }) => {
		const api = await mockStaffDirectory(page, { hold: kind });
		await page.goto(staffPath());
		const slow = page.getByTestId(kind === 'profile' ? 'staff-profile' : 'staff-achievements');
		await expect(slow.getByRole('status')).toBeVisible();
		await expect(
			page.getByTestId(kind === 'profile' ? 'staff-achievements' : 'staff-profile')
		).toContainText(kind === 'profile' ? 'ผลงานที่พร้อม' : 'บุคลากรแรก');
		api.release();
		await expect(slow).toContainText(kind === 'profile' ? 'บุคลากรแรก' : 'ผลงานที่พร้อม');
		expect(api.count(profileEndpoint)).toBe(1);
		expect(api.count('/api/achievements')).toBe(1);
		expect(api.count('/api/roles')).toBe(0);
	});
	test(`selected ${kind} has focused first error and retry`, async ({ page }) => {
		const api = await mockStaffDirectory(page, { fail: kind });
		await page.goto(staffPath());
		const region = page.getByTestId(kind === 'profile' ? 'staff-profile' : 'staff-achievements');
		await expect(region).toContainText(`region ${kind} ไม่พร้อม`);
		await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(region).toContainText(kind === 'profile' ? 'บุคลากรแรก' : 'ผลงานที่พร้อม');
		expect(api.count(profileEndpoint)).toBe(kind === 'profile' ? 2 : 1);
		expect(api.count('/api/achievements')).toBe(kind === 'achievements' ? 2 : 1);
	});
}
test('late selected profile cannot paint another person', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'profile' });
	await page.goto(staffPath());
	await expect(page.getByTestId('staff-profile').getByRole('status')).toBeVisible();
	await navigate(page, staffPath(secondStaff));
	await expect(page.getByTestId('staff-profile')).toContainText('บุคลากรที่สอง');
	api.release();
	await expect(page.getByTestId('staff-profile')).not.toContainText('บุคลากรแรก');
});
test('read-only profile has no achievement or mutation option requests', async ({ page }) => {
	const api = await mockStaffDirectory(page, { permissions: ['staff_profile.read.school'] });
	await page.goto(staffPath());
	await expect(page.getByTestId('staff-profile')).toContainText('บุคลากรแรก');
	await expect(page.getByTestId('staff-achievements')).toContainText('ไม่มีสิทธิ์ดูผลงาน');
	expect(api.count('/api/achievements')).toBe(0);
	expect(api.count('/api/roles')).toBe(0);
});
for (const kind of ['assignments', 'permissions'] as const) {
	test(`user ${kind} starts independently and retries alone`, async ({ page }) => {
		const api = await mockStaffDirectory(page, { fail: kind });
		await page.goto(staffPath(firstStaff, '/roles'));
		const region = page.getByTestId(
			kind === 'assignments' ? 'user-role-assignments' : 'user-effective-permissions'
		);
		await expect(region).toContainText(`region ${kind} ไม่พร้อม`);
		await expect(
			page.getByTestId(
				kind === 'assignments' ? 'user-effective-permissions' : 'user-role-assignments'
			)
		).toContainText(kind === 'assignments' ? 'staff_profile.read.own' : 'บทบาทตัวเลือก');
		await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(region).toContainText(
			kind === 'assignments' ? 'บทบาทตัวเลือก' : 'staff_profile.read.own'
		);
		expect(api.count(rolesEndpoint)).toBe(kind === 'assignments' ? 2 : 1);
		expect(api.count(permissionsEndpoint)).toBe(kind === 'permissions' ? 2 : 1);
		expect(api.count('/api/roles')).toBe(0);
	});
	test(`user ${kind} has a first skeleton while its sibling is ready`, async ({ page }) => {
		const api = await mockStaffDirectory(page, { hold: kind });
		await page.goto(staffPath(firstStaff, '/roles'));
		await expect(
			page
				.getByTestId(
					kind === 'assignments' ? 'user-role-assignments' : 'user-effective-permissions'
				)
				.getByRole('status')
		).toBeVisible();
		await expect(
			page.getByTestId(
				kind === 'assignments' ? 'user-effective-permissions' : 'user-role-assignments'
			)
		).toContainText(kind === 'assignments' ? 'staff_profile.read.own' : 'บทบาทตัวเลือก');
		api.release();
	});
}
test('read-only role manager has no assignment catalog or buttons', async ({ page }) => {
	const api = await mockStaffDirectory(page, { permissions: ['roles.read.all'] });
	await page.goto(staffPath(firstStaff, '/roles'));
	await expect(page.getByTestId('user-role-assignments')).toContainText('บทบาทตัวเลือก');
	await expect(page.getByRole('button', { name: 'เพิ่มบทบาท', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เพิกถอน' })).toHaveCount(0);
	expect(api.count('/api/roles')).toBe(0);
});
test('write-only role manager does not request denied regions or catalog', async ({ page }) => {
	const api = await mockStaffDirectory(page, { permissions: ['roles.assign.all'] });
	await page.goto(staffPath(firstStaff, '/roles'));
	await expect(page.getByTestId('user-role-assignments')).toContainText('ไม่มีสิทธิ์อ่านบทบาท');
	await page.getByRole('button', { name: 'เพิ่มบทบาท', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('ไม่มีสิทธิ์อ่านตัวเลือกบทบาท');
	expect(api.reads).toHaveLength(0);
});
test('assignment picker is lazy and has focused retry', async ({ page }) => {
	const api = await mockStaffDirectory(page, { fail: 'catalog' });
	await page.goto(staffPath(firstStaff, '/roles'));
	await expect(page.getByTestId('user-role-assignments')).toContainText('บทบาทตัวเลือก');
	expect(api.count('/api/roles')).toBe(0);
	await page.getByRole('button', { name: 'เพิ่มบทบาท', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toContainText('region catalog ไม่พร้อม');
	await dialog.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(dialog.locator('#role')).toBeVisible();
	expect(api.count('/api/roles')).toBe(2);
	expect(api.count(rolesEndpoint)).toBe(1);
	expect(api.count(permissionsEndpoint)).toBe(1);
});
test('role assignment refreshes only the affected assignments and effective permissions', async ({
	page
}) => {
	const api = await mockStaffDirectory(page);
	await page.goto(staffPath(firstStaff, '/roles'));
	await page.getByRole('button', { name: 'เพิ่มบทบาท', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.locator('#role').click();
	await page.getByRole('option', { name: 'บทบาทใหม่ (SYNTHETIC_NEW_ROLE)' }).click();
	await dialog.getByRole('button', { name: 'มอบหมาย', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(page.getByTestId('user-role-assignments')).toContainText('บทบาทใหม่');
	expect(api.count(rolesEndpoint)).toBe(2);
	expect(api.count(permissionsEndpoint)).toBe(2);
	expect(api.count('/api/roles')).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
});
test('role removal refreshes only both affected regions', async ({ page }) => {
	const api = await mockStaffDirectory(page);
	await page.goto(staffPath(firstStaff, '/roles'));
	page.on('dialog', (dialog) => dialog.accept());
	await page.getByRole('button', { name: 'เพิกถอน' }).click();
	await expect(page.getByTestId('user-role-assignments')).toContainText('ยังไม่มีบทบาทที่ได้รับ');
	expect(api.count(rolesEndpoint)).toBe(2);
	expect(api.count(permissionsEndpoint)).toBe(2);
	expect(api.count('/api/roles')).toBe(0);
});
test('late roles cannot replace the next user', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'assignments' });
	await page.goto(staffPath(firstStaff, '/roles'));
	await expect(page.getByTestId('user-role-assignments').getByRole('status')).toBeVisible();
	await navigate(page, staffPath(secondStaff, '/roles'));
	await expect(page.getByTestId('user-role-assignments')).toContainText('ยังไม่มีบทบาทที่ได้รับ');
	api.release();
	await expect(page.getByTestId('user-role-assignments')).not.toContainText('บทบาทตัวเลือก');
});
test('new staff form has no primary or unopened choices read', async ({ page }) => {
	const api = await mockStaffDirectory(page);
	await page.goto('/staff/manage/new');
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toBeVisible();
	expect(api.reads).toHaveLength(0);
});
test('create-only form keeps denied choices out of startup and their opened step', async ({
	page
}) => {
	const api = await mockStaffDirectory(page, { permissions: ['staff.create.all'] });
	await page.goto('/staff/manage/new');
	await fillCreate(page);
	await next(page);
	await expect(page.getByText('ไม่มีสิทธิ์อ่านตัวเลือกบทบาท', { exact: true })).toBeVisible();
	expect(api.reads).toHaveLength(0);
});
test('creation choices start only at their steps and have independent retry', async ({ page }) => {
	const api = await mockStaffDirectory(page, { fail: 'catalog' });
	await page.goto('/staff/manage/new');
	await fillCreate(page);
	await next(page);
	await expect(page.getByTestId('staff-create-form')).toContainText('region catalog ไม่พร้อม');
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await page
		.getByRole('button', { name: /บทบาทตัวเลือก/ })
		.first()
		.click();
	expect(api.count('/api/organization/units')).toBe(0);
	await next(page);
	await expect.poll(() => api.count('/api/organization/units')).toBe(1);
	expect(api.count('/api/roles')).toBe(2);
});
test('sensitive creation inputs never enter persistent drafts or reappear after reload', async ({
	page
}) => {
	await mockStaffDirectory(page);
	await page.goto('/staff/manage/new');
	await fillCreate(page);
	await page.getByPlaceholder('1234567890123 (ไม่บังคับ)').fill('0000000000000');
	await next(page);
	const stored = await page.evaluate(() =>
		Object.keys(localStorage)
			.filter((key) => key.startsWith('staff-create-draft'))
			.map((key) => JSON.parse(localStorage.getItem(key)!))
	);
	expect(stored).toHaveLength(1);
	expect(stored[0].fields).not.toHaveProperty('national_id');
	expect(stored[0].fields).not.toHaveProperty('password');
	expect(stored[0].fields).not.toHaveProperty('confirmPassword');
	await page.reload();
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรใหม่');
	await expect(page.getByPlaceholder('1234567890123 (ไม่บังคับ)')).toHaveValue('');
	await expect(page.locator('input[type=password]').nth(0)).toHaveValue('');
});
test('unsafe ownerless creation draft is removed without restoring its fields', async ({
	page
}) => {
	await page.addInitScript(() =>
		localStorage.setItem('staff-create-draft', '{"first_name":"unknown-owner"}')
	);
	await mockStaffDirectory(page);
	await page.goto('/staff/manage/new');
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('');
	expect(await page.evaluate(() => localStorage.getItem('staff-create-draft'))).toBeNull();
});
for (const mode of ['delayed', 'failed'] as const)
	test(`editor ${mode} primary state is independent of unopened options`, async ({ page }) => {
		const api = await mockStaffDirectory(
			page,
			mode === 'delayed' ? { hold: 'profile' } : { fail: 'profile' }
		);
		await page.goto(staffPath(firstStaff, '/edit'));
		const region = page.getByTestId('staff-edit-profile');
		if (mode === 'delayed') {
			await expect(region.getByRole('status')).toBeVisible();
			api.release();
		} else {
			await expect(region).toContainText('region profile ไม่พร้อม');
			await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		}
		await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรแรก');
		expect(api.count('/api/roles')).toBe(0);
		expect(api.count('/api/organization/units')).toBe(0);
	});
test('editor choices are opened by step and typed empty save navigates without shared refresh', async ({
	page
}) => {
	const api = await mockStaffDirectory(page);
	await page.goto(staffPath(firstStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรแรก');
	await page.getByPlaceholder('ชื่อ', { exact: true }).fill('บุคลากรแก้ไข');
	expect(api.count('/api/roles')).toBe(0);
	await next(page, 2);
	await expect.poll(() => api.count('/api/roles')).toBe(1);
	expect(api.count('/api/organization/units')).toBe(0);
	await next(page);
	await expect.poll(() => api.count('/api/organization/units')).toBe(1);
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect(page).toHaveURL(staffPath());
	await expect(page.getByTestId('staff-profile')).toContainText('บุคลากรแก้ไข');
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/menu/user')).toBe(1);
});
test('late editor read cannot initialize the next person draft', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'profile' });
	await page.goto(staffPath(firstStaff, '/edit'));
	await expect(page.getByTestId('staff-edit-profile').getByRole('status')).toBeVisible();
	await navigate(page, staffPath(secondStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรที่สอง');
	api.release();
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรที่สอง');
});
test('late editor save cannot navigate or reset a newer person', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'mutation' });
	await page.goto(staffPath(firstStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรแรก');
	await next(page, 3);
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, staffPath(secondStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรที่สอง');
	api.release();
	await expect(page).toHaveURL(staffPath(secondStaff, '/edit'));
});
test('same-person editor refresh retains the draft and reports a localized error', async ({
	page
}) => {
	await mockStaffDirectory(page, { fail: 'profile', failAt: 2 });
	await page.goto(staffPath(firstStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรแรก');
	await page.getByPlaceholder('ชื่อ', { exact: true }).fill('ร่างที่ยังไม่บันทึก');
	await page.getByRole('button', { name: 'รีเฟรชข้อมูล', exact: true }).click();
	await expect(page.getByTestId('staff-edit-profile')).toContainText('region profile ไม่พร้อม');
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('ร่างที่ยังไม่บันทึก');
});
test('achievement mutation patches only its history without profile or identity reread', async ({
	page
}) => {
	const api = await mockStaffDirectory(page);
	await page.goto(staffPath());
	await page.getByRole('button', { name: 'เพิ่มผลงาน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.locator('#title').fill('ผลงานใหม่');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(page.getByTestId('staff-achievements')).toContainText('ผลงานใหม่');
	expect(api.count('/api/achievements')).toBe(1);
	expect(api.count(profileEndpoint)).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
});
async function seedCreateDraft(page: Page) {
	await page.addInitScript(
		({ actor, roleId, organizationId }) =>
			localStorage.setItem(
				`staff-create-draft:v2:${location.origin}:${actor}`,
				JSON.stringify({
					version: 2,
					expiresAt: Date.now() + 30 * 60 * 1000,
					fields: {
						role_ids: [roleId],
						primary_role_id: roleId,
						organization_assignments: [
							{
								organization_unit_id: organizationId,
								position_code: 'member',
								is_primary: true,
								responsibilities: ''
							}
						]
					}
				})
			),
		{ actor, roleId, organizationId }
	);
}
test('creation UUID reply clears its draft and loads only the destination list', async ({
	page
}) => {
	await seedCreateDraft(page);
	const api = await mockStaffDirectory(page);
	await page.goto('/staff/manage/new');
	await fillCreate(page);
	await next(page, 2);
	await page.getByRole('button', { name: 'สร้างบุคลากร', exact: true }).click();
	await expect(page).toHaveURL(directoryPath);
	await expect(page.getByTestId('staff-directory')).toBeVisible();
	expect(api.writes).toEqual([{ method: 'POST', path: '/api/staff' }]);
	expect(api.count('/api/staff')).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/menu/user')).toBe(1);
	expect(
		await page.evaluate(() =>
			Object.keys(localStorage).some((key) => key.startsWith('staff-create-draft:v2:'))
		)
	).toBe(false);
});
test('late creation cannot navigate from a newer route', async ({ page }) => {
	await seedCreateDraft(page);
	const api = await mockStaffDirectory(page, { hold: 'mutation' });
	await page.goto('/staff/manage/new');
	await fillCreate(page);
	await next(page, 2);
	await page.getByRole('button', { name: 'สร้างบุคลากร', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, staffPath(secondStaff));
	await expect(page.getByTestId('staff-profile')).toContainText('บุคลากรที่สอง');
	const response = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === '/api/staff' && response.request().method() === 'POST'
	);
	api.release();
	await (await response).finished();
	await expect(page).toHaveURL(staffPath(secondStaff));
});
const pixel = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==',
	'base64'
);
test('closed achievement upload cannot submit or reset a newer person dialog', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'upload' });
	await page.goto(staffPath());
	await page.getByRole('button', { name: 'เพิ่มผลงาน', exact: true }).click();
	let dialog = page.getByRole('dialog');
	await dialog.locator('#title').fill('ผลงานเก่า');
	await dialog
		.locator('input[type=file]')
		.setInputFiles({ name: 'synthetic.png', mimeType: 'image/png', buffer: pixel });
	await expect(dialog.getByRole('img', { name: 'Preview' })).toBeVisible();
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => api.writes.filter((write) => write.path === '/api/files').length).toBe(1);
	await page.keyboard.press('Escape');
	await navigate(page, staffPath(secondStaff));
	await page.getByRole('button', { name: 'เพิ่มผลงาน', exact: true }).click();
	dialog = page.getByRole('dialog');
	await dialog.locator('#title').fill('ร่างคนใหม่');
	const response = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/files'
	);
	api.release();
	await (await response).finished();
	await expect(dialog.locator('#title')).toHaveValue('ร่างคนใหม่');
	expect(api.writes.filter((write) => write.path === '/api/achievements')).toHaveLength(0);
});
test('old avatar upload cannot attach to the next person', async ({ page }) => {
	const api = await mockStaffDirectory(page, { hold: 'upload' });
	await page.goto(staffPath(firstStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรแรก');
	await page
		.locator('input[type=file]')
		.setInputFiles({ name: 'synthetic.png', mimeType: 'image/png', buffer: pixel });
	await page.getByRole('button', { name: 'บันทึกรูปภาพ', exact: true }).click();
	await expect.poll(() => api.writes.filter((write) => write.path === '/api/files').length).toBe(1);
	await navigate(page, staffPath(secondStaff, '/edit'));
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรที่สอง');
	const response = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/files'
	);
	api.release();
	await (await response).finished();
	expect(api.writes.filter((write) => write.method === 'PUT')).toHaveLength(0);
	await expect(page.getByPlaceholder('ชื่อ', { exact: true })).toHaveValue('บุคลากรที่สอง');
});
