// Intercepted synthetic APIs verify browser transport/state; backend fixtures own authorization and accounting.
import { test, expect, type Page } from '@playwright/test';
import {
	mockBudget,
	budgetProjectId,
	budgetRequestId,
	budgetYearId,
	budgetFundId,
	materialsId,
	equipmentId
} from './fixtures/budget';
import { id, actor } from './fixtures/staff-home-route-data';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';
test.use({ serviceWorkers: 'block' });
async function choose(page: Page, label: string, option: string) {
	await page.getByRole('combobox', { name: label, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}
async function date(page: Page, label: string) {
	await page.getByRole('button', { name: label, exact: true }).click();
	await page
		.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: /^20$/ })
		.click();
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
	await page.getByRole('link', { name: 'รายงาน', exact: true }).click();
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
