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
		holdMembers?: boolean;
		inactive?: boolean;
		empty?: boolean;
		activity?: boolean;
		failReadAt?: number;
	} = {}
) {
	const organization = await mockOrganization(page, {
		permissions: options.permissions,
		hold: options.holdStructure ? 'structure' : options.holdMembers ? 'members' : undefined
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
	if (options.inactive)
		groups.push({
			...group,
			id: areaId.replace(/1$/, '4'),
			code: 'SG-000004',
			nameTh: 'ภาษาฝรั่งเศส',
			nameEn: 'French',
			isActive: false
		});
	if (options.activity)
		groups.push({
			...group,
			id: areaId.replace(/1$/, '5'),
			code: 'AC',
			nameTh: 'กิจกรรมพัฒนาผู้เรียน'
		});
	if (options.empty) groups = [];
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
			if (reads === options.failReadAt)
				return reply(route, 'ทะเบียนไม่พร้อม กรุณาลองอีกครั้ง', 503);
			return reply(route, groups);
		}
		const body = route.request().postDataJSON();
		writes.push(body);
		if (options.failSave && writes.length === 1)
			return reply(route, 'บันทึกไม่สำเร็จ กรุณาลองอีกครั้ง', 409);
		if (options.holdSave) await pending;
		const targetId = new URL(route.request().url()).pathname.split('/').at(-1);
		const existing = groups.find((item) => item.id === targetId);
		const updated = {
			...(existing ?? group),
			...body,
			id: existing?.id ?? areaId.replace(/1$/, '3'),
			code: existing?.code ?? 'SG-000003',
			rowVersion: 2
		};
		groups = existing
			? groups.map((item) => (item.id === existing.id ? updated : item))
			: [...groups, updated];
		await reply(route, updated);
	});
	return {
		...organization,
		writes,
		releaseStructure: organization.release,
		releaseMembers: organization.release,
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
			if (viewport.width < 1024)
				await page.getByRole('button', { name: 'คณิตศาสตร์', exact: true }).click();
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

test('group search and status survive reload, structure tabs and a unit detail return', async ({
	page
}) => {
	const state = await fixture(page, { inactive: true });
	await page.goto(path);
	const list = page.getByTestId('learning-area-list');
	await list.getByLabel('ค้นหากลุ่มสาระ').fill('French');
	await list.getByRole('radio', { name: 'ปิดใช้งาน', exact: true }).click();
	await list.getByRole('radio', { name: 'ปิดใช้งาน', exact: true }).click();
	await expect(list.getByRole('radio', { name: 'ปิดใช้งาน', exact: true })).toBeChecked();
	await expect(list.getByRole('button', { name: /ภาษาฝรั่งเศส/ })).toBeVisible();
	await expect(list.getByRole('button', { name: 'คณิตศาสตร์', exact: true })).toHaveCount(0);
	await expect(page).toHaveURL(/group_q=French/);
	await expect(page).toHaveURL(/group_status=inactive/);
	expect(state.reads()).toBe(1);
	await page.reload();
	await expect(list.getByLabel('ค้นหากลุ่มสาระ')).toHaveValue('French');
	await expect(list.getByRole('radio', { name: 'ปิดใช้งาน', exact: true })).toBeChecked();
	await page.getByRole('link', { name: 'กลุ่มบริหารและฝ่าย/งาน', exact: true }).click();
	await expect(page.getByTestId('organization-catalog')).toBeVisible();
	await page.getByRole('link', { name: 'เปิดรายละเอียด', exact: true }).click();
	await expect(page.getByTestId('organization-unit')).toBeVisible();
	await page.getByRole('link', { name: 'โครงสร้าง', exact: true }).click();
	await expect(page.getByTestId('organization-catalog')).toBeVisible();
	await page.getByRole('link', { name: 'กลุ่มสาระการเรียนรู้', exact: true }).click();
	await expect(list.getByLabel('ค้นหากลุ่มสาระ')).toHaveValue('French');
	await expect(list.getByRole('radio', { name: 'ปิดใช้งาน', exact: true })).toBeChecked();
	expect(state.writes).toHaveLength(0);
});

test('initial loading blocks creation and failed registry reads have a focused retry', async ({
	page
}) => {
	const state = await fixture(page, { hold: true, failReadAt: 1 });
	await page.goto(path);
	await expect(page.getByRole('status', { name: 'กำลังโหลดกลุ่มสาระ', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มกลุ่มสาระ', exact: true })).toBeDisabled();
	state.release();
	await expect(page.getByText('ทะเบียนไม่พร้อม กรุณาลองอีกครั้ง')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
	expect(state.reads()).toBe(2);
	expect(state.count('/api/organization/units')).toBe(0);
});

test('failed background registry refresh retains selection and results with no member reads', async ({
	page
}) => {
	const state = await fixture(page, { failReadAt: 2 });
	await page.goto(`${path}&group=${areaId}&section=info`);
	await expect(page.getByRole('heading', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรชกลุ่มสาระ', exact: true }).click();
	await expect(page.getByText('อัปเดตกลุ่มสาระไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(page.getByTestId('learning-area-detail')).toContainText('คณิตศาสตร์');
	await expect(page.getByTestId('learning-area-list')).toContainText('วิทยาศาสตร์');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByText('อัปเดตกลุ่มสาระไม่สำเร็จ', { exact: true })).toHaveCount(0);
	expect(state.reads()).toBe(3);
	expect(state.count(`/api/organization/units/${firstUnit}/members`)).toBe(0);
});

test('empty registry, empty search and a stale selected identity remain distinct', async ({
	page
}) => {
	await fixture(page, { empty: true });
	await page.goto(path);
	await expect(page.getByText('ยังไม่มีกลุ่มสาระ', { exact: true })).toBeVisible();
	await expect(page.getByText('ไม่พบกลุ่มสาระที่ตรงเงื่อนไข', { exact: true })).toHaveCount(0);
	await fixture(page);
	await page.reload();
	await page.getByLabel('ค้นหากลุ่มสาระ').fill('ไม่ตรงรายการ');
	await expect(page.getByText('ไม่พบกลุ่มสาระที่ตรงเงื่อนไข', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).click();
	await expect(page.getByRole('button', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
	await page.goto(`${path}&group=missing&section=members`);
	await expect(page.getByText('ไม่พบกลุ่มสาระที่เลือก', { exact: true })).toBeVisible();
	await expect(page.getByTestId('organization-members')).toHaveCount(0);
});

test('permission matrix remains action-lazy and academic-only permission deep links are denied', async ({
	page
}) => {
	const state = await fixture(page);
	await page.goto(`${path}&group=${areaId}&section=permissions`);
	await expect(page.getByRole('heading', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })).toBeVisible();
	expect(state.count(`/api/organization/units/${firstUnit}/permissions`)).toBe(0);
	expect(state.count('/api/permissions/modules')).toBe(0);
	await page.getByRole('button', { name: 'กำหนดสิทธิ์ตามตำแหน่ง', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('ดูโปรไฟล์ตนเอง');
	expect(state.count(`/api/organization/units/${firstUnit}/permissions`)).toBe(1);
	await page.keyboard.press('Escape');
	const reader = await fixture(page, { permissions: ['academic_catalog.read.school'] });
	await page.reload();
	await expect(page.getByText('ไม่มีสิทธิ์ดูการกำหนดสิทธิ์', { exact: true })).toBeVisible();
	expect(reader.count(`/api/organization/units/${firstUnit}/permissions`)).toBe(0);
	expect(reader.count('/api/permissions/modules')).toBe(0);
});

test('pending members show unknown counts and a late previous group cannot replace the selected group', async ({
	page
}) => {
	const state = await fixture(page, { holdMembers: true });
	await page.goto(`${path}&group=${areaId}&section=members`);
	const members = page.getByTestId('organization-members');
	await expect(members.getByRole('status', { name: 'กำลังโหลดสมาชิก', exact: true })).toBeVisible();
	await expect(members).toContainText('— คน');
	await expect(members).not.toContainText('0 คน');
	await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).click();
	await page.getByRole('button', { name: 'หัวหน้าและสมาชิก', exact: true }).click();
	await expect(members).toContainText('สมาชิกสอง');
	state.releaseMembers();
	await expect(members).not.toContainText('สมาชิกแรก');
});

test('editing uses the existing revision contract and preserves the protected activity registry', async ({
	page
}) => {
	const state = await fixture(page, { activity: true });
	await page.goto(`${path}&group=${areaId}&section=info`);
	await page.getByRole('button', { name: 'แก้ไขข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อกลุ่มสาระ').fill('คณิตศาสตร์เพิ่มเติม');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'คณิตศาสตร์เพิ่มเติม', exact: true })
	).toBeVisible();
	expect(state.writes[0]).toMatchObject({ rowVersion: 1, nameTh: 'คณิตศาสตร์เพิ่มเติม' });
	expect(state.writes[0]).not.toHaveProperty('code');
	await expect(page.locator('main')).not.toContainText('SG-000001');
	await page.getByRole('button', { name: 'กิจกรรมพัฒนาผู้เรียน', exact: true }).click();
	await page.getByRole('button', { name: 'แก้ไขข้อมูล', exact: true }).click();
	await expect(
		page.getByRole('dialog').getByRole('switch', { name: 'เปิดใช้งาน', exact: true })
	).toHaveCount(0);
});

test.describe('mobile group workspace', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('selection opens detail, back restores filters, and history restores the selected group', async ({
		page
	}, testInfo) => {
		const state = await fixture(page);
		await page.goto(path);
		await expect(page.getByTestId('learning-area-list')).toBeVisible();
		await page.screenshot({
			path: testInfo.outputPath('mobile-list.png'),
			fullPage: true,
			animations: 'disabled'
		});
		await page.getByLabel('ค้นหากลุ่มสาระ').fill('วิทยาศาสตร์');
		await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).tap();
		const heading = page.getByRole('heading', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ });
		await expect(heading).toBeFocused();
		await expect(page.getByTestId('learning-area-list')).not.toBeVisible();
		await page.getByRole('button', { name: 'หัวหน้าและสมาชิก', exact: true }).tap();
		await expect(page.getByTestId('organization-members')).toContainText('สมาชิกสอง');
		await page.screenshot({
			path: testInfo.outputPath('mobile-members.png'),
			fullPage: true,
			animations: 'disabled'
		});
		await page.reload();
		await expect(page.getByTestId('organization-members')).toContainText('สมาชิกสอง');
		await page.getByRole('button', { name: 'กลับไปเลือกกลุ่มสาระ', exact: true }).tap();
		await expect(page.getByRole('heading', { name: 'เลือกกลุ่มสาระ', exact: true })).toBeFocused();
		await expect(page.getByLabel('ค้นหากลุ่มสาระ')).toHaveValue('วิทยาศาสตร์');
		await page.goBack();
		await expect(heading).toBeVisible();
		await expect(page.getByTestId('organization-members')).toContainText('สมาชิกสอง');
		expect(state.writes).toHaveLength(0);
	});
});

test('a save finishing after browser history changes does not replace the newer group selection', async ({
	page
}) => {
	const state = await fixture(page, { holdSave: true });
	await page.goto(`${path}&group=${areaId}&section=info`);
	await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).click();
	await page.goBack();
	await page.getByRole('button', { name: 'แก้ไขข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อกลุ่มสาระ').fill('คณิตศาสตร์เพิ่มเติม');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(1);
	await page.goForward();
	await expect(page).toHaveURL(/group=.*2&section=info/);
	state.release();
	await expect(page.getByRole('dialog')).not.toBeVisible();
	await expect(
		page.getByRole('heading', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ })
	).toBeVisible();
	await expect(page).toHaveURL(/group=.*2&section=info/);
	expect(state.reads()).toBe(1);
});

for (const width of [390, 1440])
	test(`section navigation stays in place for long names at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await fixture(page);
		await page.goto(`${path}&group=${areaId}&section=info`);
		const panel = page.getByTestId('learning-area-detail');
		const nav = page.getByRole('navigation', { name: 'ข้อมูลกลุ่มสาระ', exact: true });
		const offset = async () => {
			const panelBox = await panel.boundingBox(),
				navBox = await nav.boundingBox();
			if (!panelBox || !navBox) throw new Error('Expected visible detail navigation');
			return navBox.y - panelBox.y;
		};
		await expect(page.getByRole('heading', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
		const before = await offset();
		if (width < 1024)
			await page.getByRole('button', { name: 'กลับไปเลือกกลุ่มสาระ', exact: true }).click();
		await page.getByRole('button', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ }).click();
		await expect(
			page.getByRole('heading', { name: /วิทยาศาสตร์และเทคโนโลยีที่มีชื่อยาว/ })
		).toBeVisible();
		expect(Math.abs((await offset()) - before)).toBeLessThan(1);
	});
