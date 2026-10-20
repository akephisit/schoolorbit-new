import { expect, test, type Page } from '@playwright/test';
import { mockOrganization, firstUnit, secondUnit } from './fixtures/organization-route-data';
import { firstStaff, id } from './fixtures/staff-home-route-data';
import { mockStaffDirectory, staffPath } from './fixtures/staff-directory-route-data';
import type {
	OrganizationMemberItem,
	OrganizationUnit,
	OrganizationUnitResponse,
	UpdateMemberBody,
	UpdateStaffRequest
} from '../../src/lib/api/staff.js';

test.use({ serviceWorkers: 'block' });
const title = 'ผู้รับผิดชอบงานที่กำหนดเอง';
const responsibilities = 'หน้าที่ต้นทางที่ต้องคงอยู่';
async function sourceFixture(page: Page) {
	await mockOrganization(page);
	const source: OrganizationMemberItem = {
		organization_unit_id: firstUnit,
		organization_unit_name: 'หน่วยงานแรก',
		user_id: firstStaff,
		name: 'บุคลากรตำแหน่งเฉพาะ',
		title: '',
		position_code: 'head',
		position_title: title,
		is_primary: false,
		responsibilities,
		started_at: '2026-01-01'
	};
	await page.route('**/api/organization/units/*/members?**', (route) =>
		route.fulfill({ json: { success: true, data: [source] } })
	);
	let saved: UpdateMemberBody | undefined;
	await page.route(
		`**/api/organization/units/${firstUnit}/members/${firstStaff}`,
		async (route) => {
			saved = route.request().postDataJSON();
			await route.fulfill({ json: { success: true, data: {} } });
		}
	);
	return () => saved;
}
for (const move of [false, true])
	test(`source ${move ? 'move' : 'primary edit'} preserves the persisted custom title and responsibilities`, async ({
		page
	}) => {
		const saved = await sourceFixture(page);
		await page.goto(`/staff/organization/${firstUnit}`);
		await page
			.getByRole('button', { name: 'แก้ไขสมาชิก บุคลากรตำแหน่งเฉพาะ ใน หน่วยงานแรก', exact: true })
			.click();
		const dialog = page.getByRole('dialog');
		if (move) {
			await dialog.getByRole('button', { name: 'หน่วยงานแรก', exact: true }).click();
			await page.getByRole('option', { name: 'หน่วยงานสอง', exact: true }).click();
		} else await dialog.locator('#edit-primary').check();
		await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect
			.poll(saved)
			.toMatchObject({
				position_title: title,
				responsibilities,
				...(move ? { new_organization_unit_id: secondUnit } : { is_primary: true })
			});
	});

test('bulk primary edit retains inactive source custom titles and original null responsibilities', async ({
	page
}) => {
	const administration: OrganizationUnitResponse = {
		id: firstUnit,
		code: 'OLD',
		name: 'สังกัดบริหารเดิม',
		category: 'administrative',
		unit_type: 'management_group',
		subject_group_id: null,
		position_code: 'head',
		position_title: title,
		is_primary: true,
		responsibilities: null
	};
	const learningArea: OrganizationUnitResponse = {
		...administration,
		id: secondUnit,
		code: 'AREA',
		name: 'กลุ่มสาระทดสอบ',
		unit_type: 'subject_group',
		subject_group_id: id(901),
		position_title: 'หัวหน้ากลุ่มสาระเฉพาะ',
		is_primary: false,
		responsibilities
	};
	await mockStaffDirectory(page, {
		profile: { organization_units: [administration, learningArea] }
	});
	const units: OrganizationUnit[] = [administration, learningArea].map((source, index) => ({
		id: source.id,
		name: source.name,
		code: source.code,
		category: source.category ?? 'general',
		unit_type: source.unit_type ?? 'unit',
		subject_group_id: source.subject_group_id,
		is_active: index === 1,
		is_system: false,
		name_en: null,
		parent_unit_id: null,
		display_order: index,
		description: null,
		phone: null,
		email: null,
		location: null,
		created_at: '',
		updated_at: ''
	}));
	await page.route('**/api/organization/units**', (route) =>
		route.fulfill({ json: { success: true, data: units } })
	);
	let saved: UpdateStaffRequest | undefined;
	await page.route(`**/api/staff/${firstStaff}`, async (route) => {
		if (route.request().method() === 'GET') return route.fallback();
		saved = route.request().postDataJSON();
		await route.fulfill({ json: { success: true, data: {} } });
	});
	await page.goto(staffPath(firstStaff, '/edit?section=organizations'));
	await page.getByRole('checkbox', { name: 'สังกัดหลักหน่วยงานที่ 2', exact: true }).check();
	await expect(
		page.getByRole('checkbox', { name: 'สังกัดหลักหน่วยงานที่ 1', exact: true })
	).toBeChecked();
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect
		.poll(() => saved?.organization_assignments)
		.toEqual([
			{
				organization_unit_id: firstUnit,
				position_code: 'head',
				position_title: title,
				is_primary: true,
				responsibilities: null
			},
			{
				organization_unit_id: secondUnit,
				position_code: 'head',
				position_title: learningArea.position_title,
				is_primary: true,
				responsibilities
			}
		]);
});
