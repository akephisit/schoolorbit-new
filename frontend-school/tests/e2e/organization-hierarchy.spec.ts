import { expect, test, type Page } from '@playwright/test';
import { mockOrganization, firstUnit } from './fixtures/organization-route-data';
import { expectCompactOrganizationToolbar } from './fixtures/organization-toolbar';
import type { OrganizationUnit, UpdateOrganizationUnitRequest } from '../../src/lib/api/staff.js';

test.use({ serviceWorkers: 'block' });
const unitId = (index: number) => `40000000-0000-4000-8000-${String(index).padStart(12, '0')}`;
const academic = unitId(1),
	budget = unitId(2),
	registration = unitId(3),
	finance = unitId(4),
	nested = unitId(5);
const groupName = 'กลุ่มบริหารวิชาการและพัฒนาคุณภาพการศึกษาที่มีชื่อยาว';
const makeUnit = (
	id: string,
	name: string,
	kind: string,
	parent: string | null,
	order: number
): OrganizationUnit => ({
	id,
	name,
	code: `INTERNAL-${order}`,
	name_en: null,
	unit_type: kind,
	parent_unit_id: parent,
	category: 'general',
	display_order: order,
	is_active: true,
	is_system: id === firstUnit,
	description: null,
	phone: null,
	email: null,
	location: null,
	subject_group_id: null,
	created_at: '',
	updated_at: ''
});
const units = [
	makeUnit(firstUnit, 'โรงเรียนตัวอย่าง', 'school', null, 0),
	makeUnit(academic, groupName, 'management_group', firstUnit, 1),
	makeUnit(budget, 'กลุ่มบริหารงบประมาณ', 'management_group', firstUnit, 2),
	makeUnit(registration, 'งานทะเบียน', 'division', academic, 3),
	makeUnit(finance, 'งานการเงิน', 'division', budget, 4),
	makeUnit(
		nested,
		'งานวัดผลและประเมินผลการเรียนรู้ที่มีชื่อยาวเพื่อทดสอบการตัดบรรทัด',
		'division',
		registration,
		5
	),
	makeUnit(unitId(6), 'งานกิจการนักเรียน', 'division', firstUnit, 6),
	{ ...makeUnit(unitId(7), 'งานเดิมที่ปิดใช้งาน', 'division', budget, 7), is_active: false }
];
async function fixture(page: Page, options: { empty?: boolean; fail?: string } = {}) {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
	return mockOrganization(page, { units: options.empty ? [] : units, fail: options.fail });
}
const tree = (page: Page) =>
	page.getByRole('navigation', { name: 'โครงสร้างหน่วยงาน', exact: true });
const choose = (page: Page, name: string) =>
	tree(page).getByRole('button', { name: new RegExp(`^${name}`) });

test('search and type filters retain the real parent chain and survive reload without refetching', async ({
	page
}) => {
	const state = await fixture(page);
	await page.goto('/staff/organization');
	await page.getByLabel('ค้นหากลุ่มบริหารหรือฝ่าย/งาน').fill('งานทะเบียน');
	await expect(tree(page).getByRole('button', { name: new RegExp(groupName) })).toBeVisible();
	await expect(tree(page).getByRole('button', { name: /^โรงเรียนตัวอย่าง/ })).toBeVisible();
	await expect(tree(page).getByRole('button', { name: /^กลุ่มบริหารงบประมาณ/ })).toHaveCount(0);
	await expect(tree(page).getByLabel(`ภายใต้ ${groupName}`, { exact: true })).toContainText(
		'งานทะเบียน'
	);
	await expect(tree(page).getByLabel('ภายใต้ งานทะเบียน', { exact: true })).toContainText(
		'งานวัดผล'
	);
	await page.getByRole('radio', { name: 'ฝ่าย/งาน', exact: true }).click();
	await page.getByRole('radio', { name: 'ฝ่าย/งาน', exact: true }).click();
	await expect(page.getByRole('radio', { name: 'ฝ่าย/งาน', exact: true })).toBeChecked();
	await expect(page).toHaveURL(/type=division/);
	expect(state.count('/api/organization/units')).toBe(1);
	await page.reload();
	await expect(page.getByLabel('ค้นหากลุ่มบริหารหรือฝ่าย/งาน')).toHaveValue('งานทะเบียน');
	await expect(page.getByRole('radio', { name: 'ฝ่าย/งาน', exact: true })).toBeChecked();
	await expect(tree(page).getByLabel(`ภายใต้ ${groupName}`, { exact: true })).toContainText(
		'งานทะเบียน'
	);
	expect(state.writes).toHaveLength(0);
});

test('selected details expose the management group, breadcrumbs and children without organization codes', async ({
	page
}) => {
	const state = await fixture(page);
	await page.goto(
		`/staff/organization?unit=${registration}&tab=administration&group=remembered&section=info`
	);
	const detail = page.getByTestId('organization-selected-detail');
	await expect(detail.getByRole('heading', { name: 'งานทะเบียน', exact: true })).toBeVisible();
	await expect(detail.locator('dl')).toContainText(groupName);
	await expect(page.getByRole('navigation', { name: 'สายสังกัด' })).toContainText(groupName);
	await expect(detail.getByRole('button', { name: /^งานวัดผล/ })).toBeVisible();
	await expect(page.locator('main')).not.toContainText('INTERNAL-');
	await choose(page, 'งานการเงิน').focus();
	await page.keyboard.press('Enter');
	await expect(detail.getByRole('heading', { name: 'งานการเงิน', exact: true })).toBeVisible();
	await expect(detail.locator('dl')).toContainText('กลุ่มบริหารงบประมาณ');
	await expect(page).toHaveURL(/group=remembered/);
	await page.goBack();
	await expect(detail.getByRole('heading', { name: 'งานทะเบียน', exact: true })).toBeVisible();
	expect(state.count('/api/organization/units')).toBe(1);
	expect(state.writes).toHaveLength(0);
});

test('contextual create names its parent without codes and preserves a failed draft', async ({
	page
}) => {
	await fixture(page);
	await page.route('**/api/organization/units', async (route) => {
		if (route.request().method() === 'GET') return route.fallback();
		expect(route.request().postDataJSON()).toMatchObject({ parent_unit_id: academic });
		expect(route.request().postDataJSON()).not.toHaveProperty('code');
		return route.fulfill({
			status: 409,
			contentType: 'application/json',
			body: JSON.stringify({ success: false, error: 'ทดสอบบันทึกไม่สำเร็จ' })
		});
	});
	await page.goto(`/staff/organization?unit=${academic}`);
	await page.getByRole('button', { name: 'เพิ่มฝ่าย/งาน', exact: true }).click();
	await expect(page.getByRole('dialog').getByLabel('สังกัดภายใต้')).toContainText(groupName);
	await expect(page.getByRole('dialog')).not.toContainText('INTERNAL-');
	await page.getByLabel('ชื่อหน่วยงาน (ไทย)').fill('งานใหม่');
	await page.getByRole('button', { name: 'สร้างหน่วยงาน', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('ทดสอบบันทึกไม่สำเร็จ');
	await expect(page.getByLabel('ชื่อหน่วยงาน (ไทย)')).toHaveValue('งานใหม่');
});

test('an empty structure does not masquerade as a filter result', async ({ page }) => {
	await fixture(page, { empty: true });
	await page.goto('/staff/organization');
	await expect(page.getByText('ยังไม่มีหน่วยงาน', { exact: true })).toBeVisible();
	await expect(page.getByText('ไม่พบหน่วยงานที่ตรงเงื่อนไข', { exact: true })).toHaveCount(0);
});

for (const theme of ['light', 'dark'] as const)
	for (const width of [390, 510, 1440]) {
		test(`hierarchy layout ${theme} ${width}`, async ({ page }, info) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme: theme });
			await fixture(page);
			await page.goto('/staff/organization');
			await expect(choose(page, 'งานทะเบียน')).toBeVisible();
			await expectCompactOrganizationToolbar(page, 'organization');
			if (theme === 'dark') await expect(page.locator('html')).toHaveClass(/dark/);
			else await expect(page.locator('html')).not.toHaveClass(/dark/);
			await expect(page.locator('main')).not.toContainText('INTERNAL-');
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: info.outputPath('hierarchy.png'),
				fullPage: true,
				animations: 'disabled'
			});
			await choose(page, 'งานทะเบียน').click();
			await expect(
				page
					.getByTestId('organization-selected-detail')
					.getByRole('heading', { name: 'งานทะเบียน', exact: true })
			).toBeVisible();
			await expectCompactOrganizationToolbar(page, 'organization', true);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: info.outputPath('selected.png'),
				fullPage: true,
				animations: 'disabled'
			});
		});
	}

test.describe('touch navigation', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('mobile create keeps a long parent name inside the dialog', async ({ page }) => {
		await fixture(page);
		await page.goto(`/staff/organization?unit=${academic}`);
		await page.getByRole('button', { name: 'เพิ่มฝ่าย/งาน', exact: true }).tap();
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByLabel('สังกัดภายใต้')).toContainText(groupName);
		expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
			true
		);
		await expect(dialog).not.toContainText('INTERNAL-');
		await page.keyboard.press('Escape');
		await expect(dialog).not.toBeVisible();
	});
	test('mobile opens details directly, restores the tree and supports URL history', async ({
		page
	}) => {
		const state = await fixture(page);
		await page.goto('/staff/organization');
		await expect(page.getByTestId('organization-hierarchy')).toBeVisible();
		await choose(page, 'งานทะเบียน').tap();
		await expect(page.getByTestId('organization-hierarchy')).not.toBeVisible();
		await expect(page.getByRole('heading', { name: 'งานทะเบียน', exact: true })).toBeFocused();
		await expect(page.getByTestId('organization-selected-detail')).toContainText(groupName);
		await page.getByRole('button', { name: 'กลับไปเลือกหน่วยงาน', exact: true }).tap();
		await expect(page.getByRole('heading', { name: 'เลือกหน่วยงาน', exact: true })).toBeFocused();
		await expect(tree(page)).toBeVisible();
		await page.goBack();
		await expect(page.getByRole('heading', { name: 'งานทะเบียน', exact: true })).toBeVisible();
		await page.reload();
		await expect(page.getByRole('heading', { name: 'งานทะเบียน', exact: true })).toBeVisible();
		await expect(page.getByTestId('organization-hierarchy')).not.toBeVisible();
		expect(state.writes).toHaveLength(0);
	});
});

test('detail return restores selected unit and search context and rejects external return targets', async ({
	page
}) => {
	await fixture(page);
	await page.goto(`/staff/organization?unit=${registration}&q=งานทะเบียน&type=division`);
	await page.getByRole('link', { name: 'เปิดรายละเอียด', exact: true }).click();
	await expect(page.getByTestId('organization-unit')).toContainText('งานทะเบียน');
	await expect(page.locator('main')).not.toContainText('INTERNAL-');
	await page.getByRole('link', { name: 'โครงสร้าง', exact: true }).click();
	await expect(page.getByLabel('ค้นหากลุ่มบริหารหรือฝ่าย/งาน')).toHaveValue('งานทะเบียน');
	await expect(
		page
			.getByTestId('organization-selected-detail')
			.getByRole('heading', { name: 'งานทะเบียน', exact: true })
	).toBeVisible();
	await expect(page.getByRole('radio', { name: 'ฝ่าย/งาน', exact: true })).toBeChecked();
	await page.goto(`/staff/organization/${registration}?returnTo=https%3A%2F%2Fexample.com`);
	await expect(page.getByRole('link', { name: 'โครงสร้าง', exact: true })).toHaveAttribute(
		'href',
		'/staff/organization'
	);
});

test('editing can move a division to school level with an explicit parent identity', async ({
	page
}) => {
	await fixture(page);
	let saved: UpdateOrganizationUnitRequest | undefined;
	await page.route(`**/api/organization/units/${registration}`, async (route) => {
		if (route.request().method() !== 'PUT') return route.fallback();
		saved = route.request().postDataJSON();
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({ success: true, data: {} })
		});
	});
	await page.goto(`/staff/organization?unit=${registration}`);
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('dialog').getByLabel('สังกัดภายใต้').click();
	await page.getByRole('option', { name: 'ระดับโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(page.getByRole('dialog')).not.toBeVisible();
	expect(saved).toMatchObject({ parent_unit_id: firstUnit });
	expect(saved).not.toHaveProperty('code');
});
