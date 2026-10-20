import { test, expect, type Page } from '@playwright/test';
import { mockBudget, budgetProjectId } from './fixtures/budget';
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
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	await page.getByRole('button', { name: 'สร้างคำขอใช้เงิน' }).click();
	await page.getByLabel('ชื่อคำขอใช้เงิน *').fill('ขอซื้อกระดาษ');
	await choose(page, 'กิจกรรม', 'กิจกรรมทดลอง');
	await page.getByRole('checkbox', { name: 'กระดาษ', exact: true }).check();
	await page.getByLabel('จำนวนเงิน (บาท) *', { exact: true }).fill('15.00');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByTestId('budget-request-detail')).toContainText('15.00');
	await page.getByRole('button', { name: 'ส่งคำขอพิจารณา' }).click();
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
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
	await page
		.getByLabel('แนบหลักฐาน', { exact: true })
		.setInputFiles({
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
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('budget-adjustment-detail')).toContainText('อนุมัติแล้ว');
});
