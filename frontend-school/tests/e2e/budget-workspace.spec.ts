// Intercepted synthetic APIs verify browser transport/state; backend fixtures own authorization and accounting.
import { test, expect, type Page, type Locator } from '@playwright/test';
import {
	mockBudget,
	budgetProjectId,
	budgetRequestId,
	budgetYearId,
	budgetFundId,
	materialsId,
	equipmentId,
	budgetMenuGroups
} from './fixtures/budget';
import { id, actor } from './fixtures/staff-home-route-data';
import type { BudgetDashboard } from '../../src/lib/api/budget';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';
test.use({ serviceWorkers: 'block' });
async function choose(page: Page, label: string, option: string) {
	await page.getByRole('combobox', { name: label, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}
async function date(page: Page, label: string) {
	const trigger = page.getByRole('button', { name: label, exact: true });
	await trigger.click();
	await expect(trigger).toHaveAttribute('aria-expanded', 'true');
	const contentId = await trigger.getAttribute('aria-controls');
	expect(contentId).toBeTruthy();
	const popover = page.locator(`[id="${contentId}"][data-state="open"]`);
	await expect(popover).toBeVisible();
	const day = popover
		.locator('[data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: /^20$/ });
	await expect(day).toHaveCount(1);
	await day.click();
	await expect(trigger).toHaveAttribute('aria-expanded', 'false');
	await expect(trigger).toContainText(/^\s*20\s/);
}

test('reader sees independent regions and exact activity fund/category pivot without action reads', async ({
	page
}) => {
	const api = await mockBudget(page, { permissions: [PERMISSIONS.BUDGET_READ_SCHOOL] });
	await page.goto('/staff/budget/reports');
	await expect(page.getByTestId('budget-pivot')).toContainText('9,928.00');
	await expect(
		page.getByTestId('budget-pivot').getByRole('columnheader', { name: 'เงินอุดหนุน', exact: true })
	).toBeVisible();
	await expect(
		page
			.getByTestId('budget-pivot')
			.getByRole('columnheader', { name: 'เงินพัฒนาผู้เรียน', exact: true })
	).toBeVisible();
	await expect(
		page.getByTestId('budget-pivot').getByRole('row').filter({ hasText: 'กิจกรรมที่หนึ่ง' })
	).toContainText('3,588.00');
	await expect(page.getByRole('button', { name: 'ส่งออก CSV' })).toHaveCount(0);
	expect(
		api.reads.some(
			(path) =>
				path.includes('/settings') || path.includes('supply-catalog') || path.includes('/lookup/')
		)
	).toBe(false);
	await expect(page).not.toHaveURL(/academicYearId|academicTermId/);
});

test('failed project read retries its region and read-only users do not load creator options', async ({
	page
}) => {
	const api = await mockBudget(page, {
		permissions: [PERMISSIONS.BUDGET_READ_SCHOOL],
		fail: 'projects'
	});
	await page.goto('/staff/budget/projects');
	await expect(page.getByTestId('budget-projects')).toContainText('บริการ projects ไม่พร้อม');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('budget-projects')).toContainText('โครงการคณิตศาสตร์');
	await expect(page.getByRole('button', { name: 'สร้างโครงการ', exact: true })).toHaveCount(0);
	expect(api.reads.filter((path) => path.startsWith('/api/budget/projects')).length).toBe(2);
	expect(api.reads.some((path) => path.includes('/catalogs') || path.includes('/settings'))).toBe(
		false
	);
});

test('minimal project to plan approval, activity request, partial payment and close', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-20T08:00:00Z'));
	const api = await mockBudget(page, { approved: false });
	await page.goto('/staff/budget/projects');
	await page.getByRole('button', { name: 'สร้างโครงการ', exact: true }).click();
	await expect(
		page.getByLabel(/รหัสโครงการ|รหัสกิจกรรม|วัตถุประสงค์|หลักการและเหตุผล|ตัวชี้วัด/)
	).toHaveCount(0);
	await page.getByLabel('ชื่อโครงการ *', { exact: true }).fill('โครงการใหม่');
	await choose(page, 'ปีงบประมาณ', '2570');
	await choose(page, 'หน่วยงานที่รับผิดชอบ', 'กลุ่มสาระคณิตศาสตร์');
	await date(page, 'วันที่เริ่มต้น');
	await date(page, 'วันที่สิ้นสุด');
	await page.getByRole('button', { name: 'บันทึกโครงการ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/projects/${budgetProjectId}`));
	await page.getByRole('button', { name: 'แก้ไขกิจกรรมและค่าใช้จ่าย' }).click();
	await page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม *', { exact: true }).fill('กิจกรรมทดลอง');
	await page.getByRole('button', { name: 'เพิ่มรายการกำหนดเอง' }).click();
	await page.getByLabel('ชื่อสินค้า / บริการ *').fill('กระดาษ');
	await page.getByLabel('จำนวน *', { exact: true }).fill('2');
	await page.getByLabel('หน่วยนับ *').fill('ชุด');
	await page.getByLabel('ราคาต่อหน่วย (บาท) *').fill('10.00');
	await choose(page, 'แหล่งเงิน', 'เงินอุดหนุน');
	await choose(page, 'หมวดรายจ่าย', 'วัสดุ');
	await page.getByRole('button', { name: 'บันทึกแผนฉบับร่าง' }).click();
	await expect(page.getByTestId('budget-project-detail')).toContainText('20.00');
	await page.getByRole('button', { name: 'ส่งแผนพิจารณา' }).click();
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toHaveCount(0);
	await api.asReviewer();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	await api.asRequester();
	await page.getByRole('button', { name: 'สร้างคำขอใช้เงิน' }).click();
	await page.getByLabel('ชื่อคำขอใช้เงิน *').fill('ขอซื้อกระดาษ');
	await choose(page, 'กิจกรรม', 'กิจกรรมทดลอง');
	await page.getByRole('checkbox', { name: 'กระดาษ', exact: true }).check();
	await page.getByLabel('จำนวนเงิน (บาท) *', { exact: true }).fill('15.00');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByTestId('budget-request-detail')).toContainText('15.00');
	await page.getByRole('button', { name: 'ส่งคำขอพิจารณา' }).click();
	await api.asReviewer();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	await api.asRequester();
	await page.locator('summary').filter({ hasText: 'ประวัติการดำเนินการ' }).click();
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).click();
	await date(page, 'วันที่จ่ายเงิน');
	await page.getByLabel('เลขที่ใบสำคัญ *').fill('PV-01');
	await page.getByLabel('จำนวนเงินจ่าย กระดาษ', { exact: true }).fill('5.00');
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).last().click();
	await expect(page.getByTestId('budget-payments')).toContainText('PV-01');
	await expect(page.getByTestId('budget-request-detail')).toContainText('10.00');
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).click();
	await date(page, 'วันที่จ่ายเงิน');
	await page.getByLabel('เลขที่ใบสำคัญ *').fill('PV-02');
	await page.getByLabel('จำนวนเงินจ่าย กระดาษ', { exact: true }).fill('3.00');
	await page.getByLabel('แนบหลักฐาน', { exact: true }).setInputFiles({
		name: 'หลักฐานทดสอบ.pdf',
		mimeType: 'application/pdf',
		buffer: Buffer.from('%PDF synthetic fixture')
	});
	await expect(page.getByRole('button', { name: 'นำออก', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).last().click();
	await expect(page.getByTestId('budget-payments')).toContainText('PV-02');
	await expect(page.getByTestId('budget-request-detail')).toContainText('7.00');
	await page.getByRole('button', { name: 'ปิดคำขอ', exact: true }).click();
	await page.getByLabel('เหตุผลการปิดคำขอ *').fill('ดำเนินการครบแล้ว');
	await page.getByRole('button', { name: 'ยืนยันปิดคำขอ' }).click();
	await expect(page.getByTestId('budget-request-detail')).toContainText('ปิดแล้ว');
	await page.getByRole('button', { name: 'บันทึกคืนเงิน', exact: true }).first().click();
	await date(page, 'วันที่คืนเงิน');
	await page.getByLabel('จำนวนเงินคืน กระดาษ', { exact: true }).fill('2.00');
	await page.getByLabel('เหตุผลการคืนเงิน *').fill('คืนส่วนที่ไม่ได้ใช้');
	await page.getByRole('button', { name: 'ยืนยันคืนเงิน', exact: true }).click();
	await expect(page.getByTestId('budget-payments')).toContainText('คืนเงิน 2.00 บาท');
	const history = page
		.locator('details')
		.filter({ has: page.locator('summary').filter({ hasText: 'ประวัติการดำเนินการ' }) });
	await expect(history).toContainText('บันทึกการจ่ายเงิน');
	await expect(history).toContainText('ปิดคำขอ');
	await expect(history).toContainText('คืนเงิน');
	const payment = api.writes.find((write) => write.path.endsWith('/payments'))!;
	expect(payment.payload.allocations).toEqual([
		{ planLineId: api.workspace.activities[0].lines[0].id, amountSatang: '500' }
	]);
	expect(payment.payload.evidenceFileIds).toEqual([]);
	expect(
		api.writes.filter((write) => write.path.endsWith('/payments')).at(-1)!.payload.evidenceFileIds
	).toHaveLength(1);
});

test('failed project mutation preserves all four groups and retry key', async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-20T08:00:00Z'));
	const api = await mockBudget(page, { failMutation: true });
	await page.goto('/staff/budget/projects');
	await page.getByRole('button', { name: 'สร้างโครงการ', exact: true }).click();
	await page.getByLabel('ชื่อโครงการ *', { exact: true }).fill('ชื่อที่ต้องคงอยู่');
	await choose(page, 'ปีงบประมาณ', '2570');
	await choose(page, 'หน่วยงานที่รับผิดชอบ', 'กลุ่มสาระคณิตศาสตร์');
	await date(page, 'วันที่เริ่มต้น');
	await date(page, 'วันที่สิ้นสุด');
	await page.getByRole('button', { name: 'บันทึกโครงการ', exact: true }).click();
	await expect(page.getByTestId('budget-project-form')).toContainText('บันทึกไม่สำเร็จชั่วคราว');
	await expect(page.getByLabel('ชื่อโครงการ *', { exact: true })).toHaveValue('ชื่อที่ต้องคงอยู่');
	await page.getByRole('button', { name: 'บันทึกโครงการ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(budgetProjectId));
	expect(api.writes[0].payload.requestId).toBe(api.writes[1].payload.requestId);
});

for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 1440, height: 1000 }
])
	for (const theme of ['light', 'dark'])
		test(`report fits ${viewport.width}px ${theme} and keeps exact totals`, async ({ page }) => {
			await page.setViewportSize(viewport);
			await page.addInitScript(
				(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
				theme
			);
			await mockBudget(page, {
				permissions: [PERMISSIONS.BUDGET_READ_SCHOOL, PERMISSIONS.BUDGET_DOWNLOAD_SCHOOL]
			});
			await page.goto('/staff/budget/reports');
			await expect(page.getByTestId('budget-pivot')).toContainText('9,928.00');
			if (theme === 'dark') await expect(page.locator('html')).toHaveClass(/dark/);
			else await expect(page.locator('html')).not.toHaveClass(/dark/);
			await page.screenshot({
				path: `/tmp/budget-report-${viewport.width}-${theme}.png`,
				fullPage: true
			});
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
			await page.getByRole('radio', { name: 'จ่ายจริง', exact: true }).click();
			await expect(page).toHaveURL(/metric=paid/);
			await expect(
				page.getByTestId('budget-pivot').getByRole('row').filter({ hasText: 'รวมทั้งหมด' })
			).toContainText('0.00');
		});

for (const [name, permissions, path] of [
	['creator only', [PERMISSIONS.BUDGET_CREATE_OWN], '/staff/budget/projects'],
	['finance only', [PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL], '/staff/budget/requests']
] as const)
	test(`${name} gets missing-read guidance without primary reads or mutations`, async ({
		page
	}) => {
		const api = await mockBudget(page, { permissions: [...permissions] });
		await page.goto(path);
		await expect(page.getByText('ต้องมีสิทธิ์ดูงบประมาณ', { exact: true })).toBeVisible();
		await expect(page.getByRole('button', { name: 'สร้างโครงการ', exact: true })).toHaveCount(0);
		expect(api.reads).toHaveLength(0);
		expect(api.writes).toHaveLength(0);
	});

test('settings-only user saves a new fund with server-owned identity and no project reads', async ({
	page
}) => {
	const api = await mockBudget(page, { permissions: [PERMISSIONS.BUDGET_MANAGE_SCHOOL] });
	await page.goto('/staff/budget/settings');
	await page.getByRole('tab', { name: 'แหล่งเงิน', exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มแหล่งเงิน', exact: true }).click();
	await page.getByLabel('ชื่อแหล่งเงิน *', { exact: true }).last().fill('ทุนใหม่');
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	const saved = api.writes.find((write) => write.path.endsWith('/settings'))!;
	expect((saved.payload.funds as { name: string; id?: string }[]).at(-1)).toEqual({
		name: 'ทุนใหม่',
		active: true
	});
	expect(api.reads.every((path) => path.startsWith('/api/budget/settings'))).toBe(true);
});

test('assigned reviewer reads projects without creator or administration option reads', async ({
	page
}) => {
	const api = await mockBudget(page, {
		permissions: [PERMISSIONS.BUDGET_READ_ASSIGNED, PERMISSIONS.BUDGET_APPROVE_ASSIGNED]
	});
	await page.goto('/staff/budget/projects');
	await expect(page.getByTestId('budget-projects')).toContainText('โครงการคณิตศาสตร์');
	await expect(page.getByRole('button', { name: 'สร้างโครงการ', exact: true })).toHaveCount(0);
	expect(api.reads.every((path) => path.startsWith('/api/budget/projects'))).toBe(true);
});

test('project and catalogs render while the independent request region waits', async ({ page }) => {
	const api = await mockBudget(page, { hold: 'requests' });
	await page.goto(`/staff/budget/projects/${budgetProjectId}`);
	await expect(page.getByTestId('budget-project-detail')).toContainText('กิจกรรมที่หนึ่ง');
	await expect(page.getByTestId('budget-requests')).toHaveAttribute('aria-busy', 'true');
	api.release();
	await expect(page.getByTestId('budget-requests')).toContainText('ยังไม่มีคำขอใช้เงิน');
});

test('activity defaults leave old rows unchanged and explicit bulk edit changes selected rows only', async ({
	page
}) => {
	await mockBudget(page, { approved: false });
	await page.goto(`/staff/budget/projects/${budgetProjectId}`);
	await page.getByRole('button', { name: 'แก้ไขกิจกรรมและค่าใช้จ่าย' }).click();
	const activity = page.getByRole('region', { name: 'กิจกรรมที่หนึ่ง', exact: true });
	await activity.getByRole('combobox', { name: 'แหล่งเงินเริ่มต้น', exact: true }).click();
	await page.getByRole('option', { name: 'เงินพัฒนาผู้เรียน', exact: true }).click();
	await expect(activity.getByRole('combobox', { name: 'แหล่งเงิน', exact: true })).toContainText(
		'เงินอุดหนุน'
	);
	await activity.getByRole('button', { name: 'เพิ่มรายการกำหนดเอง' }).click();
	await expect(
		activity.getByRole('combobox', { name: 'แหล่งเงิน', exact: true }).last()
	).toContainText('เงินพัฒนาผู้เรียน');
	await activity.getByRole('combobox', { name: 'แหล่งเงิน', exact: true }).last().click();
	await page.getByRole('option', { name: 'เงินอุดหนุน', exact: true }).click();
	await activity
		.getByRole('checkbox', { name: 'เลือกรายการที่ 1 เพื่อเปลี่ยนค่า', exact: true })
		.check();
	await activity.getByRole('button', { name: 'ใช้ค่าเริ่มต้นกับ 1 รายการที่เลือก' }).click();
	await expect(
		activity.getByRole('combobox', { name: 'แหล่งเงิน', exact: true }).first()
	).toContainText('เงินพัฒนาผู้เรียน');
	await expect(
		activity.getByRole('combobox', { name: 'แหล่งเงิน', exact: true }).last()
	).toContainText('เงินอุดหนุน');
});

test('amendment edits draft while accepted plan and fiscal context remain fixed', async ({
	page
}) => {
	await mockBudget(page);
	await page.goto(`/staff/budget/projects/${budgetProjectId}`);
	await page.getByRole('button', { name: 'แก้ไขแผนที่อนุมัติ', exact: true }).click();
	await page.getByLabel('เหตุผลการแก้ไข *').fill('ปรับรายละเอียดกิจกรรม');
	await page.getByRole('button', { name: 'เริ่มแก้ไขแผน', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม *', { exact: true }).first().fill('กิจกรรมฉบับแก้ไข');
	await page.getByRole('button', { name: 'บันทึกแผนฉบับร่าง' }).click();
	await expect(page.getByTestId('budget-project-detail')).toContainText('กิจกรรมฉบับแก้ไข');
	const accepted = page
		.locator('details')
		.filter({ has: page.locator('summary').filter({ hasText: 'แผนที่อนุมัติ รุ่น 1' }) });
	await accepted.locator('summary').click();
	await expect(accepted).toContainText('กิจกรรมที่หนึ่ง');
	await expect(accepted).not.toContainText('กิจกรรมฉบับแก้ไข');
	await page.getByRole('button', { name: 'แก้ไขข้อมูลโครงการ' }).click();
	await expect(page.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true })).toBeDisabled();
	await expect(
		page.getByRole('combobox', { name: 'หน่วยงานที่รับผิดชอบ', exact: true })
	).toBeDisabled();
});

test('adjustment creation uses approved allocation choices and exact money', async ({ page }) => {
	const api = await mockBudget(page);
	await page.goto('/staff/budget/adjustments');
	await page.getByRole('button', { name: 'สร้างรายการปรับงบ', exact: true }).click();
	await choose(page, 'ปีงบประมาณ', '2570');
	await page.getByRole('combobox', { name: 'งบประมาณปลายทาง', exact: true }).click();
	await page.getByRole('option').filter({ hasText: 'กิจกรรมที่หนึ่ง' }).click();
	await page.getByLabel('จำนวนเงิน (บาท) *', { exact: true }).fill('123.45');
	await page.getByLabel('เหตุผล *', { exact: true }).fill('เพิ่มงบกิจกรรม');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByTestId('budget-adjustment-detail')).toContainText('123.45');
	expect(
		api.writes.find((write) => write.path === '/api/budget/adjustments')!.payload.amountSatang
	).toBe('12345');
	await page.getByRole('button', { name: 'ส่งรายการพิจารณา', exact: true }).click();
	await api.asReviewer();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('budget-adjustment-detail')).toContainText('อนุมัติแล้ว');
});

function seedTransfer(api: Awaited<ReturnType<typeof mockBudget>>) {
	const adjustmentId = id(970);
	api.adjustments.push({
		id: adjustmentId,
		revision: 1,
		ownerId: actor,
		status: 'submitted',
		kind: 'transfer',
		fiscalYearId: budgetYearId,
		from: {
			projectId: budgetProjectId,
			activityId: api.workspace.acceptedPlan!.activities[0].id,
			fundId: budgetFundId,
			expenseCategoryId: materialsId
		},
		to: {
			projectId: budgetProjectId,
			activityId: api.workspace.acceptedPlan!.activities[1].id,
			fundId: budgetFundId,
			expenseCategoryId: equipmentId
		},
		amountSatang: '10000',
		reason: 'โอนภายในโครงการ',
		loanId: null,
		loanOutstandingSatang: '0',
		createdAt: '2026-10-20T01:00:00Z',
		capabilities: { ...api.workspace.capabilities, canAct: true, canEdit: false, canSubmit: false }
	});
	api.workspace.activities[0].title = 'ชื่อฉบับร่างที่ไม่ใช่แผนอนุมัติ';
	return adjustmentId;
}

test('same-project transfer review resolves distinct accepted debit and credit allocations', async ({
	page
}) => {
	const api = await mockBudget(page, { reviewer: true });
	const adjustmentId = seedTransfer(api);
	await page.goto(`/staff/budget/adjustments/${adjustmentId}`);
	const debit = page.getByRole('region', { name: 'งบประมาณต้นทาง (หักออก)', exact: true }),
		credit = page.getByRole('region', { name: 'งบประมาณปลายทาง (รับเข้า)', exact: true });
	await expect(debit).toContainText('กิจกรรมที่หนึ่ง');
	await expect(debit).toContainText('วัสดุ');
	await expect(credit).toContainText('กิจกรรมที่สอง');
	await expect(credit).toContainText('ครุภัณฑ์');
	await expect(debit).not.toContainText('ชื่อฉบับร่าง');
	await expect(page.getByRole('region', { name: 'ปีงบประมาณของรายการ' })).toContainText('2570');
	expect(
		api.reads.filter((path) => path === `/api/budget/dashboard?projectId=${budgetProjectId}`)
	).toHaveLength(1);
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toBeVisible();
});

test('repayment review resolves retained historical activity after an approved amendment removes it', async ({
	page
}) => {
	const api = await mockBudget(page, { reviewer: true });
	const adjustmentId = seedTransfer(api);
	const historical = api.workspace.acceptedPlan!.activities.shift()!;
	const adjustment = api.adjustments[0];
	adjustment.kind = 'repayment';
	adjustment.loanId = id(969);
	adjustment.reason = 'คืนเงินยืมจากยอดกิจกรรมเดิม';
	api.workspace.acceptedPlan!.version++;
	api.retainedRows.push({
		projectId: budgetProjectId,
		projectName: api.workspace.acceptedPlan!.project.name,
		activityId: historical.id,
		activityTitle: historical.title,
		fundId: budgetFundId,
		fundName: 'เงินอุดหนุน',
		expenseCategoryId: materialsId,
		expenseCategoryName: 'วัสดุ',
		plannedSatang: '0',
		adjustedSatang: '10000',
		currentSatang: '10000',
		reservedSatang: '0',
		paidSatang: '0',
		availableSatang: '10000',
		loanOutstandingSatang: '10000'
	});
	await page.goto(`/staff/budget/adjustments/${adjustmentId}`);
	const debit = page.getByRole('region', { name: 'งบประมาณต้นทาง (หักออก)', exact: true });
	await expect(debit).toContainText('กิจกรรมที่หนึ่ง');
	await expect(debit).not.toContainText('ชื่อฉบับร่าง');
	await expect(
		page.getByRole('region', { name: 'งบประมาณปลายทาง (รับเข้า)', exact: true })
	).toContainText('กิจกรรมที่สอง');
	expect(
		api.workspace.acceptedPlan!.activities.some((activity) => activity.id === historical.id)
	).toBe(false);
	expect(
		api.reads.filter((path) => path === `/api/budget/dashboard?projectId=${budgetProjectId}`)
	).toHaveLength(1);
	expect(api.reads.some((path) => path === `/api/budget/projects/${budgetProjectId}`)).toBe(false);
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('budget-adjustment-detail')).toContainText('อนุมัติแล้ว');
});

test('failed allocation lookup preserves adjustment and blocks approval until focused retries succeed', async ({
	page
}) => {
	const api = await mockBudget(page, { reviewer: true });
	const adjustmentId = seedTransfer(api);
	let reads = 0;
	await page.route(
		(url) =>
			url.pathname === '/api/budget/dashboard' &&
			url.searchParams.get('projectId') === budgetProjectId,
		(route) => {
			reads++;
			return reads === 1
				? route.fulfill({
						status: 503,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'ข้อมูลแผนยังไม่พร้อม' })
					})
				: route.fallback();
		}
	);
	await page.goto(`/staff/budget/adjustments/${adjustmentId}`);
	const debit = page.getByRole('region', { name: 'งบประมาณต้นทาง (หักออก)', exact: true }),
		credit = page.getByRole('region', { name: 'งบประมาณปลายทาง (รับเข้า)', exact: true });
	await expect(debit).toContainText('ข้อมูลแผนยังไม่พร้อม');
	await expect(credit).toContainText('ข้อมูลแผนยังไม่พร้อม');
	expect(reads).toBe(1);
	await expect(page.getByTestId('budget-adjustment-detail')).toContainText('โอนภายในโครงการ');
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toHaveCount(0);
	await debit.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(debit).toContainText('กิจกรรมที่หนึ่ง');
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toHaveCount(0);
	await credit.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(credit).toContainText('กิจกรรมที่สอง');
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toBeVisible();
	expect(api.writes).toHaveLength(0);
});

test('plan conflict reload retains draft, reviews latest state and reconciles parent before retry', async ({
	page
}) => {
	const api = await mockBudget(page, { approved: false, stalePlan: true });
	await page.goto(`/staff/budget/projects/${budgetProjectId}`);
	await page.getByRole('button', { name: 'แก้ไขกิจกรรมและค่าใช้จ่าย' }).click();
	await page.getByLabel('ชื่อกิจกรรม *', { exact: true }).first().fill('กิจกรรมฉบับร่างของฉัน');
	await page.getByRole('button', { name: 'บันทึกแผนฉบับร่าง' }).click();
	await expect(page.getByRole('region', { name: 'ทบทวนแผนล่าสุด' })).toContainText(
		'ฉบับร่างของคุณยังอยู่'
	);
	await expect(page.getByRole('button', { name: 'บันทึกแผนฉบับร่าง' })).toBeDisabled();
	await page.getByRole('button', { name: 'โหลดแผนล่าสุดและเก็บฉบับร่าง' }).click();
	await expect(page.getByRole('region', { name: 'ทบทวนแผนล่าสุด' })).toContainText(
		'กิจกรรมจากอีกคน'
	);
	await expect(page.getByLabel('ชื่อกิจกรรม *', { exact: true }).first()).toHaveValue(
		'กิจกรรมฉบับร่างของฉัน'
	);
	await page
		.getByRole('checkbox', {
			name: 'ฉันเปรียบเทียบแผนล่าสุดแล้ว และต้องการบันทึกกิจกรรมในฉบับร่างนี้'
		})
		.check();
	await page.getByRole('button', { name: 'ใช้แผนล่าสุดเป็นฐานและเก็บฉบับร่าง' }).click();
	await page.getByRole('button', { name: 'บันทึกแผนฉบับร่าง' }).click();
	await expect(page.getByTestId('budget-project-detail')).toContainText('กิจกรรมฉบับร่างของฉัน');
	expect(api.workspace.project.name).toBe('ชื่อโครงการจากอีกคน');
	const writes = api.writes.filter(
		(write) => write.path === `/api/budget/projects/${budgetProjectId}`
	);
	expect(writes).toHaveLength(2);
	expect(writes.map((write) => write.payload.expectedRevision)).toEqual([1, 2]);
	expect(writes[0].payload.requestId).not.toBe(writes[1].payload.requestId);
});

test('payment conflict reload recomputes remaining amount and requires fresh confirmation without duplicates', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-20T08:00:00Z'));
	const api = await mockBudget(page, { withRequest: true, stalePayment: true });
	await page.goto(`/staff/budget/requests/${budgetRequestId}`);
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).click();
	await date(page, 'วันที่จ่ายเงิน');
	await page.getByLabel('เลขที่ใบสำคัญ *').fill('PV-MINE');
	await page.getByLabel('จำนวนเงินจ่าย รายการค่าใช้จ่าย 1', { exact: true }).fill('7.00');
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).last().click();
	await expect(
		page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).last()
	).toBeDisabled();
	await page.getByRole('button', { name: 'โหลดยอดล่าสุดและเก็บข้อมูลที่กรอก' }).click();
	await expect(page.getByTestId('budget-payment-form')).toContainText('ที่จ่ายได้ 5.00 บาท');
	await expect(page.getByLabel('เลขที่ใบสำคัญ *')).toHaveValue('PV-MINE');
	await expect(page.getByLabel('จำนวนเงินจ่าย รายการค่าใช้จ่าย 1', { exact: true })).toHaveValue(
		'7.00'
	);
	await expect(page.getByRole('button', { name: 'ยืนยันใช้ยอดล่าสุด' })).toBeDisabled();
	await page
		.getByRole('checkbox', { name: 'ฉันตรวจสอบยอดคงเหลือล่าสุดและรายการที่ต้องการบันทึกแล้ว' })
		.check();
	await page.getByRole('button', { name: 'ยืนยันใช้ยอดล่าสุด' }).click();
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).last().click();
	await expect(page.getByTestId('budget-payment-form')).toContainText(
		'จำนวนเงินเกินยอดคงเหลือของรายการ'
	);
	expect(api.writes.filter((write) => write.path.endsWith('/payments'))).toHaveLength(1);
	await page.getByLabel('จำนวนเงินจ่าย รายการค่าใช้จ่าย 1', { exact: true }).fill('4.00');
	await page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }).last().click();
	await expect(page.getByTestId('budget-payments')).toContainText('PV-OTHER');
	await expect(page.getByTestId('budget-payments')).toContainText('PV-MINE');
	expect(api.payments).toHaveLength(2);
	expect(api.request!.request.paidSatang).toBe('900');
	expect(api.writes.filter((write) => write.path.endsWith('/payments'))).toHaveLength(2);
});

test('requester cannot approve own plan; explicit reviewer identity can review the submitted plan', async ({
	page
}) => {
	const api = await mockBudget(page, { approved: false });
	await page.goto(`/staff/budget/projects/${budgetProjectId}`);
	await page.getByRole('button', { name: 'ส่งแผนพิจารณา' }).click();
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toHaveCount(0);
	const status = await page.evaluate(async (id) => {
		const response = await fetch(`/api/budget/projects/${id}/actions`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				requestId: crypto.randomUUID(),
				expectedRevision: 2,
				expectedProcessRevision: 1,
				action: 'approve',
				comment: null
			})
		});
		return response.status;
	}, budgetProjectId);
	expect(status).toBe(403);
	expect(api.workspace.project.status).toBe('submitted');
	await api.asReviewer();
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toBeVisible();
});

test('mixed read and download scopes use server-scoped report preview and matching CSV', async ({
	page
}) => {
	const api = await mockBudget(page, {
		permissions: [PERMISSIONS.BUDGET_READ_SCHOOL, PERMISSIONS.BUDGET_DOWNLOAD_OWN],
		exportFiltered: true
	});
	await page.goto('/staff/budget');
	await expect(page.getByTestId('budget-dashboard')).toContainText('9,928.00');
	// This case compares server scopes; workspace navigation is owned by the sidebar.
	await page.goto('/staff/budget/reports');
	await expect(
		page.getByText('รายงานนี้แสดงเฉพาะข้อมูลที่คุณมีสิทธิ์ดูและส่งออก', { exact: true })
	).toBeVisible();
	await expect(page.getByTestId('budget-pivot')).toContainText('3,588.00');
	await expect(page.getByTestId('budget-pivot')).not.toContainText('กิจกรรมที่สอง');
	const downloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: 'ส่งออก CSV' }).click();
	const download = await downloadPromise;
	const stream = await download.createReadStream();
	const chunks = [];
	for await (const chunk of stream) chunks.push(chunk);
	const csv = Buffer.concat(chunks).toString('utf8');
	expect(csv).toContain('3588.00');
	expect(csv).not.toContain('9928.00');
	expect(api.reads.some((path) => path.includes('/dashboard?exportScope=true'))).toBe(true);
	expect(api.reads.some((path) => path.includes('/reports/export?exportScope=true'))).toBe(true);
});

async function activate(control: Locator, touch: boolean) {
	if (touch) await control.tap();
	else await control.click();
}
async function assertBudgetGeometry(page: Page) {
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
		true
	);
	for (const pane of await page.locator('[data-slot="table-container"]').all()) {
		const geometry = await pane.evaluate((element) => ({
			overflow: getComputedStyle(element).overflowX,
			width: element.clientWidth,
			content: element.scrollWidth
		}));
		if (geometry.content > geometry.width) expect(geometry.overflow).toBe('auto');
	}
}
async function captureBudget(page: Page, name: string, width: number, theme: string) {
	await assertBudgetGeometry(page);
	await page.screenshot({
		path: `/tmp/budget-design-${width}-${theme}-${name}.png`,
		fullPage: true,
		animations: 'disabled'
	});
}
const budgetViews = [
	['overview', '/staff/budget', 'budget-dashboard', '9,928.00'],
	['projects', '/staff/budget/projects', 'budget-projects', 'โครงการคณิตศาสตร์'],
	[
		'project-detail',
		`/staff/budget/projects/${budgetProjectId}`,
		'budget-project-detail',
		'3,588.00'
	],
	['requests', '/staff/budget/requests', 'budget-requests', 'คำขอทดสอบ'],
	['request-detail', `/staff/budget/requests/${budgetRequestId}`, 'budget-request-detail', '10.00'],
	['adjustments', '/staff/budget/adjustments', 'budget-adjustments', 'โอนภายในโครงการ'],
	[
		'adjustment-detail',
		`/staff/budget/adjustments/${id(970)}`,
		'budget-adjustment-detail',
		'โอนภายในโครงการ'
	],
	['reports', '/staff/budget/reports', 'budget-pivot', '9,928.00'],
	['settings', '/staff/budget/settings', 'budget-settings', '']
] as const;
const budgetDialogs = [
	'project-new',
	'plan-editor',
	'amendment',
	'request-form',
	'payment',
	'refund',
	'close',
	'adjustment-form',
	'workflow'
] as const;
for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 1440, height: 1000 }
])
	for (const theme of ['light', 'dark'] as const)
		test.describe(`budget design ${viewport.width}px ${theme}`, () => {
			test.use({ viewport, hasTouch: viewport.width === 390 });
			test.beforeEach(async ({ page }) => {
				await page.addInitScript(
					(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
					theme
				);
			});
			for (const [name, path, region, text] of budgetViews)
				test(`${name} uses the shared page pattern without unintended writes`, async ({ page }) => {
					const api = await mockBudget(page, { withRequest: true });
					seedTransfer(api);
					api.request!.request.title =
						'คำขอทดสอบวัสดุสำหรับกิจกรรมการเรียนรู้แบบบูรณาการและพัฒนาทักษะของนักเรียนทุกระดับชั้น';
					api.request!.lines[0].description =
						'วัสดุประกอบกิจกรรมการเรียนรู้แบบบูรณาการสำหรับนักเรียน';
					await page.goto(path);
					await expect(page.getByTestId(region)).toBeVisible();
					if (text) await expect(page.getByTestId(region)).toContainText(text);
					else
						await expect(page.getByLabel('ชื่อปีงบประมาณ *', { exact: true })).toHaveValue('2570');
					await expect(page.locator('html')).toHaveClass(
						theme === 'dark' ? /dark/ : /^(?!.*\bdark\b)/
					);
					await expect(page.locator('.school-page-shell h1')).toHaveCount(1);
					await expect(
						page.getByRole('navigation', { name: 'งานงบประมาณ', exact: true })
					).toHaveCount(0);
					if (name === 'projects' || name === 'requests' || name === 'adjustments') {
						const pagination = page.getByRole('navigation', {
							name: 'หน้ารายการงบประมาณ',
							exact: true
						});
						await expect(pagination).toBeVisible();
						await expect(pagination).toContainText('1–1 จาก 1 รายการ');
						await expect(
							pagination.getByRole('button', { name: 'ก่อนหน้า', exact: true })
						).toBeDisabled();
						await expect(
							pagination.getByRole('button', { name: 'ถัดไป', exact: true })
						).toBeDisabled();
					}
					if (name === 'requests' || name === 'adjustments')
						await expect(
							page.getByRole('columnheader', { name: 'จำนวนเงิน (บาท)', exact: true })
						).toHaveCSS('text-align', 'right');
					await captureBudget(page, name, viewport.width, theme);
					expect(api.writes).toHaveLength(0);
				});
			test('overview empty state remains useful and neutral', async ({ page }) => {
				const api = await mockBudget(page, { empty: true });
				await page.goto('/staff/budget');
				await expect(
					page.getByText('ยังไม่มีรายการงบประมาณที่อนุมัติ', { exact: true })
				).toBeVisible();
				await captureBudget(page, 'overview-empty', viewport.width, theme);
				expect(api.writes).toHaveLength(0);
			});
			for (const name of budgetDialogs)
				test(`${name} keeps fields, actions and keyboard dismissal usable`, async ({ page }) => {
					const api = await mockBudget(page, {
						approved: name !== 'plan-editor',
						withRequest: name !== 'plan-editor',
						reviewer: name === 'workflow'
					});
					if (name !== 'plan-editor') seedTransfer(api);
					if (name === 'refund') {
						const detail = api.request!;
						detail.request.paidSatang = '1000';
						detail.request.reservedSatang = '0';
						detail.request.remainingSatang = '0';
						detail.lines[0].paidSatang = '1000';
						detail.lines[0].reservedSatang = '0';
						api.payments.push({
							id: id(860),
							requestId: budgetRequestId,
							paidOn: '2026-10-20',
							voucherNumber: 'PV-UI',
							note: null,
							totalSatang: '1000',
							allocations: [{ planLineId: detail.lines[0].planLineId, amountSatang: '1000' }],
							evidence: [],
							createdBy: actor,
							createdAt: '2026-10-20T01:00:00Z',
							refunds: []
						});
					}
					const path =
						name === 'project-new'
							? '/staff/budget/projects'
							: name === 'adjustment-form'
								? '/staff/budget/adjustments'
								: name === 'workflow'
									? `/staff/budget/adjustments/${id(970)}`
									: ['payment', 'refund', 'close'].includes(name)
										? `/staff/budget/requests/${budgetRequestId}`
										: `/staff/budget/projects/${budgetProjectId}`;
					await page.goto(path);
					const trigger =
						name === 'project-new'
							? 'สร้างโครงการ'
							: name === 'plan-editor'
								? 'แก้ไขกิจกรรมและค่าใช้จ่าย'
								: name === 'amendment'
									? 'แก้ไขแผนที่อนุมัติ'
									: name === 'request-form'
										? 'สร้างคำขอใช้เงิน'
										: name === 'payment'
											? 'บันทึกการจ่ายเงิน'
											: name === 'refund'
												? 'บันทึกคืนเงิน'
												: name === 'close'
													? 'ปิดคำขอ'
													: name === 'adjustment-form'
														? 'สร้างรายการปรับงบ'
														: 'อนุมัติ';
					await activate(
						page.getByRole('button', { name: trigger, exact: true }).first(),
						viewport.width === 390
					);
					const dialog = page.getByRole(name === 'close' ? 'alertdialog' : 'dialog');
					if (name === 'plan-editor') {
						// The plan editor is an inline workflow with its own trailing actions.
						await expect(page.getByTestId('budget-plan-editor')).toBeVisible();
						await expect(page.getByLabel('ชื่อกิจกรรม *', { exact: true }).first()).toHaveValue(
							'กิจกรรมที่หนึ่ง'
						);
					} else {
						await expect(dialog).toBeVisible();
						if (name === 'project-new')
							await expect(
								page.getByRole('combobox', { name: 'ปีงบประมาณ', exact: true })
							).toBeEnabled();
						if (name === 'adjustment-form')
							await expect(
								dialog.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true })
							).toBeEnabled();
					}
					await captureBudget(page, name, viewport.width, theme);
					if (name === 'request-form') {
						await dialog.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
						await expect(dialog).toContainText('กรอกชื่อคำขอและเลือกกิจกรรม');
						await expect(dialog.getByLabel('ชื่อคำขอใช้เงิน *')).toBeFocused();
						await page.keyboard.type('ฉบับร่างสำหรับตรวจสอบหน้าจอ');
					} else if (name === 'payment' || name === 'refund') {
						await dialog
							.getByRole('button', {
								name: name === 'payment' ? 'บันทึกการจ่ายเงิน' : 'ยืนยันคืนเงิน',
								exact: true
							})
							.click();
						await expect(dialog).toContainText('เลือกวันที่ทำรายการ');
						await expect(dialog.getByLabel('แนบหลักฐาน', { exact: true })).toBeEnabled();
					} else if (name === 'workflow') {
						await dialog.getByLabel('ความคิดเห็น / เหตุผล').focus();
						await page.keyboard.type('ตรวจสอบข้อมูลก่อนพิจารณา');
					}
					const cancel = (
						name === 'plan-editor' ? page.getByTestId('budget-plan-editor') : dialog
					).getByRole('button', { name: 'ยกเลิก', exact: true });
					await cancel.scrollIntoViewIfNeeded();
					await captureBudget(page, `${name}-actions`, viewport.width, theme);
					await cancel.focus();
					await page.keyboard.press('Enter');
					if (name === 'plan-editor')
						await expect(page.getByTestId('budget-plan-editor')).toHaveCount(0);
					else await expect(dialog).toHaveCount(0);
					expect(api.writes).toHaveLength(0);
				});
		});

for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 1440, height: 1000 }
])
	for (const theme of ['light', 'dark'] as const)
		test.describe(`budget feedback ${viewport.width}px ${theme}`, () => {
			test.use({ viewport, hasTouch: viewport.width === 390 });
			test.beforeEach(async ({ page }) => {
				await page.addInitScript(
					(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
					theme
				);
			});
			test('loading and forbidden results retain the page header and recovery surface', async ({
				page
			}) => {
				const api = await mockBudget(page, { hold: 'projects' });
				await page.goto('/staff/budget/projects');
				await expect(page.getByTestId('budget-projects')).toHaveAttribute('aria-busy', 'true');
				await expect(
					page.getByRole('heading', { name: 'โครงการงบประมาณ', exact: true })
				).toBeVisible();
				await captureBudget(page, 'projects-loading', viewport.width, theme);
				api.release();
				await expect(page.getByTestId('budget-projects')).toContainText('โครงการคณิตศาสตร์');
				await page.route(
					(url) => url.pathname === '/api/budget/projects',
					(route) =>
						route.fulfill({
							status: 403,
							contentType: 'application/json',
							body: JSON.stringify({ success: false, error: 'คุณไม่มีสิทธิ์ดูโครงการนี้' })
						})
				);
				await page.reload();
				await expect(page.getByTestId('budget-projects')).toContainText(
					'คุณไม่มีสิทธิ์ดูโครงการนี้'
				);
				await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeEnabled();
				await captureBudget(page, 'projects-forbidden', viewport.width, theme);
				expect(api.writes).toHaveLength(0);
			});
			test('pending payment disables duplicate submit and keeps entered fields visible', async ({
				page
			}) => {
				const api = await mockBudget(page, { withRequest: true, holdMutation: true });
				await page.goto(`/staff/budget/requests/${budgetRequestId}`);
				await activate(
					page.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true }),
					viewport.width === 390
				);
				await date(page, 'วันที่จ่ายเงิน');
				await page.getByLabel('เลขที่ใบสำคัญ *').fill('PV-PENDING');
				await page
					.getByLabel(`จำนวนเงินจ่าย ${api.request!.lines[0].description}`, { exact: true })
					.fill('2.00');
				await page
					.getByRole('dialog')
					.getByRole('button', { name: 'บันทึกการจ่ายเงิน', exact: true })
					.click();
				await expect(
					page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true })
				).toBeDisabled();
				await expect(
					page.getByRole('dialog').getByRole('button', { name: 'กำลังดำเนินการ...', exact: true })
				).toBeDisabled();
				await expect(page.getByLabel('เลขที่ใบสำคัญ *')).toHaveValue('PV-PENDING');
				await captureBudget(page, 'payment-pending', viewport.width, theme);
				expect(api.writes).toHaveLength(1);
				api.release();
				await expect(page.getByTestId('budget-payments')).toContainText('PV-PENDING');
				expect(api.writes).toHaveLength(1);
			});
		});

test('settings is discoverable from authorized headers and absent for readers', async ({
	page
}) => {
	const api = await mockBudget(page);
	for (const path of ['/staff/budget', '/staff/budget/projects', '/staff/budget/reports']) {
		await page.goto(path);
		await expect(page.getByRole('link', { name: 'ตั้งค่างบประมาณ', exact: true })).toBeVisible();
	}
	await page.getByRole('link', { name: 'ตั้งค่างบประมาณ', exact: true }).focus();
	await page.keyboard.press('Enter');
	await expect(page).toHaveURL(/\/staff\/budget\/settings$/);
	await expect(page.getByTestId('budget-settings')).toBeVisible();
	await page.getByRole('link', { name: 'กลับภาพรวมงบประมาณ', exact: true }).click();
	await expect(page).toHaveURL(/\/staff\/budget$/);
	expect(api.writes).toHaveLength(0);
});
test('reader has no settings or create action in the project header', async ({ page }) => {
	const api = await mockBudget(page, { permissions: [PERMISSIONS.BUDGET_READ_SCHOOL] });
	await page.goto('/staff/budget/projects');
	await expect(page.getByTestId('budget-projects')).toContainText('โครงการคณิตศาสตร์');
	await expect(page.getByRole('link', { name: 'ตั้งค่างบประมาณ', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'สร้างโครงการ', exact: true })).toHaveCount(0);
	expect(api.reads.some((path) => path.includes('/settings') || path.includes('/lookup/'))).toBe(
		false
	);
	expect(api.writes).toHaveLength(0);
});

const largeAmountDashboard: BudgetDashboard = {
	rows: [
		{
			projectId: budgetProjectId,
			projectName: 'โครงการงบประมาณจำนวนมาก',
			activityId: id(820),
			activityTitle: 'กิจกรรมที่มีการจ่ายและผูกพันบางส่วน',
			fundId: budgetFundId,
			fundName: 'เงินอุดหนุน',
			expenseCategoryId: materialsId,
			expenseCategoryName: 'วัสดุ',
			plannedSatang: '9007199254740993',
			adjustedSatang: '0',
			currentSatang: '9007199254740993',
			reservedSatang: '3002399751580331',
			paidSatang: '3002399751580331',
			availableSatang: '3002399751580331',
			loanOutstandingSatang: '0'
		}
	],
	totals: {
		plannedSatang: '9007199254740993',
		adjustedSatang: '0',
		currentSatang: '9007199254740993',
		reservedSatang: '3002399751580331',
		paidSatang: '3002399751580331',
		availableSatang: '3002399751580331',
		loanOutstandingSatang: '0'
	}
};
for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 1440, height: 1000 }
])
	for (const theme of ['light', 'dark'] as const)
		test.describe(`budget navigation and large money ${viewport.width}px ${theme}`, () => {
			test.use({ viewport, hasTouch: viewport.width === 390 });
			test.beforeEach(async ({ page }) => {
				await page.addInitScript(
					(theme) =>
						localStorage.setItem(
							'ui-preferences',
							JSON.stringify({ theme, sidebarCollapsed: false, sidebarExpandedGroups: {} })
						),
					theme
				);
			});
			test('large exact amounts wrap inside compact summary cards and reconcile with their allocation', async ({
				page
			}) => {
				const api = await mockBudget(page, { dashboardOverride: largeAmountDashboard });
				const totals = largeAmountDashboard.totals;
				expect(BigInt(totals.currentSatang)).toBe(
					BigInt(totals.reservedSatang) + BigInt(totals.paidSatang) + BigInt(totals.availableSatang)
				);
				for (const [key, value] of Object.entries(totals))
					expect(largeAmountDashboard.rows[0][key as keyof typeof totals]).toBe(value);
				await page.goto('/staff/budget');
				const summary = page.getByTestId('budget-dashboard');
				const cards = summary.locator('[data-slot="card"]').filter({ has: page.locator('dt') });
				await expect(cards).toHaveCount(4);
				const labels = ['งบประมาณปัจจุบัน', 'ผูกพัน', 'จ่ายจริง', 'คงเหลือใช้ได้'];
				for (let index = 0; index < labels.length; index++) {
					await expect(cards.nth(index).locator('dt')).toHaveText(labels[index]);
					await expect(cards.nth(index).locator('dd')).toHaveText(
						`${index === 0 ? '90,071,992,547,409.93' : '30,023,997,515,803.31'} บาท`
					);
					expect(
						await cards
							.nth(index)
							.evaluate(
								(element) =>
									element.scrollWidth <= element.clientWidth &&
									[...element.querySelectorAll('dt,dd')].every(
										(text) => text.scrollWidth <= text.clientWidth
									)
							)
					).toBe(true);
				}
				const boxes = await cards.evaluateAll((elements) =>
					elements.map((element) => {
						const box = element.getBoundingClientRect();
						return { x: box.x, y: box.y, right: box.right, bottom: box.bottom };
					})
				);
				if (viewport.width === 390) {
					expect(Math.abs(boxes[0].y - boxes[1].y)).toBeLessThan(1);
					expect(Math.abs(boxes[2].y - boxes[3].y)).toBeLessThan(1);
					expect(Math.abs(boxes[0].x - boxes[2].x)).toBeLessThan(1);
					expect(boxes[1].x).toBeGreaterThan(boxes[0].right);
					expect(boxes[2].y).toBeGreaterThan(boxes[0].bottom);
				} else {
					for (let index = 1; index < boxes.length; index++) {
						expect(Math.abs(boxes[index].y - boxes[0].y)).toBeLessThan(1);
						expect(boxes[index].x).toBeGreaterThan(boxes[index - 1].right);
					}
				}
				await expect(
					summary.getByRole('cell', { name: '90,071,992,547,409.93', exact: true })
				).toHaveCount(2);
				await captureBudget(page, 'overview-large-amounts', viewport.width, theme);
				expect(api.writes).toHaveLength(0);
			});
			test('real budget sidebar sections navigate by pointer and keyboard while list paging stays accessible', async ({
				page
			}) => {
				const api = await mockBudget(page, { withRequest: true });
				seedTransfer(api);
				await page.goto('/staff/budget');
				await expect(page.getByTestId('budget-dashboard')).toContainText('9,928.00');
				const sidebar = page.locator('aside');
				const mobile = viewport.width === 390;
				if (mobile) await page.getByRole('button', { name: 'Open Menu', exact: true }).tap();
				const planning = sidebar.getByRole('button', { name: 'แผนงานและโครงการ', exact: true });
				await expect(planning).toHaveAttribute('aria-expanded', 'true');
				const finance = sidebar.getByRole('button', { name: 'การเงินและงบประมาณ', exact: true });
				await finance.focus();
				await page.keyboard.press('Enter');
				await expect(finance).toHaveAttribute('aria-expanded', 'true');
				for (const group of budgetMenuGroups)
					for (const item of group.items)
						await expect(
							sidebar.getByRole('link', { name: item.name, exact: true })
						).toHaveAttribute('href', item.path);
				await captureBudget(page, 'sidebar-navigation', viewport.width, theme);
				await activate(sidebar.getByRole('link', { name: 'คำขอใช้เงิน', exact: true }), mobile);
				await expect(page).toHaveURL(/\/staff\/budget\/requests$/);
				await expect(page.getByTestId('budget-requests')).toContainText('คำขอทดสอบ');
				await expect(
					page.getByRole('navigation', { name: 'หน้ารายการงบประมาณ', exact: true })
				).toBeVisible();
				if (mobile) await page.getByRole('button', { name: 'Open Menu', exact: true }).tap();
				await sidebar.getByRole('link', { name: 'ปรับงบประมาณและเงินยืม', exact: true }).focus();
				await page.keyboard.press('Enter');
				await expect(page).toHaveURL(/\/staff\/budget\/adjustments$/);
				await expect(page.getByTestId('budget-adjustments')).toContainText('โอนภายในโครงการ');
				await expect(
					page.getByRole('navigation', { name: 'หน้ารายการงบประมาณ', exact: true })
				).toBeVisible();
				if (mobile) await page.getByRole('button', { name: 'Open Menu', exact: true }).tap();
				if ((await planning.getAttribute('aria-expanded')) !== 'true')
					await activate(planning, mobile);
				await sidebar.getByRole('link', { name: 'รายงานงบประมาณ', exact: true }).focus();
				await page.keyboard.press('Enter');
				await expect(page).toHaveURL(/\/staff\/budget\/reports$/);
				await expect(page.getByTestId('budget-pivot')).toContainText('9,928.00');
				await expect(
					page.getByRole('navigation', { name: 'งานงบประมาณ', exact: true })
				).toHaveCount(0);
				expect(api.writes).toHaveLength(0);
			});
		});
