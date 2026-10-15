import { expect, test, type Page, type Route } from '@playwright/test';
import { mockOrganization, firstUnit, secondUnit } from './fixtures/organization-route-data';

test.use({ serviceWorkers: 'block' });
const areaId = '30000000-0000-4000-8000-000000000001';
const path = '/staff/organization?tab=subject-groups';
const group = {
	id: areaId,
	organizationUnitId: firstUnit,
	code: 'SG-000001',
	nameTh: 'คณิตศาสตร์',
	nameEn: '',
	displayOrder: 1,
	isActive: true,
	rowVersion: 1,
	createdAt: '',
	updatedAt: ''
};
async function fixture(
	page: Page,
	options: {
		permissions?: string[];
		failSave?: boolean;
		hold?: boolean;
		holdStructure?: boolean;
		holdSave?: boolean;
	} = {}
) {
	const organization = await mockOrganization(page, {
		permissions: options.permissions,
		hold: options.holdStructure ? 'structure' : undefined
	});
	let groups = [
		group,
		{
			...group,
			id: areaId.replace(/1$/, '2'),
			organizationUnitId: secondUnit,
			code: 'SG-000002',
			nameTh: 'วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาวสำหรับทดสอบการแสดงผล'
		}
	];
	const writes: object[] = [];
	let reads = 0,
		release = () => {};
	const pending = new Promise<void>((resolve) => (release = resolve));
	async function reply(route: Route, data: unknown, status = 200) {
		await route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	}
	await page.route('**/api/academic/catalog/subject-groups**', async (route) => {
		if (route.request().method() === 'GET') {
			reads++;
			if (options.hold) await pending;
			return reply(route, groups);
		}
		const body = route.request().postDataJSON();
		writes.push(body);
		if (options.failSave && writes.length === 1)
			return reply(route, 'บันทึกไม่สำเร็จ กรุณาลองอีกครั้ง', 409);
		if (options.holdSave) await pending;
		const updated = {
			...group,
			...body,
			id: areaId.replace(/1$/, '3'),
			code: 'SG-000003',
			rowVersion: 2
		};
		groups = [...groups, updated];
		await reply(route, updated);
	});
	return {
		...organization,
		writes,
		releaseStructure: organization.release,
		release: () => release(),
		reads: () => reads
	};
}

test('academic-only readers use the same registry without administrative reads or mutation controls', async ({
	page
}) => {
	const state = await fixture(page, { permissions: ['academic_catalog.read.school'] });
	await page.goto('/staff/academic/catalog/subject-groups');
	await expect(page).toHaveURL(/organization\?tab=subject-groups/);
	await expect(page.getByRole('heading', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ' })).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'กลุ่มบริหารและฝ่าย/งาน' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'สิทธิ์', exact: true })).toHaveCount(0);
	expect(state.count('/api/organization/units')).toBe(0);
	expect(state.count(`/api/organization/units/${firstUnit}/members`)).toBe(0);
	expect(state.reads()).toBe(1);
});

test('unit-scoped catalog management cannot open the school registry', async ({ page }) => {
	const state = await fixture(page, { permissions: ['academic_catalog.manage.organization_unit'] });
	await page.goto(path);
	await expect(page).toHaveURL(/\/403/);
	expect(state.reads()).toBe(0);
	expect(state.count('/api/organization/units')).toBe(0);
});

test('academic managers can edit the registry without administering memberships or grants', async ({
	page
}) => {
	await fixture(page, { permissions: ['academic_catalog.manage.school'] });
	await page.goto(path);
	await expect(page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'สิทธิ์', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'หัวหน้าและสมาชิก' }).click();
	await expect(page.getByText('สมาชิกแรก', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มสมาชิก', exact: true })).toHaveCount(0);
});

test('creation omits codes, keeps a failed draft, and patches the collection once', async ({
	page
}) => {
	const state = await fixture(page, { failSave: true });
	await page.goto(path);
	await page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByLabel('รหัส', { exact: true })).toHaveCount(0);
	await dialog.getByLabel('ชื่อกลุ่มสาระ').fill('ภาษาจีน');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('บันทึกไม่สำเร็จ');
	await expect(dialog.getByLabel('ชื่อกลุ่มสาระ')).toHaveValue('ภาษาจีน');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(page.getByRole('heading', { name: 'ภาษาจีน', exact: true })).toBeVisible();
	expect(state.writes).toEqual([
		{ nameTh: 'ภาษาจีน', nameEn: '' },
		{ nameTh: 'ภาษาจีน', nameEn: '' }
	]);
	expect(state.reads()).toBe(1);
	await expect(page).toHaveURL(/group=30000000-0000-4000-8000-000000000003/);
});

test('member details are lazy and selection survives reload and history', async ({ page }) => {
	const state = await fixture(page);
	await page.goto(path);
	await expect(page.getByRole('heading', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
	expect(state.count(`/api/organization/units/${firstUnit}/members`)).toBe(0);
	await page.getByRole('button', { name: 'หัวหน้าและสมาชิก' }).click();
	await expect(page.getByText('สมาชิกแรก', { exact: true })).toBeVisible();
	expect(state.count(`/api/organization/units/${firstUnit}/members`)).toBe(1);
	await page.reload();
	await expect(page.getByText('สมาชิกแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).click();
	await expect(page).toHaveURL(/group=.*2&section=info/);
	await page.goBack();
	await expect(page.getByText('สมาชิกแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).click();
	await page.getByRole('link', { name: 'กลุ่มบริหารและฝ่าย/งาน', exact: true }).click();
	await expect(page.getByTestId('organization-catalog')).toBeVisible();
	await page.getByRole('link', { name: 'กลุ่มสาระการเรียนรู้', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ })
	).toBeVisible();
});

for (const theme of ['light', 'dark'] as const)
	for (const viewport of [
		{ width: 390, height: 844 },
		{ width: 1440, height: 1000 }
	]) {
		test(`learning-area layout ${theme} ${viewport.width}`, async ({ page }, testInfo) => {
			await page.setViewportSize(viewport);
			await page.emulateMedia({ colorScheme: theme });
			const state = await fixture(page, { hold: true });
			await page.goto(path, { waitUntil: 'domcontentloaded' });
			await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
			state.release();
			await expect(page.getByTestId('catalog-subject-groups-ready')).toBeVisible();
			await expect(page.getByRole('heading', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: testInfo.outputPath('learning-areas.png'),
				fullPage: true,
				animations: 'disabled'
			});
		});
	}

test.describe('mobile touch and keyboard', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('group selection and edit dialog support touch, keyboard and focus return', async ({
		page
	}) => {
		await fixture(page);
		await page.goto(path);
		await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).tap();
		await expect(
			page.getByRole('heading', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ })
		).toBeVisible();
		await page.getByRole('button', { name: 'หัวหน้าและสมาชิก', exact: true }).tap();
		await expect(page.getByText('สมาชิกสอง', { exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'ข้อมูล', exact: true }).tap();
		const edit = page.getByRole('button', { name: 'แก้ไขข้อมูล', exact: true });
		await edit.focus();
		await page.keyboard.press('Enter');
		await expect(page.getByRole('dialog')).toBeVisible();
		await page.getByLabel('ชื่อกลุ่มสาระ').focus();
		await page.keyboard.press('Escape');
		await expect(page.getByRole('dialog')).not.toBeVisible();
		await expect(edit).toBeFocused();
	});
});

test('first administrative visit after a learning-area entry shows its initial skeleton', async ({
	page
}) => {
	const state = await fixture(page, { holdStructure: true });
	await page.goto(path);
	await expect(page.getByTestId('catalog-subject-groups-ready')).toBeVisible();
	expect(state.count('/api/organization/units')).toBe(0);
	await page.getByRole('link', { name: 'กลุ่มบริหารและฝ่าย/งาน', exact: true }).click();
	await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
	state.releaseStructure();
	await expect(page.getByTestId('organization-selected-members')).toContainText('สมาชิกแรก');
});

test('a pending learning-area save blocks duplicate submission', async ({ page }) => {
	const state = await fixture(page, { holdSave: true });
	await page.goto(path);
	await page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ', exact: true }).click();
	await page.getByLabel('ชื่อกลุ่มสาระ').fill('ภาษาจีน');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(1);
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'กำลังดำเนินการ...' })
	).toBeDisabled();
	state.release();
	await expect(page.getByRole('dialog')).not.toBeVisible();
	expect(state.writes).toHaveLength(1);
});
