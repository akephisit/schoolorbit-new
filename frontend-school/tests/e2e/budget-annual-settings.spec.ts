import { test, expect, type Page } from '@playwright/test';
import {
	mockBudget,
	budgetYearSettings,
	budgetYearId,
	budgetFundId,
	budgetOrgId,
	budgetProjectId
} from './fixtures/budget';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';
import { id } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block' });
async function choose(page: Page, label: string, option: string) {
	await page.getByRole('combobox', { name: label, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}
function configured() {
	const year = budgetYearSettings();
	year.authorizedFunding = year.funds.map((fund) => ({
		fundId: fund.id,
		carryoverSatang: '100000',
		annualFundingSatang: '900000',
		reserveSatang: '100000'
	}));
	year.sourceBalances = year.funds.map((fund) => ({
		fundId: fund.id,
		carryoverSatang: '100000',
		annualFundingSatang: '900000',
		reserveSatang: '100000',
		authorizedSatang: '1000000',
		allocatedSatang: '500000',
		unallocatedSatang: '400000',
		committedSatang: '250000',
		remainingSatang: '750000',
		receivedSatang: null
	}));
	year.unitAllocations = [
		{ organizationUnitId: budgetOrgId, fundId: budgetFundId, allocatedSatang: '500000' }
	];
	year.unitBalances = [
		{
			organizationUnitId: budgetOrgId,
			fundId: budgetFundId,
			allocatedSatang: '500000',
			committedSatang: '250000',
			remainingSatang: '250000'
		}
	];
	return year;
}

test('year selection isolates unsaved drafts and returning restores edits without saving', async ({
	page
}) => {
	const first = budgetYearSettings(),
		second = budgetYearSettings();
	second.fiscalYear = {
		...second.fiscalYear,
		id: id(999),
		name: '2571',
		startDate: '2027-10-01',
		endDate: '2028-09-30'
	};
	second.funds[0].name = 'แหล่งเงินปีถัดไป';
	const api = await mockBudget(page, {
		permissions: [PERMISSIONS.BUDGET_MANAGE_SCHOOL],
		yearSettings: [first, second]
	});
	await page.goto(`/staff/budget/settings?fiscalYearId=${budgetYearId}`);
	await page.getByRole('tab', { name: 'แหล่งเงิน', exact: true }).click();
	await page.getByLabel('ชื่อแหล่งเงิน *', { exact: true }).first().fill('ฉบับร่างปีแรก');
	await choose(page, 'ปีงบประมาณ', '2571');
	await page.getByRole('tab', { name: 'แหล่งเงิน', exact: true }).click();
	await expect(page.getByLabel('ชื่อแหล่งเงิน *', { exact: true }).first()).toHaveValue(
		'แหล่งเงินปีถัดไป'
	);
	await page.getByLabel('ชื่อแหล่งเงิน *', { exact: true }).first().fill('ฉบับร่างปีถัดไป');
	await choose(page, 'ปีงบประมาณ', '2570');
	await page.getByRole('tab', { name: 'แหล่งเงิน', exact: true }).click();
	await expect(page.getByLabel('ชื่อแหล่งเงิน *', { exact: true }).first()).toHaveValue(
		'ฉบับร่างปีแรก'
	);
	await choose(page, 'ปีงบประมาณ', '2571');
	await page.getByRole('tab', { name: 'แหล่งเงิน', exact: true }).click();
	await expect(page.getByLabel('ชื่อแหล่งเงิน *', { exact: true }).first()).toHaveValue(
		'ฉบับร่างปีถัดไป'
	);
	expect(api.writes).toHaveLength(0);
});

test('finance-only records actual receipts independently and preserves unknown versus confirmed zero', async ({
	page
}) => {
	const api = await mockBudget(page, {
		permissions: [PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL],
		settings: configured()
	});
	await page.goto('/staff/budget/settings');
	await expect(page.getByRole('tab', { name: 'รับเงินจริง', exact: true })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'ขั้นตอนพิจารณา', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เพิ่มปีงบประมาณ', exact: true })).toHaveCount(0);
	await page.getByLabel('เงินอุดหนุน (บาท)', { exact: true }).fill('0');
	await page.getByRole('button', { name: 'บันทึกยอดรับเงินจริง', exact: true }).click();
	await expect(page.getByText('บันทึกยอดรับเงินจริงแล้ว', { exact: true })).toBeVisible();
	expect(api.writes[0].payload.funds).toEqual([
		{ fundId: budgetFundId, receivedSatang: '0' },
		{ fundId: configured().funds[1].id, receivedSatang: null }
	]);
	expect(api.writes[0].path).toBe(`/api/budget/fiscal-years/${budgetYearId}/funding`);
});

for (const width of [390, 1440])
	test(`annual source authority, reserve and unit allocations stay readable at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 950 });
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		const api = await mockBudget(page, { settings: configured() });
		await page.goto('/staff/budget/settings');
		await expect(
			page.getByRole('region', { name: 'กรอบงบประมาณประจำปี', exact: true })
		).toContainText('20,000.00');
		await page.getByLabel('เงินยกมา (บาท)', { exact: true }).first().fill('1250.25');
		await page.screenshot({
			path: testInfo.outputPath(`annual-funding-${width}.png`),
			fullPage: true
		});
		await page.getByRole('tab', { name: 'จัดสรรหน่วยงาน', exact: true }).click();
		await expect(page.getByRole('combobox', { name: 'หน่วยงาน', exact: true })).toContainText(
			'กลุ่มสาระคณิตศาสตร์'
		);
		await page.getByLabel('วงเงิน (บาท)', { exact: true }).fill('5500.25');
		await page.screenshot({
			path: testInfo.outputPath(`annual-allocation-${width}.png`),
			fullPage: true
		});
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
		await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
		expect(api.writes[0].payload.authorizedFunding).toContainEqual({
			fundId: budgetFundId,
			carryoverSatang: '125025',
			annualFundingSatang: '900000',
			reserveSatang: '100000'
		});
		expect(api.writes[0].payload.unitAllocations).toEqual([
			{ organizationUnitId: budgetOrgId, fundId: budgetFundId, allocatedSatang: '550025' }
		]);
		expect(errors).toEqual([]);
	});

test('used fiscal dates are locked and close-year requires the selected name then freezes edits', async ({
	page
}) => {
	const settings = configured();
	settings.fiscalYear.dateLocked = true;
	settings.fiscalYear.used = true;
	const api = await mockBudget(page, { settings });
	await page.goto('/staff/budget/settings');
	await page.getByRole('tab', { name: 'ปีงบประมาณ', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'วันที่เริ่มต้นปีงบประมาณ', exact: true })
	).toBeDisabled();
	await page.getByRole('button', { name: 'ปิดปีงบประมาณ', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ยืนยันปิดปีงบประมาณ', exact: true })
	).toBeDisabled();
	await page.getByLabel('พิมพ์ชื่อปี 2570 เพื่อยืนยัน').fill('2570');
	await page.getByRole('button', { name: 'ยืนยันปิดปีงบประมาณ', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ปิดปีงบประมาณแล้ว', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true })).toHaveCount(0);
	await page.getByRole('tab', { name: 'กรอบเงินและรับเงิน', exact: true }).click();
	await expect(page.getByLabel('เงินยกมา (บาท)', { exact: true }).first()).toBeDisabled();
	await expect(page.getByRole('button', { name: 'บันทึกยอดรับเงินจริง', exact: true })).toHaveCount(
		0
	);
	expect(api.writes).toHaveLength(1);
});

for (const status of ['submitted', 'returned', 'rejected'] as const)
	test(`projects page permanently deletes a never-approved ${status} project with scoped confirmation`, async ({
		page
	}, testInfo) => {
		const api = await mockBudget(page, {
			approved: false,
			projectStatus: status,
			permissions: [PERMISSIONS.BUDGET_READ_OWN, PERMISSIONS.BUDGET_DELETE_OWN]
		});
		await page.goto('/staff/budget/projects');
		await page.getByRole('button', { name: 'ลบโครงการ', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog).toContainText('3 กิจกรรม');
		await expect(dialog).toContainText('6 รายการประวัติ');
		await expect(
			dialog.getByRole('button', { name: 'ยืนยันลบโครงการถาวร', exact: true })
		).toBeDisabled();
		await page.screenshot({
			path: testInfo.outputPath(`project-delete-${status}.png`),
			fullPage: true
		});
		await dialog.getByLabel('พิมพ์ชื่อโครงการเพื่อยืนยัน').fill('โครงการคณิตศาสตร์');
		await dialog.getByRole('button', { name: 'ยืนยันลบโครงการถาวร', exact: true }).click();
		await expect(page.getByTestId('budget-projects')).not.toContainText('โครงการคณิตศาสตร์');
		expect(api.writes).toHaveLength(1);
		expect(api.writes[0].path).toBe(`/api/budget/projects/${budgetProjectId}`);
	});

test('year deletion names draft project and complete history scope before confirmation', async ({
	page
}, testInfo) => {
	const settings = budgetYearSettings();
	settings.fiscalYear.deletionImpact = {
		projects: 2,
		activities: 4,
		planLines: 7,
		attempts: 3,
		workflowInstances: 3,
		historyEntries: 11
	};
	const api = await mockBudget(page, { settings });
	await page.goto('/staff/budget/settings');
	await page.getByRole('button', { name: 'ลบปีงบประมาณ', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('2 โครงการ');
	await expect(page.getByRole('dialog')).toContainText('11 รายการประวัติ');
	await page.screenshot({ path: testInfo.outputPath('year-delete-scope.png'), fullPage: true });
	await page.getByLabel('พิมพ์ชื่อปี 2570 เพื่อยืนยัน').fill('2570');
	await page.getByRole('button', { name: 'ยืนยันลบปีงบประมาณถาวร', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'ยังไม่มีปีงบประมาณ', exact: true })
	).toBeVisible();
	expect(api.writes).toHaveLength(1);
});

test('new fiscal year copies configurations while requiring fresh authority and actual receipts', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2027-10-20T08:00:00Z'));
	const source = configured();
	source.sourceBalances[0].receivedSatang = '750000';
	const api = await mockBudget(page, { settings: source });
	await page.goto('/staff/budget/settings');
	await page.getByRole('button', { name: 'เพิ่มปีงบประมาณ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อปีงบประมาณ *', { exact: true }).fill('2571');
	for (const label of ['วันที่เริ่มต้นปีงบประมาณใหม่', 'วันที่สิ้นสุดปีงบประมาณใหม่']) {
		const trigger = dialog.getByRole('button', { name: label, exact: true });
		await trigger.click();
		const contentId = await trigger.getAttribute('aria-controls');
		await page
			.locator(
				`[id="${contentId}"][data-state="open"] [data-calendar-day]:not([data-outside-month])`
			)
			.filter({ hasText: /^20$/ })
			.click();
	}
	await choose(page, 'คัดลอกการตั้งค่าจากปี', '2570');
	await dialog.getByRole('button', { name: 'เพิ่มปีงบประมาณ', exact: true }).click();
	await expect(page.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true })).toContainText(
		'2571'
	);
	await expect(page.getByRole('button', { name: 'กำหนดกรอบงบประมาณ', exact: true })).toBeVisible();
	await expect(page.getByLabel('เงินอุดหนุน (บาท)', { exact: true })).toHaveValue('');
	expect(api.writes[0].payload.copyFromFiscalYearId).toBe(budgetYearId);
	expect(api.writes[0].payload).not.toHaveProperty('authorizedFunding');
	expect(api.writes[0].payload).not.toHaveProperty('receivedSatang');
});

test('blocked year closure stays discoverable without allowing a write', async ({ page }) => {
	const settings = configured();
	settings.fiscalYear.canClose = false;
	const api = await mockBudget(page, { settings });
	await page.goto('/staff/budget/settings');
	await expect(page.getByRole('button', { name: 'ปิดปีงบประมาณ', exact: true })).toBeDisabled();
	await expect(
		page.getByText('ปิดปีได้เมื่อไม่มีรายการรอพิจารณา ยอดผูกพัน หรือเงินยืมค้างคืน', {
			exact: true
		})
	).toBeVisible();
	expect(api.writes).toHaveLength(0);
});
