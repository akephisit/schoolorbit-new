import { expect, test } from '@playwright/test';
import { mockStaffRoles, rolePath, firstRole, secondRole } from './fixtures/staff-role-route-data';
import { navigate } from './fixtures/supervision-route-data';
import { homePath, publicPath } from './fixtures/staff-home-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
test('list starts once in the route and has no editor catalog request', async ({ page }) => {
	const api = await mockStaffRoles(page, { hold: 'list' });
	await page.goto('/staff/roles');
	await expect(page.getByTestId('roles-list').getByRole('status')).toBeVisible();
	api.release();
	await expect(page.getByText('บทบาทแรก', { exact: true })).toBeVisible();
	expect(api.count('/api/roles')).toBe(1);
	expect(api.count('/api/permissions/modules')).toBe(0);
	expect(api.count('/api/auth/me')).toBe(1);
});
test('list has a focused first-read retry', async ({ page }) => {
	const api = await mockStaffRoles(page, { fail: 'list' });
	await page.goto('/staff/roles');
	const region = page.getByTestId('roles-list');
	await expect(region).toContainText('region บทบาทไม่พร้อม');
	await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('บทบาทแรก', { exact: true })).toBeVisible();
	expect(api.count('/api/roles')).toBe(2);
	expect(api.count('/api/permissions/modules')).toBe(0);
});
test('list retains its rows through a refresh error', async ({ page }) => {
	await mockStaffRoles(page, { fail: 'list', failAt: 2 });
	await page.goto('/staff/roles');
	await expect(page.getByText('บทบาทแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('roles-list')).toContainText('region บทบาทไม่พร้อม');
	await expect(page.getByText('บทบาทแรก', { exact: true })).toBeVisible();
});
for (const hold of ['role', 'catalog'] as const) {
	test(`selected role and visible catalog render independently of ${hold}`, async ({ page }) => {
		const api = await mockStaffRoles(page, { hold });
		await page.goto(rolePath());
		await expect(
			page.getByTestId(hold === 'role' ? 'role-detail' : 'role-permissions').getByRole('status')
		).toBeVisible();
		if (hold === 'role') {
			await expect(page.getByText('อ่านโปรไฟล์ตนเอง', { exact: true })).toBeVisible();
			await expect(
				page.getByRole('button', { name: 'บันทึก', exact: true }).first()
			).toBeDisabled();
		} else await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
		api.release();
		await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
		expect(api.count(`/api/roles/${firstRole}`)).toBe(1);
		expect(api.count('/api/permissions/modules')).toBe(1);
	});
}
for (const fail of ['role', 'catalog'] as const) {
	test(`${fail} retries without rereading the ready sibling`, async ({ page }) => {
		const api = await mockStaffRoles(page, { fail });
		await page.goto(rolePath());
		const region = page.getByTestId(fail === 'role' ? 'role-detail' : 'role-permissions');
		await expect(region).toContainText('region บทบาทไม่พร้อม');
		await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
		await expect(page.getByText('อ่านโปรไฟล์ตนเอง', { exact: true })).toBeVisible();
		expect(api.count(`/api/roles/${firstRole}`)).toBe(fail === 'role' ? 2 : 1);
		expect(api.count('/api/permissions/modules')).toBe(fail === 'catalog' ? 2 : 1);
	});
}
test('read-only role does not request a denied permission catalog', async ({ page }) => {
	const api = await mockStaffRoles(page, { permissions: ['roles.read.all'] });
	await page.goto(rolePath());
	await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
	await expect(page.locator('#name')).toBeDisabled();
	await expect(
		page.getByText('ไม่มีสิทธิ์ดูรายการ permission catalog', { exact: true })
	).toBeVisible();
	expect(api.count('/api/permissions/modules')).toBe(0);
});
test('create-only new role needs no selected-role read', async ({ page }) => {
	const api = await mockStaffRoles(page, {
		permissions: ['roles.create.all', 'settings.read.all']
	});
	await page.goto(rolePath('new'));
	await expect(page.locator('#name')).toHaveValue('');
	await expect(page.getByText('อ่านโปรไฟล์ตนเอง', { exact: true })).toBeVisible();
	expect(api.count('/api/roles/new')).toBe(0);
	expect(api.count('/api/permissions/modules')).toBe(1);
});
test('create-only user cannot preload an existing role or catalog', async ({ page }) => {
	const api = await mockStaffRoles(page, {
		permissions: ['roles.create.all', 'settings.read.all']
	});
	await page.goto(rolePath());
	await expect(page.getByText('ไม่มีสิทธิ์ดูบทบาท', { exact: true })).toBeVisible();
	expect(api.count(`/api/roles/${firstRole}`)).toBe(0);
	expect(api.count('/api/permissions/modules')).toBe(0);
});
test('late prior role cannot replace the next selected identity', async ({ page }) => {
	const api = await mockStaffRoles(page, { hold: 'role' });
	await page.goto(rolePath());
	await expect(page.getByTestId('role-detail').getByRole('status')).toBeVisible();
	await navigate(page, rolePath(secondRole));
	await expect(page.locator('#name')).toHaveValue('บทบาทที่สอง');
	api.release();
	await expect(page.locator('#name')).toHaveValue('บทบาทที่สอง');
});
test('catalog retry preserves a role form draft', async ({ page }) => {
	const api = await mockStaffRoles(page, { fail: 'catalog' });
	await page.goto(rolePath());
	await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
	await page.locator('#name').fill('ร่างที่คงอยู่');
	await page.getByTestId('role-permissions').getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('อ่านโปรไฟล์ตนเอง', { exact: true })).toBeVisible();
	await expect(page.locator('#name')).toHaveValue('ร่างที่คงอยู่');
	expect(api.count(`/api/roles/${firstRole}`)).toBe(1);
});
test('save refreshes only the destination list, keeping shared app reads', async ({ page }) => {
	const api = await mockStaffRoles(page);
	await page.goto(rolePath());
	await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
	await page.locator('#name').fill('ชื่อที่บันทึก');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	await expect(page).toHaveURL(/\/staff\/roles$/);
	await expect(page.getByText('ชื่อที่บันทึก', { exact: true })).toBeVisible();
	expect(api.count(`/api/roles/${firstRole}`)).toBe(1);
	expect(api.count('/api/roles')).toBe(1);
	expect(api.count('/api/menu/user')).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
});
test('late save cannot redirect or toast over the next role', async ({ page }) => {
	const api = await mockStaffRoles(page, { hold: 'mutation' });
	await page.goto(rolePath());
	await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
	await page.locator('#name').fill('ร่างเก่า');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, rolePath(secondRole));
	await expect(page.locator('#name')).toHaveValue('บทบาทที่สอง');
	const settled = page.waitForResponse(
		(response) =>
			response.url().includes(`/api/roles/${firstRole}`) && response.request().method() === 'PUT'
	);
	api.release();
	await settled;
	await expect(page.locator('#name')).toHaveValue('บทบาทที่สอง');
	await expect(page.getByText('บันทึกข้อมูลสำเร็จ', { exact: true })).toHaveCount(0);
	await expect(page).toHaveURL(new RegExp(secondRole));
});
test('late deactivate cannot close or redirect a newer route', async ({ page }) => {
	const api = await mockStaffRoles(page, { hold: 'mutation' });
	await page.goto(rolePath());
	await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
	await page.getByRole('button', { name: 'ปิดใช้งาน', exact: true }).click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'ปิดใช้งานบทบาท', exact: true })
		.click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await navigate(page, homePath());
	await expect(page.getByRole('heading', { name: 'หน้าหลักของฉัน', exact: true })).toBeVisible();
	const settled = page.waitForResponse(
		(response) =>
			response.url().includes(`/api/roles/${firstRole}`) && response.request().method() === 'DELETE'
	);
	api.release();
	await settled;
	await expect(page.getByText('ปิดใช้งานบทบาทสำเร็จ', { exact: true })).toHaveCount(0);
});

test('a realtime read grant loads the formerly denied selected role', async ({ page }) => {
	const api = await mockStaffRoles(page, {
		permissions: ['roles.create.all', 'settings.read.all'],
		signalPermissionChange: true
	});
	await page.goto(rolePath());
	await expect(page.getByText('ไม่มีสิทธิ์ดูบทบาท', { exact: true })).toBeVisible();
	api.signalPermissions(['roles.read.all', 'settings.read.all']);
	await expect.poll(() => api.count('/api/auth/me')).toBe(2);
	await expect(page.locator('#name')).toHaveValue('บทบาทแรก');
	expect(api.count(`/api/roles/${firstRole}`)).toBe(1);
});
test('a revoked catalog permission hides its labels immediately', async ({ page }) => {
	const api = await mockStaffRoles(page, { signalPermissionChange: true });
	await page.goto(rolePath());
	await expect(page.getByText('อ่านโปรไฟล์ตนเอง', { exact: true })).toBeVisible();
	api.signalPermissions(['roles.read.all']);
	await expect(
		page.getByText('ไม่มีสิทธิ์ดูรายการ permission catalog', { exact: true })
	).toBeVisible();
	await expect(page.getByText('อ่านโปรไฟล์ตนเอง', { exact: true })).toHaveCount(0);
});
test('public achievements disappear when realtime access is revoked', async ({ page }) => {
	const api = await mockStaffRoles(page, {
		permissions: ['roles.read.all', 'achievement.read.all'],
		signalPermissionChange: true
	});
	await page.goto(publicPath());
	await expect(page.getByTestId('staff-public-achievements')).toContainText('ผลงานที่พร้อมก่อน');
	api.signalPermissions(['roles.read.all']);
	await expect.poll(() => api.count('/api/auth/me')).toBe(2);
	await expect(page.getByText('ไม่มีสิทธิ์ดูผลงานของบุคลากรนี้')).toBeVisible();
	await expect(page.getByTestId('staff-public-achievements')).not.toContainText(
		'ผลงานที่พร้อมก่อน'
	);
});

test('new role uses its UUID mutation result and refreshes only the destination list', async ({
	page
}) => {
	const api = await mockStaffRoles(page);
	await page.goto(rolePath('new'));
	await page.locator('#name').fill('บทบาทที่สร้าง');
	await page.locator('#code').fill('CREATED');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).first().click();
	await expect(page.getByText('บทบาทที่สร้าง', { exact: true })).toBeVisible();
	expect(api.count('/api/roles/new')).toBe(0);
	expect(api.count('/api/roles')).toBe(1);
	expect(api.count('/api/permissions/modules')).toBe(1);
	expect(api.writes[0].method).toBe('POST');
});
test('reactivation refreshes the destination list without rereading selected role or catalog', async ({
	page
}) => {
	const api = await mockStaffRoles(page, { inactive: true });
	await page.goto(rolePath());
	await page.getByRole('button', { name: 'เปิดใช้งาน', exact: true }).click();
	await expect(page.getByText('บทบาทแรก', { exact: true })).toBeVisible();
	expect(api.count(`/api/roles/${firstRole}`)).toBe(1);
	expect(api.count('/api/permissions/modules')).toBe(1);
	expect(api.count('/api/roles')).toBe(1);
	expect(api.writes[0].payload).toEqual({ is_active: true });
});
