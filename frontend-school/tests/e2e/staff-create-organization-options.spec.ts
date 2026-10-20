import { expect, test, type Page } from '@playwright/test';
import type { CreateStaffRequest, OrganizationUnit } from '../../src/lib/api/staff.js';
import { mockStaffDirectory, roleId, staffPath } from './fixtures/staff-directory-route-data';
import { actor, id } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block' });
const school = id(201),
	academic = id(202),
	division = id(203),
	deepest = id(204),
	budget = id(205);
const makeUnit = (
	id: string,
	name: string,
	parent_unit_id: string | null,
	unit_type: string,
	display_order = 0
): OrganizationUnit => ({
	id,
	name,
	parent_unit_id,
	unit_type,
	display_order,
	code: `SYNTHETIC-${id.slice(-3)}`,
	name_en: null,
	category: 'general',
	subject_group_id: null,
	is_active: true,
	is_system: false,
	description: null,
	phone: null,
	email: null,
	location: null,
	created_at: '',
	updated_at: ''
});
const units = [
	makeUnit(
		deepest,
		'งานประเมินผลการเรียนรู้ที่มีชื่อยาวเพื่อทดสอบการเลือกสังกัด',
		division,
		'division'
	),
	makeUnit(division, 'งานทะเบียน', academic, 'division'),
	makeUnit(id(206), 'งานทะเบียน', budget, 'division'),
	makeUnit(budget, 'กลุ่มบริหารงบประมาณ', school, 'management_group', 2),
	makeUnit(academic, 'กลุ่มบริหารวิชาการ', school, 'management_group', 1),
	{ ...makeUnit(school, 'โรงเรียนตัวอย่าง', null, 'school'), code: 'SCHOOL' },
	makeUnit(id(207), 'กลุ่มสาระภาษาไทย', school, 'subject_group', 3),
	makeUnit(id(208), 'งานเดิม', id(999), 'division', 4)
];
const positions = [
	['director', 'ผู้อำนวยการ'],
	['deputy_director', 'รองผู้อำนวยการ'],
	['head', 'หัวหน้า'],
	['deputy_head', 'รองหัวหน้า'],
	['coordinator', 'ผู้ประสานงาน'],
	['member', 'สมาชิก']
] as const;

async function fixture(page: Page, options: { empty?: boolean; failFirst?: boolean } = {}) {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
	await page.addInitScript(
		({ actor, roleId }) => {
			const key = `staff-create-draft:v5:${location.origin}:${actor}`;
			if (!localStorage.getItem(key))
				localStorage.setItem(
					key,
					JSON.stringify({
						version: 5,
						expiresAt: Date.now() + 30 * 60 * 1000,
						fields: {
							role_ids: [roleId],
							primary_role_id: roleId,
							organization_assignments: [
								{
									organization_unit_id: '',
									position_code: 'member',
									is_primary: true,
									responsibilities: ''
								}
							]
						}
					})
				);
		},
		{ actor, roleId }
	);
	const api = await mockStaffDirectory(page);
	let organizationReads = 0;
	await page.route('**/api/organization/units', (route) => {
		organizationReads++;
		const failed = options.failFirst && organizationReads === 1;
		return route.fulfill({
			status: failed ? 503 : 200,
			contentType: 'application/json',
			body: JSON.stringify(
				failed
					? { success: false, error: 'ตัวเลือกสังกัดยังไม่พร้อม' }
					: { success: true, data: options.empty ? [] : units }
			)
		});
	});
	return { api, organizationReads: () => organizationReads };
}

async function next(page: Page, count = 1) {
	for (let index = 0; index < count; index++)
		await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
}

async function personal(page: Page, restoring = false) {
	if (!restoring) {
		await page.getByRole('button', { name: 'คำนำหน้า (ไทย)', exact: true }).click();
		await page.getByRole('option', { name: 'นาย', exact: true }).click();
		await page.getByPlaceholder('ชื่อ', { exact: true }).fill('บุคลากรใหม่');
		await page.getByPlaceholder('นามสกุล', { exact: true }).fill('ทดสอบ');
	}
	await page.locator('input[type=password]').nth(0).fill('synthetic-passphrase');
	await page.locator('input[type=password]').nth(1).fill('synthetic-passphrase');
}

const picker = (page: Page, row = 1) =>
	page.getByRole('combobox', { name: `ชื่อหน่วยงานที่ ${row}`, exact: true });
const position = (page: Page, row = 1) =>
	page.getByRole('button', { name: `ตำแหน่งในหน่วยงานที่ ${row}`, exact: true });
const path = (unitId: string): string => {
	const unit = units.find((unit) => unit.id === unitId)!;
	return unit.parent_unit_id && units.some((parent) => parent.id === unit.parent_unit_id)
		? `${path(unit.parent_unit_id)} › ${unit.name}`
		: unit.name;
};
const optionName = (unitId: string): string => {
	const unit = units.find((unit) => unit.id === unitId)!;
	const parent = units.find((parent) => parent.id === unit.parent_unit_id);
	return parent ? `${unit.name} ${path(parent.id)}` : unit.name;
};

async function chooseUnit(page: Page, unitId: string, row = 1) {
	await picker(page, row).click();
	await page.getByRole('option', { name: optionName(unitId), exact: true }).click();
	await expect(picker(page, row)).toContainText(path(unitId));
}

test('all six positions can be reviewed and submitted with full-depth organization IDs', async ({
	page
}) => {
	const state = await fixture(page);
	let submitted: CreateStaffRequest | undefined;
	page.on('request', (request) => {
		if (new URL(request.url()).pathname === '/api/staff' && request.method() === 'POST')
			submitted = request.postDataJSON();
	});
	await page.goto('/staff/manage/new');
	expect(state.organizationReads()).toBe(0);
	await personal(page);
	await next(page);
	expect(state.organizationReads()).toBe(0);
	await next(page);
	const selectedUnits = [school, academic, division, deepest, budget, id(207)];
	for (const [index, [, label]] of positions.entries()) {
		if (index) await page.getByRole('button', { name: '+ เพิ่มหน่วยงาน', exact: true }).click();
		await chooseUnit(page, selectedUnits[index], index + 1);
		await position(page, index + 1).click();
		await expect(page.getByRole('option')).toHaveText(positions.map(([, label]) => label));
		await page.getByRole('option', { name: label, exact: true }).click();
		await page
			.getByLabel(`หน้าที่รับผิดชอบในหน่วยงานที่ ${index + 1}`, { exact: true })
			.fill(`หน้าที่ ${index + 1}`);
	}
	await page.getByRole('checkbox', { name: 'สังกัดหลักหน่วยงานที่ 6', exact: true }).click();
	await page.getByRole('checkbox', { name: 'สังกัดหลักหน่วยงานที่ 4', exact: true }).click();
	await expect(page.getByRole('checkbox', { name: 'สังกัดหลักหน่วยงานที่ 6', exact: true })).toBeChecked();
	await next(page);
	const review = page.getByTestId('staff-create-review');
	for (const [index, [, label]] of positions.entries())
		await expect(
			review.getByRole('listitem').filter({ hasText: `${path(selectedUnits[index])} • ${label}` })
		).toHaveCount(1);
	expect(state.api.writes).toHaveLength(0);
	await page.getByRole('button', { name: 'สร้างบุคลากร', exact: true }).click();
	await expect(page).toHaveURL(staffPath(id(87)));
	expect(submitted?.organization_assignments).toEqual(
		positions.map(([code], index) => ({
			organization_unit_id: selectedUnits[index],
			position_code: code,
			is_primary: index === 3 || index === 5,
			responsibilities: `หน้าที่ ${index + 1}`
		}))
	);
	expect(state.api.writes).toEqual([{ method: 'POST', path: '/api/staff' }]);
	expect(state.organizationReads()).toBe(1);
});

test('keyboard search reaches the deepest unit, shows same-name contexts and retains the v5 draft', async ({
	page
}) => {
	const state = await fixture(page);
	await page.goto('/staff/manage/new');
	await personal(page);
	await next(page, 2);
	await picker(page).focus();
	await page.keyboard.press('Enter');
	const search = page.getByRole('combobox', { name: 'ค้นหาชื่อหน่วยงานที่ 1', exact: true });
	await search.fill('งานทะเบียน');
	await expect(page.getByRole('option', { name: optionName(division), exact: true })).toBeVisible();
	await expect(page.getByRole('option', { name: optionName(id(206)), exact: true })).toBeVisible();
	await search.fill('คำค้นที่ไม่ตรง');
	await expect(page.getByText('ไม่พบหน่วยงานที่ตรงกับคำค้น', { exact: true })).toBeVisible();
	await search.fill('งานประเมินผล');
	await expect(page.getByRole('option')).toHaveCount(1);
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(picker(page)).toBeFocused();
	await expect(picker(page)).toContainText(path(deepest));
	await position(page).click();
	await page.getByRole('option', { name: 'รองหัวหน้า', exact: true }).click();
	await page.getByLabel('หน้าที่รับผิดชอบในหน่วยงานที่ 1', { exact: true }).fill('เก็บร่างหน้าที่');
	await next(page);
	await expect(page.getByTestId('staff-create-review')).toContainText('รองหัวหน้า');
	await page.getByRole('button', { name: 'แก้ไขสังกัด', exact: true }).click();
	await expect(position(page)).toContainText('รองหัวหน้า');
	await expect(page.getByLabel('หน้าที่รับผิดชอบในหน่วยงานที่ 1', { exact: true })).toHaveValue(
		'เก็บร่างหน้าที่'
	);
	await page.getByRole('button', { name: 'ย้อนกลับ', exact: true }).click();
	await next(page);
	await expect(picker(page)).toContainText(path(deepest));
	expect(state.organizationReads()).toBe(1);
	await page.reload();
	await personal(page, true);
	await next(page, 2);
	await expect(picker(page)).toContainText(path(deepest));
	await expect(position(page)).toContainText('รองหัวหน้า');
	await expect(
		page.getByRole('checkbox', { name: 'สังกัดหลักหน่วยงานที่ 1', exact: true })
	).toBeChecked();
	await expect(page.getByLabel('หน้าที่รับผิดชอบในหน่วยงานที่ 1', { exact: true })).toHaveValue(
		'เก็บร่างหน้าที่'
	);
	expect(state.api.writes).toHaveLength(0);
});

test('organization read errors retry locally and empty options are explicit', async ({ page }) => {
	const state = await fixture(page, { failFirst: true, empty: true });
	await page.goto('/staff/manage/new');
	await personal(page);
	await next(page, 2);
	await expect(page.getByText('ตัวเลือกสังกัดยังไม่พร้อม', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await picker(page).click();
	await expect(page.getByText('ยังไม่มีหน่วยงานให้เลือก', { exact: true })).toBeVisible();
	await expect(page.getByText('ไม่พบหน่วยงานที่ตรงกับคำค้น', { exact: true })).toHaveCount(0);
	await page.keyboard.press('Escape');
	await expect(picker(page)).toBeFocused();
	expect(state.organizationReads()).toBe(2);
	expect(state.api.count('/api/roles')).toBe(1);
	expect(state.api.writes).toHaveLength(0);
});

for (const theme of ['light', 'dark'] as const)
	for (const width of [390, 1440]) {
		test(`organization picker layout ${theme} ${width}`, async ({ page }, info) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme: theme });
			const state = await fixture(page);
			await page.goto('/staff/manage/new');
			await personal(page);
			await next(page, 2);
			await picker(page).click();
			const popover = page.locator('[data-slot="popover-content"]');
			await expect(
				page.getByRole('option', { name: optionName(deepest), exact: true })
			).toBeVisible();
			await popover.evaluate(async (element) => {
				await Promise.all(element.getAnimations().map((animation) => animation.finished));
			});
			const bounds = await popover.boundingBox();
			await info.attach('picker-bounds', {
				body: JSON.stringify({ bounds, viewportWidth: width }),
				contentType: 'application/json'
			});
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			expect(await popover.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
				true
			);
			await page.screenshot({
				path: info.outputPath('picker.png'),
				fullPage: true,
				animations: 'disabled'
			});
			await page.getByRole('option', { name: optionName(deepest), exact: true }).click();
			await expect(picker(page)).toContainText(path(deepest));
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			if (theme === 'dark') await expect(page.locator('html')).toHaveClass(/dark/);
			else await expect(page.locator('html')).not.toHaveClass(/dark/);
			await page.screenshot({
				path: info.outputPath('selected.png'),
				fullPage: true,
				animations: 'disabled'
			});
			expect(state.api.writes).toHaveLength(0);
		});
	}

test.describe('touch affiliation selection', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('one tap selects a deep unit and keeps the selected position', async ({ page }) => {
		const state = await fixture(page);
		await page.goto('/staff/manage/new');
		await personal(page);
		await next(page, 2);
		await picker(page).tap();
		await page
			.getByRole('combobox', { name: 'ค้นหาชื่อหน่วยงานที่ 1', exact: true })
			.fill('งานประเมินผล');
		await page.getByRole('option', { name: optionName(deepest), exact: true }).tap();
		await expect(page.locator('[data-slot="popover-content"]')).toHaveCount(0);
		await expect(picker(page)).toContainText(path(deepest));
		await position(page).tap();
		await page.getByRole('option', { name: 'ผู้ประสานงาน', exact: true }).tap();
		await picker(page).tap();
		await page.getByRole('option', { name: optionName(id(208)), exact: true }).tap();
		await expect(picker(page)).toContainText('งานเดิม');
		await expect(position(page)).toContainText('ผู้ประสานงาน');
		expect(state.api.writes).toHaveLength(0);
	});
});
