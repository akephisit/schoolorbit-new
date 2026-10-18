import { test, expect, type Page } from '@playwright/test';
import {
	mockLeave,
	leaveDetailPath,
	leaveId,
	secondLeaveId,
	leaveTypeId
} from './fixtures/staff-leave';
import { navigate } from './fixtures/supervision-route-data';
import { revalidatePage } from './fixtures/page-revalidation';
import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

async function chooseDate(page: Page, id: string, day = 19) {
	await page.locator(`#${id}`).click();
	await page
		.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
		.filter({ hasText: new RegExp(`^${day}$`) })
		.click();
}
async function draw(page: Page) {
	const surface = page.getByRole('img', { name: 'พื้นที่วาดลายมือชื่อ' });
	await surface.scrollIntoViewIfNeeded();
	const box = (await surface.boundingBox())!;
	await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.4);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.6, { steps: 8 });
	await page.mouse.up();
}

test('reader loads list without create, routing or signature reads and keeps URL paging', async ({
	page
}) => {
	const api = await mockLeave(page, { permissions: [PERMISSIONS.STAFF_LEAVE_READ_OWN] });
	await page.goto('/staff/leave');
	await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
	await expect(page.getByRole('button', { name: 'สร้างคำขอการลา' })).toHaveCount(0);
	expect(api.count('/api/staff-leave/options')).toBe(0);
	expect(api.count('/api/staff-leave/routing-options')).toBe(0);
	await page.getByRole('link', { name: 'ถัดไป', exact: true }).click();
	await expect(page).toHaveURL(/offset=20/);
	await page.getByRole('link', { name: 'เปิดคำขอ', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('เข้ารับการตรวจรักษา');
	await page.getByRole('link', { name: 'กลับรายการคำขอ' }).click();
	await expect(page).toHaveURL(/offset=20/);
});

test('initial loading, retry and background failure preserve usable request list', async ({
	page
}) => {
	const api = await mockLeave(page, { hold: 'requests' });
	await page.goto('/staff/leave');
	await expect(page.getByRole('status', { name: 'กำลังโหลดคำขอ' })).toBeVisible();
	api.release();
	await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
	await revalidatePage(page);
	await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
});
test('failed initial list retries only its region', async ({ page }) => {
	const api = await mockLeave(page, { fail: 'requests' });
	await page.goto('/staff/leave');
	await expect(page.getByTestId('leave-requests')).toContainText('บริการ requests ไม่พร้อม');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
	expect(api.count('/api/staff-leave/options')).toBe(0);
});

test('create is lazy, resolves policy, saves server-derived half-day draft then submits', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto('/staff/leave');
	expect(api.count('/api/staff-leave/options')).toBe(0);
	await page.getByRole('button', { name: 'สร้างคำขอการลา', exact: true }).click();
	await page.locator('#leave-type').click();
	await page.getByRole('option', { name: 'ลาป่วย', exact: true }).click();
	await chooseDate(page, 'leave-start-date');
	await expect(page.getByTestId('leave-effective-policy')).toContainText('คำสั่งโรงเรียนทดสอบ');
	await page.getByRole('radio', { name: 'ครึ่งวันเช้า', exact: true }).click();
	await page.getByLabel('เหตุผล *', { exact: true }).fill('ตรวจรักษาช่วงเช้า');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(leaveId));
	await expect(page.getByTestId('leave-detail')).toContainText('0.5 วัน');
	const created = api.writes.find((write) => write.path === '/api/staff-leave/requests')!;
	expect(created.payload.period).not.toHaveProperty('amount');
	expect(created.payload.period).toMatchObject({
		dayPortion: 'morning',
		reason: 'ตรวจรักษาช่วงเช้า'
	});
	await page.getByRole('button', { name: 'ส่งคำขอพิจารณา', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('รอพิจารณา');
	expect(api.count(`/api/staff-leave/requests/${leaveId}`)).toBe(1);
});

test('unconfigured applicable profile gives clear error and does not save', async ({ page }) => {
	const api = await mockLeave(page, { missingPolicy: true });
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'แก้ไขฉบับร่าง' }).click();
	await expect(page.getByTestId('leave-effective-policy')).toContainText(
		'ยังไม่มีนโยบายสำหรับประเภทบุคลากรนี้'
	);
	await expect(page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true })).toBeDisabled();
	expect(api.writes).toHaveLength(0);
});

test('draft survives save failure and exact retry keeps the mutation key', async ({ page }) => {
	const api = await mockLeave(page, { failMutation: 503 });
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'แก้ไขฉบับร่าง' }).click();
	await expect(page.getByTestId('leave-effective-policy')).toContainText('นโยบายฉบับ');
	await page.getByLabel('เหตุผล *', { exact: true }).fill('ข้อมูลที่ต้องเก็บไว้');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByTestId('leave-request-editor')).toContainText('บริการไม่พร้อม');
	await expect(page.getByLabel('เหตุผล *', { exact: true })).toHaveValue('ข้อมูลที่ต้องเก็บไว้');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('ข้อมูลที่ต้องเก็บไว้');
	expect(api.writes[0].payload.requestId).toBe(api.writes[1].payload.requestId);
});

test('reviewer uses ordinary approve action without signature upload', async ({ page }) => {
	const api = await mockLeave(page, { role: 'reviewer', status: 'submitted' });
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await expect(page.getByTestId('signature-pad')).toHaveCount(0);
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('อนุมัติแล้ว');
	expect(api.writes.at(-1)!.payload.signature).toBeNull();
});

test('configured signature requires explicit revision-bound consent and retains draft through error', async ({
	page
}) => {
	const api = await mockLeave(page, {
		role: 'reviewer',
		status: 'submitted',
		requireSignature: true,
		failMutation: 503
	});
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await draw(page);
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('leave-action-form')).toContainText('ระบุลายมือชื่อและยืนยัน');
	expect(api.writes).toHaveLength(0);
	await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('leave-action-form')).toContainText('บริการไม่พร้อม');
	await expect(page.getByTestId('signature-pad').locator('path')).toHaveCount(1);
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('ลงลายมือชื่อและยืนยันคำขอฉบับ 1');
	expect(api.writes[0].payload.requestId).toBe(api.writes[1].payload.requestId);
	expect(api.writes[1].payload.signature).toMatchObject({
		consent: true,
		input: { method: 'drawn', documentRevision: 1, contentHash: 'a'.repeat(64) }
	});
});

test('returned owner edits and resubmits; approved cancellation stays pending with history', async ({
	page
}) => {
	const api = await mockLeave(page, { status: 'returned' });
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'ส่งคำขออีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('รอพิจารณา');
	api.setStatus('approved');
	await revalidatePage(page);
	await page.getByRole('button', { name: 'ขอยกเลิกคำขอ', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('ยังคงมีผลและนับสิทธิ์');
	await page.getByLabel('เหตุผลการยกเลิก *').fill('เปลี่ยนกำหนดนัด');
	await page.getByRole('button', { name: 'ยืนยันยกเลิกคำขอ', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('รออนุมัติยกเลิก');
	await expect(page.getByTestId('leave-detail')).toContainText('ประวัติการดำเนินการ');
});

test('settings clone preserves criteria and patches type revision before later archive', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto('/staff/leave/settings');
	expect(api.count('/api/staff-leave/routing-options')).toBe(0);
	await page.getByRole('button', { name: 'ดูนโยบาย ลาป่วย', exact: true }).click();
	await page.getByRole('button', { name: 'สร้างฉบับใหม่จากฉบับนี้', exact: true }).click();
	await expect(page.locator('#policy-source')).toHaveValue('คำสั่งโรงเรียนทดสอบ 1/2569');
	await expect(page.locator('#policy-max')).toHaveValue('5');
	await expect(page.getByTestId('leave-policy-editor')).toContainText('ครูหัวหน้าฝ่าย ทดสอบ');
	await expect(page.locator('#policy-effective')).toContainText('เลือกวันที่');
	await chooseDate(page, 'policy-effective');
	await page.locator('#policy-source').fill('คำสั่งใหม่ 2/2569');
	await page.getByRole('button', { name: 'บันทึกนโยบายฉบับใหม่', exact: true }).click();
	await expect(page.getByTestId('leave-policies')).toContainText('คำสั่งใหม่ 2/2569');
	const row = page.getByRole('row').filter({ hasText: 'ลาป่วย' });
	await row.getByRole('button', { name: 'เก็บถาวร', exact: true }).click();
	expect(api.writes.at(-1)!.payload.expectedRevision).toBe(2);
});

test('report exports only loaded scoped rows with spreadsheet formula escaping', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto('/staff/leave/reports?from=2026-10-01&to=2026-10-31');
	await expect(page.getByTestId('leave-usage')).toContainText('1.5 วัน');
	const downloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: 'ดาวน์โหลด CSV' }).click();
	const download = await downloadPromise;
	const stream = await download.createReadStream();
	const chunks: Buffer[] = [];
	for await (const chunk of stream!) chunks.push(Buffer.from(chunk));
	const csv = Buffer.concat(chunks).toString('utf8');
	expect(csv).toContain('"\'=ชื่อสูตรทดสอบ"');
	expect(api.count('/api/staff-leave/usage')).toBe(1);
	expect(api.count('/api/staff-leave/routing-options')).toBe(0);
});

test('different detail context clears the previous resource before pending first paint', async ({
	page
}) => {
	await mockLeave(page);
	await page.goto(leaveDetailPath);
	await expect(page.getByTestId('leave-detail')).toContainText('ลาป่วย');
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(
		(url) => url.pathname === `/api/staff-leave/requests/${secondLeaveId}`,
		async (route) => {
			await held;
			await route.fallback();
		}
	);
	await navigate(page, `/staff/leave/requests/${secondLeaveId}`);
	await expect(page.getByTestId('leave-detail')).not.toContainText('เข้ารับการตรวจรักษา');
	release();
	await expect(page.getByTestId('leave-detail')).toContainText('คำขอคนละรายการ');
});

for (const theme of ['light', 'dark'] as const)
	for (const width of [390, 1440]) {
		test(`detail print content and responsive ${theme} ${width}`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			await page.addInitScript((theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })), theme);
			await mockLeave(page, { role: 'reviewer', status: 'submitted' });
			await page.goto(leaveDetailPath);
			await expect(page.getByTestId('leave-detail')).toContainText('คำสั่งโรงเรียนทดสอบ');
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
			await page.screenshot({
				path: testInfo.outputPath(`leave-${theme}-${width}.png`),
				fullPage: true
			});
			await page.emulateMedia({ media: 'print' });
			await expect(page.locator('#staff-leave-print')).toContainText('ประวัติการดำเนินการ');
			await expect(page.locator('#staff-leave-print')).not.toContainText(leaveTypeId);
			await expect(page.locator('#staff-leave-print')).toContainText('ครูหัวหน้าฝ่าย ทดสอบ');
		});
	}
