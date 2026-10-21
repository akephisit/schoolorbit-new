import { test, expect } from '@playwright/test';
import { mockBudget } from './fixtures/budget';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';

test.use({ serviceWorkers: 'block' });
test('adding a budget routing step keeps a usable editor without page errors', async ({
	page
}, testInfo) => {
	const errors: string[] = [];
	page.on('console', (event) => {
		if (
			event.type() === 'error' &&
			/svelte|props_invalid_value|Cannot do bind|TypeError|ReferenceError/i.test(event.text())
		)
			errors.push(event.text());
	});
	page.on('pageerror', (error) => {
		errors.push(error.message);
		console.log('budget settings pageerror:', error.message);
	});
	const api = await mockBudget(page, { permissions: [PERMISSIONS.BUDGET_MANAGE_SCHOOL] });
	await page.goto('/staff/budget/settings');
	await page.getByRole('tab', { name: 'ขั้นตอนพิจารณา', exact: true }).click();
	const plan = page
		.locator('[data-slot="card"]')
		.filter({ has: page.getByRole('heading', { name: 'พิจารณาแผนโครงการ', exact: true }) });
	await plan.getByRole('button', { name: 'เพิ่มขั้นตอน', exact: true }).click();
	try {
		await expect(plan.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true })).toBeVisible();
		expect(errors).toEqual([]);
		expect(api.lookupReads).toHaveLength(0);
		await plan.getByLabel('ชื่อขั้นตอนที่ 1 *', { exact: true }).fill('อนุมัติงบประมาณ');
		await plan.getByRole('combobox', { name: 'เพิ่มผู้พิจารณาขั้นตอนที่ 1', exact: true }).click();
		await page.getByRole('option', { name: 'ผู้พิจารณาทดสอบ', exact: true }).click();
		await expect(plan.getByRole('listitem')).toContainText('ผู้พิจารณาทดสอบ');
		await plan.getByRole('combobox', { name: 'เพิ่มผู้พิจารณาขั้นตอนที่ 1', exact: true }).click();
		await page.getByRole('option', { name: 'หน่วยงาน: กลุ่มสาระคณิตศาสตร์', exact: true }).click();
		await expect(plan.getByRole('listitem')).toHaveCount(2);
		expect(api.writes).toHaveLength(0);
		await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
		await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
		expect(api.writes).toHaveLength(1);
		expect(api.writes[0].payload.expectedRevision).toBe(1);
		expect(errors).toEqual([]);
	} finally {
		await testInfo.attach('page-errors', {
			body: JSON.stringify(errors),
			contentType: 'application/json'
		});
		await page.screenshot({
			path: '/tmp/budget-settings-baseline-add-step.png',
			fullPage: true,
			animations: 'disabled'
		});
	}
});
