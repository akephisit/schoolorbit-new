import { expect, test, type Page } from '@playwright/test';
import { mockStaffDirectory, staffPath } from './fixtures/staff-directory-route-data';
import { revalidatePage } from './fixtures/page-revalidation';
import { firstUnit, mockOrganization } from './fixtures/organization-route-data';
import { id, firstStaff, secondStaff } from './fixtures/staff-home-route-data';
import type {
	OrganizationUnit,
	OrganizationMemberItem,
	OrganizationUnitResponse
} from '../../src/lib/api/staff.js';

test.use({ serviceWorkers: 'block' });
const group = id(301),
	child = id(302),
	deep = id(303);
const unit = (
	unitId: string,
	name: string,
	unit_type: string,
	parent_unit_id: string | null
): OrganizationUnit => ({
	id: unitId,
	name,
	unit_type,
	parent_unit_id,
	code: unitId,
	name_en: null,
	category: 'administrative',
	display_order: 0,
	is_active: true,
	is_system: unitId === firstUnit,
	description: null,
	phone: null,
	email: null,
	location: null,
	subject_group_id: null,
	created_at: '',
	updated_at: ''
});
const units = [
	unit(firstUnit, 'โรงเรียน', 'school', null),
	unit(group, 'กลุ่มบริหารทดสอบ', 'management_group', firstUnit),
	unit(child, 'งานต้นทาง', 'division', group),
	unit(deep, 'งานชั้นลึก', 'division', child)
];
const member = (source: string, user = firstStaff): OrganizationMemberItem => ({
	organization_unit_id: source,
	organization_unit_name: units.find((item) => item.id === source)!.name,
	user_id: user,
	name: user === firstStaff ? 'สมาชิกหลายงาน' : 'หัวหน้าชั้นลึก',
	title: '',
	position_code: source === group ? 'member' : 'head',
	position_title: null,
	is_primary: false,
	responsibilities: null,
	started_at: '2026-01-01'
});
async function fixture(page: Page, deepName?: string) {
	const displayUnits = units.map((unit) =>
		unit.id === deep && deepName ? { ...unit, name: deepName } : unit
	);
	const state = await mockOrganization(page, { units: displayUnits });
	const rows = [member(group), member(child), member(deep), member(deep, secondStaff)].map(
		(row) => ({
			...row,
			organization_unit_name: displayUnits.find((unit) => unit.id === row.organization_unit_id)!
				.name
		})
	);
	await page.route('**/api/organization/units/*/members*', async (route) => {
		if (route.request().method() !== 'GET') return route.fallback();
		const url = new URL(route.request().url());
		const owner = url.pathname.split('/').at(-2)!;
		return route.fulfill({
			json: {
				success: true,
				data:
					url.searchParams.get('include_children') === 'true'
						? rows
						: rows.filter((row) => row.organization_unit_id === owner)
			}
		});
	});
	return state;
}

test('parent membership deduplicates all depths and mutations target the selected original assignment', async ({
	page
}) => {
	await fixture(page);
	const writes: string[] = [];
	await page.route('**/api/organization/units/*/members/*', async (route) => {
		writes.push(`${route.request().method()} ${new URL(route.request().url()).pathname}`);
		await route.fulfill({ json: { success: true, data: {} } });
	});
	await page.goto(`/staff/organization/${group}`);
	const roster = page.getByTestId('organization-members');
	await expect(roster.getByText('สมาชิกหลายงาน', { exact: true })).toHaveCount(1);
	await expect(roster.getByText('หัวหน้าชั้นลึก', { exact: true })).toHaveCount(1);
	await expect(roster).toContainText('สมาชิกจากหน่วยงานย่อย');
	const sources = roster.getByLabel('สังกัดต้นทาง สมาชิกหลายงาน', { exact: true });
	await expect(sources.getByRole('link')).toHaveCount(3);
	await sources
		.getByRole('button', { name: 'แก้ไขสมาชิก สมาชิกหลายงาน ใน งานชั้นลึก', exact: true })
		.click();
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByRole('dialog')).not.toBeVisible();
	expect(writes).toContain(`PUT /api/organization/units/${deep}/members/${firstStaff}`);
	await sources
		.getByRole('button', { name: 'ลบสมาชิก สมาชิกหลายงาน จาก งานต้นทาง', exact: true })
		.click();
	await expect
		.poll(() => writes)
		.toContain(`DELETE /api/organization/units/${child}/members/${firstStaff}`);
});

test('root overview deduplicates inherited people from every source depth', async ({ page }) => {
	await fixture(page);
	await page.goto(`/staff/organization?unit=${group}`);
	await expect(
		page.getByTestId('organization-selected-members').getByText('สมาชิกหลายงาน', { exact: true })
	).toHaveCount(1);
});

test('permanent deletion retains reference errors and removes only the successful management group', async ({
	page
}) => {
	await fixture(page);
	let attempts = 0;
	await page.route(`**/api/organization/units/${group}/permanent`, async (route) => {
		attempts++;
		await route.fulfill({
			status: attempts === 1 ? 409 : 200,
			json:
				attempts === 1
					? { success: false, error: 'ยังมีหน่วยงานย่อย กรุณาย้ายหน่วยงานย่อยก่อนลบ' }
					: { success: true, data: {} }
		});
	});
	await page.goto(`/staff/organization?unit=${group}`);
	await page.getByRole('button', { name: 'ลบกลุ่มบริหารถาวร', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true }).click();
	await expect(dialog).toContainText('กรุณาย้ายหน่วยงานย่อยก่อนลบ');
	await dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	await expect(
		page
			.getByRole('navigation', { name: 'โครงสร้างหน่วยงาน', exact: true })
			.getByRole('button', { name: /^กลุ่มบริหารทดสอบ/ })
	).toHaveCount(0);
	expect(attempts).toBe(2);
});

test('an inherited member remains available for an explicit assignment in the chosen parent', async ({
	page
}) => {
	await fixture(page);
	let target = '';
	await page.route('**/api/lookup/staff**', (route) =>
		route.fulfill({
			json: { success: true, data: [{ id: secondStaff, name: 'หัวหน้าชั้นลึก', title: '' }] }
		})
	);
	await page.route(`**/api/organization/units/${group}/members`, async (route) => {
		if (route.request().method() === 'GET') return route.fallback();
		target = route.request().postDataJSON().user_id;
		await route.fulfill({ json: { success: true, data: {} } });
	});
	await page.goto(`/staff/organization/${group}`);
	await page
		.getByTestId('organization-members')
		.getByRole('button', { name: 'เพิ่มสมาชิก', exact: true })
		.click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('combobox', { name: /บุคลากร/ }).click();
	await page.getByRole('option', { name: /หัวหน้าชั้นลึก/ }).click();
	await dialog.getByRole('button', { name: 'เพิ่ม', exact: true }).click();
	await expect.poll(() => target).toBe(secondStaff);
});

test('mobile dark source controls wrap long names and support keyboard edit and pointer removal', async ({
	page
}, info) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.emulateMedia({ colorScheme: 'dark' });
	const deepName =
		'งานวัดผลและประเมินคุณภาพการศึกษาที่มีชื่อยาวมากเพื่อทดสอบรายชื่อสังกัดต้นทางในหน้าจอโทรศัพท์';
	await fixture(page, deepName);
	const writes: string[] = [];
	await page.route('**/api/organization/units/*/members/*', async (route) => {
		writes.push(`${route.request().method()} ${new URL(route.request().url()).pathname}`);
		await route.fulfill({ json: { success: true, data: {} } });
	});
	await page.goto(`/staff/organization/${group}`);
	await expect(page.locator('html')).toHaveClass(/dark/);
	const sources = page
		.getByTestId('organization-members')
		.getByLabel('สังกัดต้นทาง สมาชิกหลายงาน', { exact: true });
	const edit = sources.getByRole('button', {
		name: `แก้ไขสมาชิก สมาชิกหลายงาน ใน ${deepName}`,
		exact: true
	});
	await edit.focus();
	await page.keyboard.press('Enter');
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	await expect(dialog).toContainText(deepName);
	expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await sources
		.getByRole('button', { name: `ลบสมาชิก สมาชิกหลายงาน จาก ${deepName}`, exact: true })
		.click();
	await expect
		.poll(() => writes)
		.toContain(`DELETE /api/organization/units/${deep}/members/${firstStaff}`);
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.screenshot({
		path: info.outputPath('mobile-dark-inherited-members.png'),
		fullPage: true
	});
});

test('staff profile preserves multiple facts for one source unit and distinguishes both primary domains', async ({
	page
}) => {
	const source: OrganizationUnitResponse = {
		id: group,
		code: 'SYNTHETIC',
		name: 'กลุ่มบริหารต้นทาง',
		category: 'administrative',
		unit_type: 'management_group',
		subject_group_id: null,
		position_code: 'head',
		position_title: null,
		is_primary: true,
		responsibilities: 'หน้าที่ช่วงปัจจุบัน'
	};
	const profile = {
		organization_units: [
			source,
			{ ...source, is_primary: false, responsibilities: 'หน้าที่กำหนดล่วงหน้า' },
			{
				...source,
				id: child,
				unit_type: 'subject_group',
				subject_group_id: id(351),
				name: 'กลุ่มสาระทดสอบ',
				responsibilities: 'หน้าที่กลุ่มสาระ'
			}
		]
	};
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await mockStaffDirectory(page, { profile });
	await page.goto(staffPath());
	await expect(page.getByText('หน้าที่ช่วงปัจจุบัน', { exact: true })).toBeVisible();
	await expect(page.getByText('หน้าที่กำหนดล่วงหน้า', { exact: true })).toBeVisible();
	await expect(page.getByText('สังกัดบริหารหลัก', { exact: true })).toBeVisible();
	await expect(page.getByText('กลุ่มสาระหลัก', { exact: true })).toBeVisible();
	profile.organization_units = profile.organization_units.filter(
		(fact) => fact.responsibilities !== 'หน้าที่กำหนดล่วงหน้า'
	);
	await revalidatePage(page);
	await expect(page.getByText('หน้าที่กำหนดล่วงหน้า', { exact: true })).toHaveCount(0);
	await expect(page.getByText('หน้าที่ช่วงปัจจุบัน', { exact: true })).toBeVisible();
	expect(errors).toEqual([]);
});
