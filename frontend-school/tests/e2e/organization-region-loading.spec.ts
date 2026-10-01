import { expect, test } from '@playwright/test';
import {
	mockOrganization,
	firstUnit,
	secondUnit,
	unitPath
} from './fixtures/organization-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const endpoint = (kind: string, unit = firstUnit) =>
	kind === 'structure'
		? '/api/organization/units'
		: `/api/organization/units/${unit}${kind === 'unit' ? '' : `/${kind}`}`;
test('overview loader reads initial members once after resolving the selected unit', async ({
	page
}) => {
	const api = await mockOrganization(page, { hold: 'members' });
	await page.goto('/staff/organization');
	await expect(page.getByTestId('organization-catalog')).toContainText('หน่วยงานแรก');
	await expect(page.getByTestId('organization-selected-members').getByRole('status')).toBeVisible();
	expect(api.count(endpoint('structure'))).toBe(1);
	expect(api.count(endpoint('members'))).toBe(1);
	api.release();
	await expect(page.getByTestId('organization-selected-members')).toContainText('สมาชิกแรก');
});
test('overview selected members have focused retry and suppress a late prior selection', async ({
	page
}) => {
	const api = await mockOrganization(page, { hold: 'members' });
	await page.goto('/staff/organization');
	await page.getByText('หน่วยงานสอง', { exact: true }).first().click();
	await expect(page.getByTestId('organization-selected-members')).toContainText('สมาชิกสอง');
	api.release();
	await expect(page.getByTestId('organization-selected-members')).not.toContainText('สมาชิกแรก');
	expect(api.count(endpoint('structure'))).toBe(1);
});
test('overview retains catalog during failed refresh and retries only it', async ({ page }) => {
	const api = await mockOrganization(page, { fail: 'structure', failAt: 2 });
	await page.goto('/staff/organization');
	await expect(page.getByTestId('organization-selected-members')).toContainText('สมาชิกแรก');
	await page.getByRole('button', { name: 'รีเฟรชโครงสร้าง', exact: true }).click();
	await expect(page.getByTestId('organization-catalog')).toContainText('region structure ไม่พร้อม');
	await expect(page.getByTestId('organization-catalog')).toContainText('หน่วยงานแรก');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	expect(api.count(endpoint('members'))).toBe(1);
});
for (const kind of ['unit', 'structure', 'members'] as const) {
	const region = () =>
		kind === 'unit'
			? 'organization-unit'
			: kind === 'structure'
				? 'organization-structure'
				: 'organization-members';
	test(`selected ${kind} can be slow while independent siblings render`, async ({ page }) => {
		const api = await mockOrganization(page, { hold: kind });
		await page.goto(unitPath());
		await expect(page.getByTestId(region()).getByRole('status').first()).toBeVisible();
		await expect(
			page.getByTestId(kind === 'members' ? 'organization-unit' : 'organization-members')
		).toContainText(kind === 'members' ? 'หน่วยงานแรก' : 'สมาชิกแรก');
		api.release();
		await expect(page.getByTestId(region())).toContainText(
			kind === 'unit' ? 'หน่วยงานแรก' : kind === 'structure' ? 'หน่วยงานสอง' : 'สมาชิกแรก'
		);
		for (const read of ['unit', 'structure', 'members']) expect(api.count(endpoint(read))).toBe(1);
		expect(api.count(endpoint('delegations'))).toBe(0);
		expect(api.count(endpoint('delegatable-permissions'))).toBe(0);
	});
	test(`selected ${kind} fails and retries without rereading siblings`, async ({ page }) => {
		const api = await mockOrganization(page, { fail: kind });
		await page.goto(unitPath());
		const selected = page.getByTestId(region());
		await expect(selected).toContainText(`region ${kind} ไม่พร้อม`);
		await selected.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(selected).not.toContainText(`region ${kind} ไม่พร้อม`);
		for (const read of ['unit', 'structure', 'members'])
			expect(api.count(endpoint(read))).toBe(read === kind ? 2 : 1);
	});
}
test('late unit and members cannot replace the next route', async ({ page }) => {
	const api = await mockOrganization(page, { hold: 'unit' });
	await page.goto(unitPath());
	await navigate(page, unitPath(secondUnit));
	await expect(page.getByTestId('organization-unit')).toContainText('หน่วยงานสอง');
	await expect(page.getByTestId('organization-members')).toContainText('สมาชิกสอง');
	api.release();
	await expect(page.getByTestId('organization-unit')).not.toContainText('หน่วยงานแรก');
});
test('reader has no unopened action reads or delegation tab', async ({ page }) => {
	const api = await mockOrganization(page, { permissions: ['roles.read.all'] });
	await page.goto(unitPath());
	await expect(page.getByTestId('organization-members')).toContainText('สมาชิกแรก');
	expect(api.reads).toHaveLength(3);
	await expect(page.getByRole('button', { name: 'มอบหมายสิทธิ์', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เพิ่มสมาชิก', exact: true })).toHaveCount(0);
});
test('delegation history is tab-lazy and choices are dialog-lazy', async ({ page }) => {
	const api = await mockOrganization(page);
	await page.goto(unitPath());
	await expect(page.getByTestId('organization-unit')).toContainText('หน่วยงานแรก');
	expect(api.count(endpoint('delegations'))).toBe(0);
	await page.getByRole('button', { name: 'มอบหมายสิทธิ์', exact: true }).click();
	await expect(page.getByText('ยังไม่มีการมอบหมายสิทธิ์', { exact: true })).toBeVisible();
	expect(api.count(endpoint('delegations'))).toBe(1);
	expect(api.count(endpoint('delegatable-permissions'))).toBe(0);
	await page.getByRole('button', { name: 'มอบหมาย', exact: true }).click();
	await expect.poll(() => api.count(endpoint('delegatable-permissions'))).toBe(1);
	expect(api.count(endpoint('members'))).toBe(1);
});
for (const kind of ['catalog', 'grants'] as const)
	test(`permission dialog ${kind} retries independently`, async ({ page }) => {
		const api = await mockOrganization(page, { fail: kind });
		await page.goto(unitPath());
		await page
			.getByTestId('organization-unit')
			.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
			.click();
		const region = page.getByTestId(
			kind === 'catalog' ? 'organization-permission-catalog' : 'organization-grants'
		);
		await expect(region).toContainText(`region ${kind} ไม่พร้อม`);
		await region.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(
			page.getByRole('checkbox', { name: 'staff_profile.read.own หัวหน้า', exact: true })
		).toBeChecked();
		expect(api.count('/api/permissions/modules')).toBe(kind === 'catalog' ? 2 : 1);
		expect(api.count(endpoint('permissions'))).toBe(kind === 'grants' ? 2 : 1);
	});
test('missing settings read never requests permission labels or enables saving', async ({
	page
}) => {
	const api = await mockOrganization(page, { permissions: ['roles.read.all', 'roles.update.all'] });
	await page.goto(unitPath());
	await page
		.getByTestId('organization-unit')
		.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
		.click();
	await expect(page.getByTestId('organization-grants')).toContainText(
		'สิทธิ์ที่บันทึกไว้ 1 รายการ'
	);
	await expect(page.getByTestId('organization-permission-catalog')).toContainText(
		'ไม่มีสิทธิ์อ่านรายการสิทธิ์'
	);
	expect(api.count('/api/permissions/modules')).toBe(0);
	await expect(page.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true })).toBeDisabled();
});
test('membership picker stays lazy and uses minimal lookup under membership permission', async ({
	page
}) => {
	const api = await mockOrganization(page, { permissions: ['roles.read.all', 'roles.assign.all'] });
	await page.goto(unitPath());
	expect(api.count('/api/lookup/staff')).toBe(0);
	await page.getByRole('button', { name: 'เพิ่มสมาชิก', exact: true }).click();
	await page.getByRole('dialog').getByRole('combobox').first().click();
	await expect(page.getByRole('option', { name: /ตัวเลือก บุคลากร/ })).toBeVisible();
	expect(api.count('/api/lookup/staff')).toBe(1);
	expect(api.count('/api/staff')).toBe(0);
});
test('membership removal rereads only the shared member region once', async ({ page }) => {
	const api = await mockOrganization(page);
	await page.goto(unitPath());
	await page.getByRole('button', { name: 'ลบสมาชิก สมาชิกแรก', exact: true }).click();
	await expect(page.getByTestId('organization-members')).not.toContainText('สมาชิกแรก');
	expect(api.count(endpoint('members'))).toBe(2);
	expect(api.count(endpoint('unit'))).toBe(1);
	expect(api.count(endpoint('structure'))).toBe(1);
});
test('membership picker retries without rereading route regions', async ({ page }) => {
	const api = await mockOrganization(page, { fail: 'staff' });
	await page.goto(unitPath());
	await page.getByRole('button', { name: 'เพิ่มสมาชิก', exact: true }).click();
	await page.getByRole('dialog').getByRole('combobox').first().click();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await page.getByRole('option', { name: /ตัวเลือก บุคลากร/ }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'เพิ่ม', exact: true }).click();
	await expect(page.getByRole('dialog')).not.toBeVisible();
	await expect(page.getByTestId('organization-members')).toContainText('สมาชิกใหม่');
	expect(api.count('/api/lookup/staff')).toBe(2);
	expect(api.count(endpoint('members'))).toBe(2);
	expect(api.count(endpoint('unit'))).toBe(1);
	expect(api.count(endpoint('structure'))).toBe(1);
});
for (const kind of ['catalog', 'grants'] as const)
	test(`permission ${kind} has its own pending state`, async ({ page }) => {
		const api = await mockOrganization(page, { hold: kind });
		await page.goto(unitPath());
		await page
			.getByTestId('organization-unit')
			.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
			.click();
		await expect(
			page
				.getByTestId(kind === 'catalog' ? 'organization-permission-catalog' : 'organization-grants')
				.getByRole('status')
		).toBeVisible();
		if (kind === 'catalog')
			await expect(page.getByTestId('organization-grants')).toContainText(
				'สิทธิ์ที่บันทึกไว้ 1 รายการ'
			);
		expect(api.count('/api/permissions/modules')).toBe(1);
		expect(api.count(endpoint('permissions'))).toBe(1);
		api.release();
		await expect(
			page.getByRole('checkbox', { name: 'staff_profile.read.own หัวหน้า', exact: true })
		).toBeChecked();
	});
test('unit empty mutation refreshes only unit and structure', async ({ page }) => {
	const api = await mockOrganization(page);
	await page.goto(unitPath());
	await page.getByRole('button', { name: 'แก้ไขหน่วยงาน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.locator('#name').fill('ชื่อหน่วยงานใหม่');
	await dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(page.getByTestId('organization-unit')).toContainText('ชื่อหน่วยงานใหม่');
	expect(api.count(endpoint('unit'))).toBe(2);
	expect(api.count(endpoint('structure'))).toBe(2);
	expect(api.count(endpoint('members'))).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
});
test('permission mutation refreshes no unrelated primary regions', async ({ page }) => {
	const api = await mockOrganization(page);
	await page.goto(unitPath());
	await page
		.getByTestId('organization-unit')
		.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
		.click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByRole('checkbox', { name: 'staff_profile.read.own หัวหน้า', exact: true })
	).toBeChecked();
	await dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	for (const kind of ['unit', 'structure', 'members']) expect(api.count(endpoint(kind))).toBe(1);
	expect(api.writes).toEqual([{ method: 'PUT', path: endpoint('permissions') }]);
});
test('closed unit save cannot toast or refresh a newer owner', async ({ page }) => {
	const api = await mockOrganization(page, { hold: 'mutation' });
	await page.goto(unitPath());
	await page.getByRole('button', { name: 'แก้ไขหน่วยงาน', exact: true }).click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })
		.click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await navigate(page, unitPath(secondUnit));
	await expect(page.getByTestId('organization-unit')).toContainText('หน่วยงานสอง');
	const response = page.waitForResponse((response) => response.request().method() === 'PUT');
	api.release();
	await (await response).finished();
	expect(api.count(endpoint('unit', secondUnit))).toBe(1);
	await expect(page.getByTestId('organization-unit')).toContainText('หน่วยงานสอง');
});

test('ordinary detail links preload their destination and preserve one primary read per region', async ({
	page
}) => {
	const api = await mockOrganization(page);
	await page.goto('/staff/organization');
	await expect(page.getByTestId('organization-selected-members')).toContainText('สมาชิกแรก');
	await page.getByRole('link', { name: 'เปิดรายละเอียด', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`${firstUnit}$`));
	await expect(page.getByTestId('organization-unit')).toContainText('หน่วยงานแรก');
	await page
		.getByRole('link', { name: /หน่วยงานสอง/ })
		.first()
		.click();
	await expect(page).toHaveURL(new RegExp(`${secondUnit}$`));
	await expect(page.getByTestId('organization-members')).toContainText('สมาชิกสอง');
	expect(api.count(endpoint('unit', firstUnit))).toBe(1);
	expect(api.count(endpoint('unit', secondUnit))).toBe(1);
	expect(api.count(endpoint('delegations'))).toBe(0);
});
