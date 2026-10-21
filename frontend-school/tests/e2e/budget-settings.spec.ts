import { test, expect, type Locator, type Page } from '@playwright/test';
import { mockBudget, budgetCatalogs, budgetOrgId } from './fixtures/budget';
import { id, actor } from './fixtures/staff-home-route-data';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';
import type { BudgetSettings } from '../../src/lib/api/budget';
import type { StaffLookupItem, OrganizationUnitLookupItem } from '../../src/lib/api/lookup';

test.use({ serviceWorkers: 'block' });
const manageOnly = [PERMISSIONS.BUDGET_MANAGE_SCHOOL];
const staff: StaffLookupItem[] = Array.from({ length: 120 }, (_, index) => ({
	id: id(1500 + index),
	name: `บุคลากร ${String(index + 1).padStart(3, '0')}`
}));
function runtimeErrors(page: Page) {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('console', (event) => {
		if (
			event.type() === 'error' &&
			/svelte|props_invalid_value|Cannot do bind|TypeError|ReferenceError/i.test(event.text())
		)
			errors.push(event.text());
	});
	return errors;
}
function planEditor(page: Page) {
	return page
		.locator('[data-slot="card"]')
		.filter({ has: page.getByRole('heading', { name: 'พิจารณาแผนโครงการ', exact: true }) });
}
async function openRouting(page: Page) {
	await page.goto('/staff/budget/settings');
	await page.getByRole('tab', { name: 'ขั้นตอนพิจารณา', exact: true }).click();
	return planEditor(page);
}
async function openPicker(
	page: Page,
	step: Locator,
	number: number,
	mode: 'pointer' | 'keyboard' | 'touch' = 'pointer'
) {
	const label = `เพิ่มผู้พิจารณาขั้นตอนที่ ${number}`;
	const trigger = step.getByRole('combobox', { name: label, exact: true });
	if (mode === 'keyboard') {
		await trigger.focus();
		await page.keyboard.press('Enter');
	} else if (mode === 'touch') await trigger.tap();
	else await trigger.click();
	await expect(trigger).toHaveAttribute('aria-expanded', 'true');
	const contentId = await trigger.getAttribute('aria-controls');
	expect(contentId).toBeTruthy();
	const popup = page.locator(`[id="${contentId}"][data-state="open"]`);
	await expect(popup).toBeVisible();
	await expect(
		popup.getByRole('combobox', { name: `ค้นหา${label}`, exact: true })
	).toHaveAccessibleName(`ค้นหา${label}`);
	return {
		trigger,
		popup,
		search: popup.getByLabel(`ค้นหา${label}`, { exact: true })
	};
}
async function chooseField(page: Page, scope: Locator, label: string, value: string) {
	const trigger = scope.getByRole('combobox', { name: label, exact: true });
	await trigger.click();
	const contentId = await trigger.getAttribute('aria-controls');
	await page
		.locator(`[id="${contentId}"][data-state="open"]`)
		.getByRole('option', { name: value, exact: true })
		.click();
}
async function assertPageFits(page: Page) {
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
		true
	);
}
async function screenshot(page: Page, name: string) {
	await assertPageFits(page);
	await page.screenshot({
		path: `/tmp/budget-settings-${name}.png`,
		fullPage: true,
		animations: 'disabled'
	});
}

test('adding a budget routing step keeps a usable editor without page or Svelte console errors', async ({
	page
}) => {
	const errors = runtimeErrors(page);
	const api = await mockBudget(page, { permissions: manageOnly });
	const plan = await openRouting(page);
	await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
	const step = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await expect(step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true })).toBeVisible();
	expect(api.lookupReads).toHaveLength(0);
	await step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('อนุมัติงบประมาณ');
	let picker = await openPicker(page, step, 1);
	await picker.popup.getByRole('option', { name: 'ผู้พิจารณาทดสอบ', exact: true }).click();
	await expect(step.getByRole('listitem')).toContainText('ผู้พิจารณาทดสอบ');
	picker = await openPicker(page, step, 1);
	await picker.popup
		.getByRole('option', { name: 'หน่วยงาน: กลุ่มสาระคณิตศาสตร์', exact: true })
		.click();
	await expect(step.getByRole('listitem')).toHaveCount(2);
	expect(api.writes).toHaveLength(0);
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(1);
	expect(api.writes[0].payload.expectedRevision).toBe(1);
	expect(errors).toEqual([]);
	await screenshot(page, 'add-step-regression');
});

for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 1528, height: 1000 }
])
	for (const theme of ['light', 'dark'] as const)
		test.describe(`settings ${viewport.width}px ${theme}`, () => {
			test.use({ viewport, hasTouch: viewport.width === 390 });
			test('saved and new controls align, wrap on mobile, and picker adds directly by touch or keyboard', async ({
				page
			}) => {
				const errors = runtimeErrors(page);
				await page.addInitScript(
					(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
					theme
				);
				const api = await mockBudget(page, { permissions: manageOnly });
				await page.goto('/staff/budget/settings');
				for (const tab of ['แหล่งเงิน', 'หมวดรายจ่าย']) {
					await page.getByRole('tab', { name: tab, exact: true }).click();
					await page.getByRole('button', { name: `เพิ่ม${tab}`, exact: true }).click();
					const inputs = page.getByLabel(`ชื่อ${tab} *`, { exact: true });
					const count = await inputs.count();
					const saved = inputs.nth(0),
						added = inputs.nth(count - 1);
					await added.fill(`รายการใหม่ ${tab}`);
					const savedId = await saved.getAttribute('id'),
						newId = await added.getAttribute('id');
					const savedRow = page
						.locator('[data-slot="card"]')
						.filter({ has: page.locator(`[id="${savedId}"]`) });
					const newRow = page
						.locator('[data-slot="card"]')
						.filter({ has: page.locator(`[id="${newId}"]`) });
					const savedInput = (await saved.boundingBox())!,
						newInput = (await added.boundingBox())!;
					const savedCheck = (await savedRow.getByRole('checkbox').boundingBox())!,
						newCheck = (await newRow.getByRole('checkbox').boundingBox())!;
					const remove = (await newRow
						.getByRole('button', { name: 'นำออก', exact: true })
						.boundingBox())!;
					expect(Math.abs(savedInput.x - newInput.x)).toBeLessThan(1);
					expect(
						Math.abs(savedInput.x + savedInput.width - newInput.x - newInput.width)
					).toBeLessThan(1);
					expect(Math.abs(savedCheck.x - newCheck.x)).toBeLessThan(1);
					expect(
						Math.abs(newCheck.y + newCheck.height / 2 - remove.y - remove.height / 2)
					).toBeLessThan(1);
					if (viewport.width > 390) {
						expect(
							Math.abs(newInput.y + newInput.height / 2 - newCheck.y - newCheck.height / 2)
						).toBeLessThan(1);
						expect(
							Math.abs(savedInput.y + savedInput.height / 2 - savedCheck.y - savedCheck.height / 2)
						).toBeLessThan(1);
					} else expect(newCheck.y).toBeGreaterThanOrEqual(newInput.y + newInput.height);
					await screenshot(
						page,
						`${viewport.width}-${theme}-${tab === 'แหล่งเงิน' ? 'funds' : 'categories'}`
					);
				}
				await page.getByRole('tab', { name: 'ปีงบประมาณ', exact: true }).click();
				const year = page
					.locator('[data-slot="card"]')
					.filter({ has: page.getByLabel('ชื่อปีงบประมาณ *', { exact: true }) });
				if (viewport.width > 390) {
					const input = (await year.getByLabel('ชื่อปีงบประมาณ *', { exact: true }).boundingBox())!;
					const check = (await year.getByRole('checkbox').boundingBox())!;
					expect(Math.abs(input.y + input.height / 2 - check.y - check.height / 2)).toBeLessThan(1);
				}
				await page.getByRole('tab', { name: 'ขั้นตอนพิจารณา', exact: true }).click();
				const plan = planEditor(page);
				await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
				const step = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
				await step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('อนุมัติแผนโครงการ');
				const picker = await openPicker(
					page,
					step,
					1,
					viewport.width === 390 ? 'touch' : 'keyboard'
				);
				await picker.search.fill('ผู้พิจารณาทดสอบ');
				const option = picker.popup.getByRole('option', { name: 'ผู้พิจารณาทดสอบ', exact: true });
				await expect(option).toBeVisible();
				const popupBox = (await picker.popup.boundingBox())!;
				expect(popupBox.x).toBeGreaterThanOrEqual(0);
				expect(popupBox.x + popupBox.width).toBeLessThanOrEqual(viewport.width);
				expect(popupBox.height).toBeLessThan(viewport.height);
				await screenshot(page, `${viewport.width}-${theme}-picker`);
				if (viewport.width === 390) await option.tap();
				else {
					await picker.search.press('ArrowDown');
					await picker.search.press('Enter');
				}
				await expect(picker.trigger).toHaveAttribute('aria-expanded', 'false');
				await expect(step.getByRole('listitem')).toContainText('ผู้พิจารณาทดสอบ');
				await screenshot(page, `${viewport.width}-${theme}-routing`);
				expect(api.writes).toHaveLength(0);
				expect(errors).toEqual([]);
			});
		});

test('remote search refines beyond the initial page, deduplicates per step, and keeps remaining identities after deleting a preceding step', async ({
	page
}) => {
	const errors = runtimeErrors(page);
	const api = await mockBudget(page, { permissions: manageOnly, staffOptions: staff });
	const plan = await openRouting(page);
	await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
	const first = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await first.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('ขั้นตอนที่จะลบ');
	let picker = await openPicker(page, first, 1);
	await expect(picker.popup.getByRole('option', { name: 'บุคลากร 111', exact: true })).toHaveCount(
		0
	);
	await picker.search.fill('บุคลากร 111');
	await picker.popup.getByRole('option', { name: 'บุคลากร 111', exact: true }).click();
	picker = await openPicker(page, first, 1);
	await picker.search.fill('บุคลากร 111');
	await expect(
		picker.popup.getByRole('option', { name: /^บุคลากร 111\s*เลือกแล้ว$/ })
	).toHaveAttribute('aria-disabled', 'true');
	await picker.search.press('Escape');
	await expect(first.getByRole('listitem')).toHaveCount(1);
	await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
	const second = plan.getByRole('region', { name: 'ขั้นตอนที่ 2', exact: true });
	await second.getByLabel('ชื่อขั้นตอนที่ 2 *', { exact: true }).fill('อนุมัติขั้นสุดท้าย');
	picker = await openPicker(page, second, 2);
	await picker.popup
		.getByRole('option', { name: 'หน่วยงาน: กลุ่มสาระคณิตศาสตร์', exact: true })
		.click();
	await first.getByRole('button', { name: 'ลบขั้นตอน', exact: true }).click();
	const remaining = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await expect(remaining.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true })).toHaveValue(
		'อนุมัติขั้นสุดท้าย'
	);
	await expect(remaining.getByRole('listitem')).toContainText('หน่วยงาน: กลุ่มสาระคณิตศาสตร์');
	picker = await openPicker(page, remaining, 1);
	await picker.search.fill('ไม่มีชื่อที่ตรงกัน');
	await expect(picker.popup).toContainText('ไม่พบผู้พิจารณาที่ตรงกับคำค้น');
	await picker.search.press('Escape');
	await expect(remaining.getByRole('listitem')).toContainText('หน่วยงาน: กลุ่มสาระคณิตศาสตร์');
	await remaining.getByRole('button', { name: 'นำออก', exact: true }).click();
	await expect(remaining.getByRole('listitem')).toHaveCount(0);
	expect(
		api.lookupReads.some(
			(url) => new URL(url, 'http://fixture').searchParams.get('search') === 'บุคลากร 111'
		)
	).toBe(true);
	expect(
		api.lookupReads.every(
			(url) => Number(new URL(url, 'http://fixture').searchParams.get('limit')) <= 100
		)
	).toBe(true);
	expect(api.writes).toHaveLength(0);
	expect(errors).toEqual([]);
});

test('failed search retries and superseded searches cannot replace current results or selections', async ({
	page
}) => {
	const errors = runtimeErrors(page);
	const api = await mockBudget(page, {
		permissions: manageOnly,
		staffOptions: [
			{ id: id(1700), name: 'ค้นหาช้า' },
			{ id: id(1701), name: 'ค้นหาเร็ว' }
		],
		failLookup: { kind: 'staff' },
		holdLookup: { kind: 'staff', search: 'ช้า' }
	});
	const plan = await openRouting(page);
	await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
	const step = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	const picker = await openPicker(page, step, 1);
	await expect(picker.popup).toContainText('บริการค้นหาผู้พิจารณายังไม่พร้อม');
	await picker.popup.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(picker.popup.getByRole('option', { name: 'ค้นหาเร็ว', exact: true })).toBeVisible();
	await picker.search.fill('ช้า');
	await expect
		.poll(() =>
			api.lookupReads.some(
				(url) => new URL(url, 'http://fixture').searchParams.get('search') === 'ช้า'
			)
		)
		.toBe(true);
	await picker.search.fill('เร็ว');
	await expect(picker.popup.getByRole('option', { name: 'ค้นหาเร็ว', exact: true })).toBeVisible();
	api.release();
	await expect(picker.popup.getByRole('option', { name: 'ค้นหาช้า', exact: true })).toHaveCount(0);
	await picker.popup.getByRole('option', { name: 'ค้นหาเร็ว', exact: true }).click();
	await expect(step.getByRole('listitem')).toHaveCount(1);
	await expect(step.getByRole('listitem')).toContainText('ค้นหาเร็ว');
	expect(api.writes).toHaveLength(0);
	expect(errors).toEqual([]);
});

test('failed save preserves names, recipients and policy; retry and reload use the current settings revision', async ({
	page
}) => {
	const errors = runtimeErrors(page);
	const api = await mockBudget(page, {
		permissions: manageOnly,
		staffOptions: staff,
		failMutation: true
	});
	const plan = await openRouting(page);
	await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
	const step = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('อนุมัติโครงการ');
	await chooseField(page, step, 'การดำเนินการ', 'ตรวจสอบ');
	await chooseField(page, step, 'การดำเนินการ', 'อนุมัติ');
	await chooseField(page, step, 'เงื่อนไขผู้พิจารณา', 'คนใดคนหนึ่งดำเนินการ');
	await step.getByRole('checkbox', { name: 'ต้องระบุความคิดเห็น', exact: true }).check();
	const picker = await openPicker(page, step, 1);
	await picker.search.fill('บุคลากร 071');
	await picker.popup.getByRole('option', { name: 'บุคลากร 071', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกไม่สำเร็จชั่วคราว', { exact: true })).toBeVisible();
	await expect(step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true })).toHaveValue(
		'อนุมัติโครงการ'
	);
	await expect(step.getByRole('listitem')).toContainText('บุคลากร 071');
	await expect(
		step.getByRole('combobox', { name: 'เงื่อนไขผู้พิจารณา', exact: true })
	).toContainText('คนใดคนหนึ่งดำเนินการ');
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(2);
	expect(api.writes[0].payload.requestId).toBe(api.writes[1].payload.requestId);
	expect(api.writes[1].payload.planSteps).toEqual([
		{
			title: 'อนุมัติโครงการ',
			kind: 'approve',
			completionPolicy: 'any',
			recipients: [{ kind: 'user', userId: staff[70].id }],
			requiresComment: true,
			requiresSignature: false,
			requiresEvidence: false,
			requiresDelegatedAuthority: false,
			dueAfterDays: null
		}
	]);
	await page.reload();
	await page.getByRole('tab', { name: 'ขั้นตอนพิจารณา', exact: true }).click();
	const saved = planEditor(page).getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await expect(saved.getByRole('listitem')).toContainText('บุคลากร 071');
	await expect(
		saved.getByRole('checkbox', { name: 'ต้องระบุความคิดเห็น', exact: true })
	).toBeChecked();
	await saved.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('อนุมัติโครงการฉบับปรับปรุง');
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(3);
	expect(api.writes[2].payload.expectedRevision).toBe(2);
	expect(api.writes[2].payload.requestId).not.toBe(api.writes[1].payload.requestId);
	expect(errors).toEqual([]);
});

test('existing selected labels hydrate while closed, unknown identities and hidden routing fields remain intact', async ({
	page
}) => {
	const original: BudgetSettings = {
		...structuredClone(budgetCatalogs),
		revision: 7,
		requestSteps: [],
		adjustmentSteps: [],
		planSteps: [
			{
				title: 'อนุมัติตามกำหนด',
				kind: 'approve',
				completionPolicy: 'all',
				recipients: [
					{ kind: 'user', userId: actor },
					{ kind: 'organization_unit', organizationUnitId: budgetOrgId },
					{ kind: 'user', userId: id(1999) }
				],
				requiresComment: true,
				requiresSignature: false,
				requiresEvidence: false,
				requiresDelegatedAuthority: false,
				dueAfterDays: 7
			}
		]
	};
	const api = await mockBudget(page, { permissions: manageOnly, settings: original });
	const plan = await openRouting(page);
	await expect(plan.getByRole('listitem').filter({ hasText: 'ผู้พิจารณาทดสอบ' })).toBeVisible();
	await expect(
		plan.getByRole('listitem').filter({ hasText: 'หน่วยงาน: กลุ่มสาระคณิตศาสตร์' })
	).toBeVisible();
	await expect(plan.getByRole('listitem').filter({ hasText: 'ยังไม่พบชื่อ' })).toContainText(
		'บุคลากรเดิมรายการที่ 3'
	);
	expect(api.lookupReads).toHaveLength(2);
	expect(
		api.lookupReads.every(
			(url) => new URL(url, 'http://fixture').searchParams.get('limit') === '100'
		)
	).toBe(true);
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.writes[0].payload.planSteps).toEqual(original.planSteps);
	expect(api.writes[0].payload.expectedRevision).toBe(7);
});

function savedRouting(
	recipients: NonNullable<BudgetSettings['planSteps'][number]['recipients']>
): BudgetSettings {
	return {
		...structuredClone(budgetCatalogs),
		revision: 7,
		requestSteps: [],
		adjustmentSteps: [],
		planSteps: [
			{
				title: 'ผู้พิจารณาที่บันทึกไว้',
				kind: 'approve',
				completionPolicy: 'all',
				recipients,
				requiresComment: true,
				requiresSignature: false,
				requiresEvidence: false,
				requiresDelegatedAuthority: false,
				dueAfterDays: 7
			}
		]
	};
}

test('search resolves selected names beyond initial hydration without changing identities, and late metadata cannot replace the current name', async ({
	page
}) => {
	const errors = runtimeErrors(page);
	const people = structuredClone(staff);
	people[110].name = 'บุคลากรเดิม';
	const units: OrganizationUnitLookupItem[] = Array.from({ length: 120 }, (_, index) => ({
		id: id(1800 + index),
		name: `หน่วยงาน${String(index + 1).padStart(3, '0')}`,
		code: `ORG${index}`,
		display_order: index,
		is_active: true
	}));
	const original = savedRouting([
		{ kind: 'user', userId: people[110].id },
		{ kind: 'organization_unit', organizationUnitId: units[110].id }
	]);
	const api = await mockBudget(page, {
		permissions: manageOnly,
		settings: original,
		staffOptions: people,
		organizationOptions: units,
		holdLookup: { kind: 'staff', search: 'บุคลากรเดิม' }
	});
	const plan = await openRouting(page);
	const step = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await expect(step.getByRole('listitem').filter({ hasText: 'ยังไม่พบชื่อ' })).toHaveCount(2);
	const picker = await openPicker(page, step, 1);
	await picker.search.fill('บุคลากรเดิม');
	await expect
		.poll(() =>
			api.lookupReads.some(
				(url) => new URL(url, 'http://fixture').searchParams.get('search') === 'บุคลากรเดิม'
			)
		)
		.toBe(true);
	people[110].name = 'บุคลากรล่าสุด';
	await picker.search.fill('บุคลากรล่าสุด');
	await expect(
		picker.popup.getByRole('option', { name: /^บุคลากรล่าสุด\s*เลือกแล้ว$/ })
	).toHaveAttribute('aria-disabled', 'true');
	await expect(step.getByRole('listitem').filter({ hasText: 'บุคลากรล่าสุด' })).toBeVisible();
	api.release();
	await expect
		.poll(() =>
			api.lookupCompletions.some(
				(url) => new URL(url, 'http://fixture').searchParams.get('search') === 'บุคลากรเดิม'
			)
		)
		.toBe(true);
	await expect(step.getByRole('listitem').filter({ hasText: 'บุคลากรเดิม' })).toHaveCount(0);
	await expect(step.getByRole('listitem').filter({ hasText: 'บุคลากรล่าสุด' })).toBeVisible();
	await picker.search.fill('หน่วยงาน111');
	await expect(
		picker.popup.getByRole('option', { name: /^หน่วยงาน: หน่วยงาน111\s*เลือกแล้ว$/ })
	).toHaveAttribute('aria-disabled', 'true');
	await expect(
		step.getByRole('listitem').filter({ hasText: 'หน่วยงาน: หน่วยงาน111' })
	).toBeVisible();
	await picker.search.press('Escape');
	await expect(step.getByRole('listitem')).toHaveCount(2);
	expect(api.writes).toHaveLength(0);
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.writes[0].payload.planSteps).toEqual(original.planSteps);
	expect(errors).toEqual([]);
});

test('closing a picker while its search is pending cancels late selected-label updates and retains the draft', async ({
	page
}) => {
	const errors = runtimeErrors(page);
	const people = structuredClone(staff);
	people[110].name = 'บุคลากรรอผล';
	const api = await mockBudget(page, {
		permissions: manageOnly,
		settings: savedRouting([{ kind: 'user', userId: people[110].id }]),
		staffOptions: people,
		holdLookup: { kind: 'staff', search: 'รอผล' }
	});
	const plan = await openRouting(page);
	const step = plan.getByRole('region', { name: 'ขั้นตอนที่ 1', exact: true });
	await expect(step.getByRole('listitem')).toContainText('ยังไม่พบชื่อ');
	await step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('ฉบับร่างยังอยู่');
	const picker = await openPicker(page, step, 1);
	await picker.search.fill('รอผล');
	await expect
		.poll(() =>
			api.lookupReads.some(
				(url) => new URL(url, 'http://fixture').searchParams.get('search') === 'รอผล'
			)
		)
		.toBe(true);
	await picker.search.press('Escape');
	await expect(picker.trigger).toHaveAttribute('aria-expanded', 'false');
	api.release();
	await expect
		.poll(() =>
			api.lookupCompletions.some(
				(url) => new URL(url, 'http://fixture').searchParams.get('search') === 'รอผล'
			)
		)
		.toBe(true);
	await expect(picker.popup).not.toBeVisible();
	await expect(step.getByRole('listitem')).toContainText('ยังไม่พบชื่อ');
	await expect(step.getByRole('listitem')).not.toContainText('บุคลากรรอผล');
	await expect(step.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true })).toHaveValue(
		'ฉบับร่างยังอยู่'
	);
	expect(api.writes).toHaveLength(0);
	expect(errors).toEqual([]);
});
