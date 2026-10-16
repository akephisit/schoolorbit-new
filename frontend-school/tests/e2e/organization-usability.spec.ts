import { expect, test, type Page } from '@playwright/test';
import { mockOrganization, firstUnit, unitPath } from './fixtures/organization-route-data';
import type { Permission } from '../../src/lib/api/roles.js';

test.use({ serviceWorkers: 'block' });
const permission = (index: number, module: string, action: string, name: string): Permission => ({
	id: `66000000-0000-4000-8000-${String(index).padStart(12, '0')}`,
	code: `${module}.${action}.school`,
	module,
	action,
	name,
	scope: 'school',
	description: `รายละเอียด ${name}`,
	created_at: ''
});
const own = permission(1, 'staff_profile', 'read', 'ดูข้อมูลบุคลากร'),
	edit = permission(2, 'staff_profile', 'update', 'แก้ไขข้อมูลบุคลากร'),
	hidden = permission(3, 'settings', 'update', 'ตั้งค่าบริการของโรงเรียน'),
	assessment = permission(4, 'academic_assessment', 'read', 'ดูโครงสร้างคะแนน');
const originalGrants = [
	{ permission_id: own.id, position_code: 'head' },
	{ permission_id: hidden.id, position_code: 'member' }
];
async function fixture(
	page: Page,
	options: { holdSave?: boolean; failSave?: boolean; readOnly?: boolean } = {}
) {
	const api = await mockOrganization(page, {
		permissions: options.readOnly ? ['roles.read.all', 'settings.read.all'] : undefined
	});
	await page.route('**/api/permissions/modules', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: { staff_profile: [own, edit], settings: [hidden], academic_assessment: [assessment] }
			}
		})
	);
	const writes: { grants: { permission_id: string; position_code: string | null }[] }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(`**/api/organization/units/${firstUnit}/permissions`, async (route) => {
		if (route.request().method() === 'GET')
			return route.fulfill({ json: { success: true, data: originalGrants } });
		writes.push(route.request().postDataJSON());
		if (options.holdSave) await held;
		return route.fulfill({
			status: options.failSave && writes.length === 1 ? 409 : 200,
			json:
				options.failSave && writes.length === 1
					? { success: false, error: 'ทดสอบบันทึกสิทธิ์ไม่สำเร็จ' }
					: { success: true, data: {} }
		});
	});
	return { ...api, writes, release };
}
async function openPermissions(page: Page) {
	await page.goto(unitPath());
	await page
		.getByTestId('organization-unit')
		.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
		.click();
	await expect(
		page.getByRole('dialog').getByRole('heading', { name: 'ข้อมูลบุคลากร', exact: true })
	).toBeVisible();
	return page.getByRole('dialog');
}

test('Thai search limits bulk selection to visible rows and retains hidden grants after failure', async ({
	page
}) => {
	const state = await fixture(page, { failSave: true });
	const dialog = await openPermissions(page);
	await dialog.getByLabel('ค้นหาสิทธิ์', { exact: true }).fill('ข้อมูลบุคลากร แก้ไข');
	await expect(dialog.getByText('1 จาก 4 สิทธิ์', { exact: true })).toBeVisible();
	await dialog
		.getByRole('checkbox', { name: 'เลือกสิทธิ์ที่แสดงในหมวดข้อมูลบุคลากร · หัวหน้า', exact: true })
		.check();
	await dialog.getByLabel('ค้นหาสิทธิ์', { exact: true }).fill('ไม่มีรายการตรงกัน');
	await expect(dialog.getByText('ไม่พบสิทธิ์ที่ตรงคำค้น', { exact: true })).toBeVisible();
	await expect(dialog).toContainText('เลือกไว้ 3 รายการ');
	await dialog.getByRole('button', { name: 'ล้างคำค้น', exact: true }).click();
	await expect(
		dialog.getByRole('checkbox', { name: `${edit.name} · ทั้งโรงเรียน · หัวหน้า`, exact: true })
	).toBeChecked();
	await dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true }).click();
	await expect(page.getByText('ทดสอบบันทึกสิทธิ์ไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(
		dialog.getByRole('checkbox', { name: `${edit.name} · ทั้งโรงเรียน · หัวหน้า`, exact: true })
	).toBeChecked();
	await dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true }).click();
	await expect(dialog).not.toBeVisible();
	for (const write of state.writes)
		expect(write.grants).toEqual(
			expect.arrayContaining([...originalGrants, { permission_id: edit.id, position_code: 'head' }])
		);
	expect(state.writes.map((write) => write.grants.length)).toEqual([3, 3]);
});

test('pending permission save blocks editing and duplicate writes', async ({ page }) => {
	const state = await fixture(page, { holdSave: true });
	const dialog = await openPermissions(page);
	await dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(1);
	await expect(
		dialog.getByRole('checkbox', { name: `${edit.name} · ทั้งโรงเรียน · หัวหน้า`, exact: true })
	).toBeDisabled();
	await expect(dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true })).toBeDisabled();
	state.release();
	await expect(dialog).not.toBeVisible();
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].grants).toEqual(originalGrants);
});

test('readers can search Thai permission names without changing grants', async ({ page }) => {
	const state = await fixture(page, { readOnly: true });
	await page.goto(unitPath());
	await page
		.getByTestId('organization-unit')
		.getByRole('button', { name: 'ดูสิทธิ์ตามตำแหน่ง', exact: true })
		.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByLabel('ค้นหาสิทธิ์', { exact: true })).toBeEnabled();
	await dialog.getByLabel('ค้นหาสิทธิ์', { exact: true }).fill('การวัดผล');
	await expect(dialog.getByRole('heading', { name: 'การวัดผล', exact: true })).toBeVisible();
	await expect(
		dialog.getByRole('checkbox', {
			name: `${assessment.name} · ทั้งโรงเรียน · หัวหน้า`,
			exact: true
		})
	).toBeDisabled();
	await expect(dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true })).toHaveCount(0);
	expect(state.writes).toHaveLength(0);
});

for (const width of [390, 1440])
	for (const theme of ['light', 'dark'] as const) {
		test(`unit detail, forms and permission layout ${width} ${theme}`, async ({ page }, info) => {
			await page.setViewportSize({ width, height: 884 });
			await page.emulateMedia({ colorScheme: theme });
			await fixture(page);
			await page.goto(unitPath());
			await expect(page.getByTestId('organization-members')).toContainText('สมาชิกแรก');
			await expect(page.locator('main').getByRole('heading', { level: 1 })).toHaveCount(1);
			await expect(page.getByRole('button', { name: /^รีเฟรช/ })).toHaveCount(0);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: info.outputPath('detail.png'),
				fullPage: true,
				animations: 'disabled'
			});
			await page
				.getByTestId('organization-unit')
				.getByRole('button', { name: 'แก้ไขหน่วยงาน', exact: true })
				.click();
			const form = page.getByRole('dialog');
			await expect(form.getByLabel('ชื่อหน่วยงาน (ไทย)')).toHaveValue('หน่วยงานแรก');
			await expect(form.getByText('โรงเรียน', { exact: true })).toBeVisible();
			await expect(form).not.toContainText('SCHOOL');
			expect(await form.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
				true
			);
			await page.screenshot({ path: info.outputPath('edit.png'), animations: 'disabled' });
			await page.keyboard.press('Escape');
			await page
				.getByTestId('organization-unit')
				.getByRole('button', { name: 'เพิ่มหน่วยงานย่อย', exact: true })
				.click();
			await expect(form.getByRole('radio', { name: 'ฝ่าย/งาน', exact: true })).toBeChecked();
			await form.getByRole('button', { name: 'ข้อมูลเพิ่มเติม', exact: true }).click();
			await expect(form.getByLabel('ชื่อหน่วยงาน (อังกฤษ)')).toBeVisible();
			await expect(
				form.getByRole('button', { name: 'สร้างหน่วยงาน', exact: true })
			).toBeInViewport();
			await form.getByLabel('สถานที่ตั้ง', { exact: true }).scrollIntoViewIfNeeded();
			const fieldBox = await form.getByLabel('สถานที่ตั้ง', { exact: true }).boundingBox();
			const footerBox = await form.locator('[data-slot="dialog-footer"]').boundingBox();
			if (!fieldBox || !footerBox) throw new Error('Expected visible form footer and input');
			expect(fieldBox.y + fieldBox.height).toBeLessThanOrEqual(footerBox.y);
			await page.screenshot({ path: info.outputPath('create.png'), animations: 'disabled' });
			await page.keyboard.press('Escape');
			await page
				.getByTestId('organization-unit')
				.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
				.click();
			const dialog = page.getByRole('dialog');
			await expect(
				dialog.getByRole('heading', { name: 'ข้อมูลบุคลากร', exact: true })
			).toBeVisible();
			if (width < 768) {
				await dialog.getByLabel('ตำแหน่งที่ต้องการกำหนด', { exact: true }).click();
				await page.getByRole('option', { name: 'สมาชิก', exact: true }).click();
				await expect(
					dialog.getByRole('checkbox', {
						name: `${hidden.name} · ทั้งโรงเรียน · สมาชิก`,
						exact: true
					})
				).toBeChecked();
				await expect(
					dialog.getByRole('checkbox', {
						name: `${own.name} · ทั้งโรงเรียน · หัวหน้า`,
						exact: true
					})
				).not.toBeVisible();
			} else {
				await dialog.evaluate(async (element) => {
					await Promise.all(element.getAnimations().map((animation) => animation.finished));
				});
				const memberBox = await dialog
					.getByRole('checkbox', { name: `${own.name} · ทั้งโรงเรียน · สมาชิก`, exact: true })
					.boundingBox();
				const headerBox = await dialog
					.getByRole('checkbox', {
						name: 'เลือกสิทธิ์ที่แสดงในหมวดข้อมูลบุคลากร · สมาชิก',
						exact: true
					})
					.boundingBox();
				const dialogBox = await dialog.boundingBox();
				if (!memberBox || !headerBox || !dialogBox)
					throw new Error('Expected visible member column');
				expect(memberBox.x + memberBox.width).toBeLessThan(dialogBox.x + dialogBox.width - 16);
				expect(
					Math.abs(memberBox.x + memberBox.width / 2 - headerBox.x - headerBox.width / 2)
				).toBeLessThan(1);
			}
			expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
				true
			);
			await expect(
				dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true })
			).toBeInViewport();
			await page.screenshot({ path: info.outputPath('permissions.png'), animations: 'disabled' });
		});
	}

test.describe('touch permission selection', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('switching a position and searching retains the other positions when saving by touch', async ({
		page
	}) => {
		const state = await fixture(page);
		const dialog = await openPermissions(page);
		await dialog.getByLabel('ตำแหน่งที่ต้องการกำหนด', { exact: true }).tap();
		await page.getByRole('option', { name: 'สมาชิก', exact: true }).tap();
		await dialog.getByLabel('ค้นหาสิทธิ์', { exact: true }).fill('ข้อมูลบุคลากร');
		await dialog
			.getByRole('checkbox', { name: `${own.name} · ทั้งโรงเรียน · สมาชิก`, exact: true })
			.tap();
		await dialog.getByRole('button', { name: 'บันทึกสิทธิ์', exact: true }).tap();
		await expect(dialog).not.toBeVisible();
		expect(state.writes).toHaveLength(1);
		expect(state.writes[0].grants).toEqual(
			expect.arrayContaining([
				...originalGrants,
				{ permission_id: own.id, position_code: 'member' }
			])
		);
		expect(state.writes[0].grants).toHaveLength(3);
	});
});

test('permissions with the same Thai name remain distinguishable by scope', async ({ page }) => {
	await fixture(page);
	const name = 'ดูพฤติกรรมและคะแนนนักเรียน';
	const narrow = {
		...permission(5, 'behavior', 'read', name),
		code: 'behavior.read.own',
		scope: 'own'
	};
	const school = permission(6, 'behavior', 'read', name);
	await page.route('**/api/permissions/modules', (route) =>
		route.fulfill({ json: { success: true, data: { behavior: [narrow, school] } } })
	);
	await page.goto(unitPath());
	await page
		.getByTestId('organization-unit')
		.getByRole('button', { name: 'สิทธิ์ตามตำแหน่ง', exact: true })
		.click();
	const dialog = page.getByRole('dialog');
	const ownCheckbox = dialog.getByRole('checkbox', {
		name: `${name} · เฉพาะตนเอง · หัวหน้า`,
		exact: true
	});
	const schoolCheckbox = dialog.getByRole('checkbox', {
		name: `${name} · ทั้งโรงเรียน · หัวหน้า`,
		exact: true
	});
	await ownCheckbox.focus();
	await page.keyboard.press('Space');
	await expect(ownCheckbox).toBeChecked();
	await expect(schoolCheckbox).not.toBeChecked();
});
