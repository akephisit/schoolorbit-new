import { test, expect, type Page } from '@playwright/test';
import {
	mockLeave,
	leaveDetailPath,
	leaveId,
	secondLeaveId,
	leaveTypeId,
	exitTypeId
} from './fixtures/staff-leave';
import { actor } from './fixtures/staff-home-route-data';
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

test('initial loading and background failure preserve usable request list', async ({ page }) => {
	const api = await mockLeave(page, { hold: 'requests' });
	await page.goto('/staff/leave');
	await expect(page.getByRole('status', { name: 'กำลังโหลดคำขอ' })).toBeVisible();
	api.release();
	await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
	let failed = false;
	await page.route(
		(url) => url.pathname === '/api/staff-leave/requests',
		async (route) => {
			if (failed) return route.fallback();
			failed = true;
			return route.fulfill({
				status: 503,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'รายการปรับปรุงยังไม่พร้อม' })
			});
		}
	);
	await revalidatePage(page);
	await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
	await expect(page.getByTestId('leave-requests')).toContainText('รายการปรับปรุงยังไม่พร้อม');
	await expect(page.getByRole('status', { name: 'กำลังโหลดคำขอ' })).toHaveCount(0);
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('leave-requests')).not.toContainText('รายการปรับปรุงยังไม่พร้อม');
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
	await page.getByRole('button', { name: 'ดูเกณฑ์ ลาป่วย', exact: true }).click();
	await expect(page.getByTestId('leave-policies')).toContainText('ต้องแนบตั้งแต่ 3 วัน');
	await page.getByRole('button', { name: 'สร้างฉบับใหม่จากฉบับนี้', exact: true }).click();
	await expect(page.locator('#policy-source')).toHaveValue('คำสั่งโรงเรียนทดสอบ 1/2569');
	await page.getByRole('tab', { name: 'จำนวน / การนับ', exact: true }).click();
	await expect(page.locator('#policy-max')).toHaveValue('5');
	expect(api.count('/api/staff-leave/routing-options')).toBe(0);
	await page.getByRole('tab', { name: 'ผู้พิจารณา', exact: true }).click();
	await expect(page.getByTestId('leave-policy-editor')).toContainText('ครูหัวหน้าฝ่าย ทดสอบ');
	expect(api.count('/api/staff-leave/routing-options')).toBe(1);
	await page.getByRole('tab', { name: 'ขอบเขต', exact: true }).click();
	await expect(page.locator('#policy-effective')).toContainText('เลือกวันที่');
	await chooseDate(page, 'policy-effective');
	await page.locator('#policy-source').fill('คำสั่งใหม่ 2/2569');
	await page.getByRole('button', { name: 'บันทึกเกณฑ์ฉบับใหม่', exact: true }).click();
	await expect(page.getByTestId('leave-policies')).toContainText('คำสั่งใหม่ 2/2569');
	await page.getByRole('button', { name: 'จัดการประเภท ลาป่วย', exact: true }).click();
	await page.getByRole('menuitem', { name: 'เก็บถาวร ลาป่วย', exact: true }).click();
	expect(api.writes.at(-1)!.payload.expectedRevision).toBe(2);
	expect(api.writes.at(-1)!.payload).not.toHaveProperty('code');
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
			await page.addInitScript(
				(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
				theme
			);
			const signedPrint = width === 1440 && theme === 'dark';
			await mockLeave(page, {
				role: 'reviewer',
				status: 'submitted',
				requireSignature: signedPrint
			});
			await page.goto(leaveDetailPath);
			if (signedPrint) {
				await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
				await draw(page);
				await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
				await page.getByRole('button', { name: 'ยืนยันอนุมัติ', exact: true }).click();
				await expect(
					page.getByRole('img', { name: 'ลายมือชื่อ ครูผู้พิจารณา ทดสอบ' })
				).toBeVisible();
			}
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
			await expect(page.getByText('ผู้ขอ', { exact: true })).toHaveCSS('color', 'rgb(0, 0, 0)');
			await expect(page.getByRole('button', { name: 'พิมพ์คำขอ' })).toBeHidden();
			await page.screenshot({
				path: testInfo.outputPath(`leave-print-${theme}-${width}.png`),
				fullPage: true
			});
			if (width === 1440 && theme === 'dark')
				await page.pdf({
					path: testInfo.outputPath('leave-print.pdf'),
					format: 'A4',
					printBackground: false
				});
		});
	}

test('stale draft reconciles only detail and requires accepting latest revision while preserving text', async ({
	page
}) => {
	const api = await mockLeave(page, { failMutation: 409 });
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'แก้ไขฉบับร่าง' }).click();
	await expect(page.getByTestId('leave-effective-policy')).toContainText('นโยบายฉบับ');
	await page.getByLabel('เหตุผล *', { exact: true }).fill('เหตุผลที่เก็บไว้หลังข้อมูลเก่า');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await page
		.getByTestId('leave-request-editor')
		.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด' })
		.click();
	await expect(page.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง' })).toBeVisible();
	await expect(page.getByLabel('เหตุผล *', { exact: true })).toHaveValue(
		'เหตุผลที่เก็บไว้หลังข้อมูลเก่า'
	);
	await page.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง' }).click();
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('เหตุผลที่เก็บไว้หลังข้อมูลเก่า');
	expect(api.count(`/api/staff-leave/requests/${leaveId}`)).toBe(2);
	expect(api.count('/api/staff-leave/options')).toBe(1);
	expect(api.writes[1].payload.expectedRevision).toBe(2);
	expect(api.writes[1].payload.requestId).not.toBe(api.writes[0].payload.requestId);
});

test('stale signed action preserves strokes and requires consent again on latest revision', async ({
	page
}) => {
	const api = await mockLeave(page, {
		role: 'reviewer',
		status: 'submitted',
		requireSignature: true,
		failMutation: 409
	});
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await draw(page);
	await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	await page
		.getByTestId('leave-action-form')
		.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด' })
		.click();
	await page.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง' }).click();
	await expect(page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ })).not.toBeChecked();
	await expect(page.getByTestId('signature-pad').locator('path')).toHaveCount(1);
	await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	expect(api.writes.at(-1)!.payload.signature).toMatchObject({ input: { documentRevision: 2 } });
	expect(api.count(`/api/staff-leave/requests/${leaveId}`)).toBe(2);
});

test('type deletion scopes retry keys to resource while identical failed delete retries same key', async ({
	page
}) => {
	const api = await mockLeave(page, { failMutation: 503 });
	await page.goto('/staff/leave/settings');
	await page.getByRole('button', { name: 'จัดการประเภท ลาป่วย', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ลบ ลาป่วย', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันลบประเภท' }).click();
	await expect(page.getByRole('alertdialog')).toContainText('บริการไม่พร้อม');
	await page.getByRole('button', { name: 'ยืนยันลบประเภท' }).click();
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'จัดการประเภท ออกนอกโรงเรียน', exact: true }).click();
	await page.getByRole('menuitem', { name: 'ลบ ออกนอกโรงเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันลบประเภท' }).click();
	await expect(page.getByTestId('leave-types')).toContainText('ยังไม่มีประเภทคำขอ');
	expect(api.writes[0].payload.requestId).toBe(api.writes[1].payload.requestId);
	expect(api.writes[2].payload.requestId).not.toBe(api.writes[1].payload.requestId);
	expect(api.writes[1].payload).toMatchObject({ expectedRevision: 1 });
	expect(api.writes[2].path).toContain(exitTypeId);
	expect(api.writes[2].payload).not.toHaveProperty('typeId');
});

const syntheticPng = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==',
	'base64'
);
test('keyboard image alternative previews signature and history reads it in request context', async ({
	page
}) => {
	const api = await mockLeave(page, {
		role: 'reviewer',
		status: 'submitted',
		requireSignature: true
	});
	await page.goto(leaveDetailPath);
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	const method = page.getByRole('radio', { name: 'อัปโหลดภาพลายมือชื่อ', exact: true });
	await method.focus();
	await page.keyboard.press('Space');
	await page
		.locator('#leave-signature-image')
		.setInputFiles({ name: 'signature.png', mimeType: 'image/png', buffer: syntheticPng });
	await expect(page.getByRole('img', { name: 'ภาพลายมือชื่อก่อนยืนยัน' })).toBeVisible();
	const consent = page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ });
	await consent.focus();
	await page.keyboard.press('Space');
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	await expect(page.getByRole('img', { name: 'ลายมือชื่อ ครูผู้พิจารณา ทดสอบ' })).toBeVisible();
	expect(api.uploads).toEqual([{ purpose: 'staff_leave_signature', resourceId: actor }]);
	expect(api.fileReads[0].resourceId).toBe(leaveId);
	expect(api.writes.at(-1)!.payload.signature).toMatchObject({
		consent: true,
		imageFileId: expect.any(String),
		input: { method: 'image', strokes: [], assetId: null }
	});
});

test('evidence upload is lazy and stays linked after save then can be removed from new draft', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto(leaveDetailPath);
	expect(api.uploads).toHaveLength(0);
	await page.getByRole('button', { name: 'แก้ไขฉบับร่าง' }).click();
	await expect(page.getByTestId('leave-effective-policy')).toContainText('นโยบายฉบับ');
	await page
		.locator('#leave-evidence')
		.setInputFiles({ name: 'evidence.png', mimeType: 'image/png', buffer: syntheticPng });
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง' }).click();
	await expect(page.getByRole('button', { name: 'ดาวน์โหลดหลักฐาน 1' })).toBeVisible();
	expect(api.uploads).toEqual([{ purpose: 'staff_leave_evidence', resourceId: leaveId }]);
	await page.getByRole('button', { name: 'แก้ไขฉบับร่าง' }).click();
	await page.getByRole('button', { name: 'นำออกจากฉบับร่าง' }).click();
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง' }).click();
	await expect(page.getByRole('button', { name: 'ดาวน์โหลดหลักฐาน 1' })).toHaveCount(0);
	expect(api.writes.at(-1)!.payload.period).toMatchObject({ evidenceFileIds: [] });
});

for (const theme of ['light', 'dark'] as const)
	for (const width of [390, 1440]) {
		test(`list editor settings and report responsive ${theme} ${width}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			await page.addInitScript(
				(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
				theme
			);
			const api = await mockLeave(page);
			await page.goto('/staff/leave');
			await expect(page.getByTestId('leave-requests')).toContainText('ลาป่วย');
			await page.screenshot({ path: testInfo.outputPath(`list-${theme}-${width}.png`) });
			await page.getByRole('button', { name: 'สร้างคำขอการลา' }).click();
			await page.locator('#leave-type').click();
			await page.getByRole('option', { name: 'ลาป่วย', exact: true }).click();
			await chooseDate(page, 'leave-start-date');
			await expect(page.getByTestId('leave-effective-policy')).toContainText('คำสั่งโรงเรียนทดสอบ');
			await page.screenshot({ path: testInfo.outputPath(`editor-${theme}-${width}.png`) });
			await page
				.getByTestId('leave-request-editor')
				.getByRole('button', { name: 'ยกเลิก', exact: true })
				.click();
			await navigate(page, '/staff/leave/settings');
			await page.getByRole('button', { name: 'ดูเกณฑ์ ลาป่วย', exact: true }).click();
			await page.screenshot({ path: testInfo.outputPath(`settings-${theme}-${width}.png`) });
			await page.getByRole('button', { name: 'เพิ่มประเภทคำขอ', exact: true }).click();
			await expect(page.getByTestId('leave-type-form')).not.toContainText('รหัส');
			await page.screenshot({ path: testInfo.outputPath(`type-form-${theme}-${width}.png`) });
			await page
				.getByTestId('leave-type-form')
				.getByRole('button', { name: 'ยกเลิก', exact: true })
				.click();
			await page.getByRole('button', { name: 'สร้างฉบับใหม่จากฉบับนี้' }).click();
			await expect(page.locator('#policy-source')).toHaveValue('คำสั่งโรงเรียนทดสอบ 1/2569');
			await page.screenshot({ path: testInfo.outputPath(`policy-${theme}-${width}.png`) });
			await page.getByRole('tab', { name: 'จำนวน / การนับ', exact: true }).click();
			await page.screenshot({ path: testInfo.outputPath(`policy-limits-${theme}-${width}.png`) });
			await page.getByRole('tab', { name: 'หลักฐาน', exact: true }).click();
			await page.screenshot({ path: testInfo.outputPath(`policy-evidence-${theme}-${width}.png`) });
			await page.getByRole('tab', { name: 'ผู้พิจารณา', exact: true }).click();
			await expect(page.getByTestId('leave-policy-editor')).toContainText('ครูหัวหน้าฝ่าย ทดสอบ');
			await page.screenshot({ path: testInfo.outputPath(`policy-steps-${theme}-${width}.png`) });
			await page
				.getByTestId('leave-policy-editor')
				.getByRole('button', { name: 'ยกเลิก', exact: true })
				.click();
			await navigate(page, '/staff/exit-permissions');
			await expect(page.getByTestId('leave-requests')).toContainText('90 นาที');
			await page.screenshot({ path: testInfo.outputPath(`exit-list-${theme}-${width}.png`) });
			await page.getByRole('button', { name: 'สร้างคำขอออกนอกโรงเรียน', exact: true }).click();
			await page.locator('#leave-type').click();
			await page.getByRole('option', { name: 'ออกนอกโรงเรียน', exact: true }).click();
			await chooseDate(page, 'leave-start-date');
			await expect(page.locator('#leave-start-time')).toBeVisible();
			await page.screenshot({ path: testInfo.outputPath(`exit-editor-${theme}-${width}.png`) });
			await page
				.getByTestId('leave-request-editor')
				.getByRole('button', { name: 'ยกเลิก', exact: true })
				.click();
			await navigate(page, '/staff/leave/reports');
			await expect(page.getByTestId('leave-usage')).toContainText('1.5 วัน');
			await expect(page.getByText(/คำขอที่เริ่มในช่วงวันที่เลือก/)).toBeVisible();
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({ path: testInfo.outputPath(`report-${theme}-${width}.png`) });
			expect(api.writes).toHaveLength(0);
		});
	}

test('school type needs only name and group and keeps internal code out of all mutations', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto('/staff/leave/settings');
	await page.getByRole('button', { name: 'เพิ่มประเภทคำขอ' }).click();
	await expect(page.getByTestId('leave-type-form')).not.toContainText('รหัส');
	await expect(page.locator('#leave-type-code')).toHaveCount(0);
	await page.locator('#leave-type-name').fill('ลาตามนโยบายโรงเรียน');
	await page.getByRole('button', { name: 'บันทึกประเภท', exact: true }).click();
	await expect(page.getByTestId('leave-types')).toContainText('ลาตามนโยบายโรงเรียน');
	expect(api.writes.at(-1)!.payload).not.toHaveProperty('code');
	await page.getByRole('button', { name: 'จัดการประเภท ลาตามนโยบายโรงเรียน', exact: true }).click();
	await page.getByRole('menuitem', { name: 'แก้ไข ลาตามนโยบายโรงเรียน', exact: true }).click();
	await expect(page.locator('#leave-type-code')).toHaveCount(0);
	await expect(page.getByRole('radio', { name: 'ออกนอกโรงเรียน', exact: true })).toBeDisabled();
	await page.locator('#leave-type-name').fill('ลาตามประกาศโรงเรียน');
	await page.getByRole('button', { name: 'บันทึกประเภท', exact: true }).click();
	await expect(page.getByTestId('leave-types')).toContainText('ลาตามประกาศโรงเรียน');
	expect(api.count('/api/staff-leave/types')).toBe(1);
	expect(api.writes.at(-1)!.payload).not.toHaveProperty('code');
	await expect(page.getByTestId('leave-types')).not.toContainText('sick');
});

test('exit menu uses planned times and the editable school-local minute profile', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto('/staff/exit-permissions');
	await expect(page.getByTestId('leave-requests')).toContainText('90 นาที');
	await page.getByRole('button', { name: 'สร้างคำขอออกนอกโรงเรียน' }).click();
	await page.locator('#leave-type').click();
	await page.getByRole('option', { name: 'ออกนอกโรงเรียน', exact: true }).click();
	await chooseDate(page, 'leave-start-date');
	await expect(page.getByTestId('leave-effective-policy')).toContainText('120 นาที');
	await page.locator('#leave-start-time').fill('09:00');
	await page.locator('#leave-end-time').fill('10:30');
	await page.getByLabel('เหตุผล *', { exact: true }).fill('ประสานงานราชการตามกำหนด');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง' }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('90 นาที');
	expect(api.writes[0].payload.period).toMatchObject({
		typeId: exitTypeId,
		startTime: '09:00:00',
		endTime: '10:30:00',
		dayPortion: 'full_day'
	});
	await expect(page.getByText(/กลับเข้าโรงเรียน|มาสาย|เกินเวลา/)).toHaveCount(0);
});

for (const action of ['ส่งกลับแก้ไข', 'ไม่อนุมัติ'])
	test(`reviewer ${action} needs a reason without a signature`, async ({ page }) => {
		const api = await mockLeave(page, {
			role: 'reviewer',
			status: 'submitted',
			requireSignature: true
		});
		await page.goto(leaveDetailPath);
		await page.getByRole('button', { name: action, exact: true }).click();
		await page.getByRole('button', { name: `ยืนยัน${action}`, exact: true }).click();
		await expect(page.getByTestId('leave-action-form')).toContainText('กรอกความคิดเห็น / เหตุผล');
		expect(api.writes).toHaveLength(0);
		await page
			.getByLabel('ความคิดเห็น / เหตุผล *', { exact: true })
			.fill('กรุณาตรวจสอบข้อมูลประกอบ');
		await page.getByRole('button', { name: `ยืนยัน${action}`, exact: true }).click();
		await expect(page.getByTestId('leave-detail')).toContainText(
			action === 'ส่งกลับแก้ไข' ? 'ส่งกลับแก้ไข' : 'ไม่อนุมัติ'
		);
		expect(api.writes.at(-1)!.payload.signature).toBeNull();
	});

test('approved cancellation uses its current workflow and keeps original policy basis visible', async ({
	page
}) => {
	await mockLeave(page, { role: 'reviewer', status: 'cancellation_pending' });
	await page.goto(leaveDetailPath);
	await expect(page.getByTestId('leave-detail')).toContainText('เกณฑ์ของคำขอเดิม');
	await expect(page.getByTestId('leave-detail')).toContainText('ขั้นตอนพิจารณาการยกเลิก');
	await page.getByRole('button', { name: 'อนุมัติ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันอนุมัติ' }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('ยกเลิกแล้ว');
	await expect(page.getByTestId('leave-detail')).toContainText('อนุมัติการยกเลิกคำขอ');
});

test('policy sections retain all edits and reveal hidden invalid fields before saving', async ({
	page
}) => {
	const api = await mockLeave(page);
	await page.goto('/staff/leave/settings');
	await page.getByRole('button', { name: 'ดูเกณฑ์ ลาป่วย', exact: true }).click();
	await page.getByRole('button', { name: 'สร้างฉบับใหม่จากฉบับนี้', exact: true }).click();
	await chooseDate(page, 'policy-effective');
	await page.locator('#policy-source').fill('เกณฑ์ทดสอบการเก็บฉบับร่าง');
	await page.getByRole('tab', { name: 'จำนวน / การนับ', exact: true }).click();
	await page.locator('#policy-max').fill('7.5');
	await page.getByRole('tab', { name: 'หลักฐาน', exact: true }).click();
	await page.locator('#policy-notice').fill('-1');
	await page.getByRole('tab', { name: 'ขอบเขต', exact: true }).click();
	await expect(page.locator('#policy-source')).toHaveValue('เกณฑ์ทดสอบการเก็บฉบับร่าง');
	await page.getByRole('button', { name: 'บันทึกเกณฑ์ฉบับใหม่', exact: true }).click();
	await expect(page.getByRole('tab', { name: 'หลักฐาน', exact: true })).toHaveAttribute(
		'aria-selected',
		'true'
	);
	await expect(page.locator('#policy-notice')).toBeFocused();
	await expect(page.getByTestId('leave-policy-editor')).toContainText(
		'จำนวนวันที่ยื่นล่วงหน้าต้องอยู่ระหว่าง 0 ถึง 365'
	);
	expect(api.writes).toHaveLength(0);
	await page.locator('#policy-notice').fill('2');
	await page.getByRole('tab', { name: 'ผู้พิจารณา', exact: true }).click();
	await page.locator('#policy-step-title-0').fill('');
	await page.getByRole('tab', { name: 'ขอบเขต', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกเกณฑ์ฉบับใหม่', exact: true }).click();
	await expect(page.getByRole('tab', { name: 'ผู้พิจารณา', exact: true })).toHaveAttribute(
		'aria-selected',
		'true'
	);
	await expect(page.locator('#policy-step-title-0')).toBeFocused();
	await page.locator('#policy-step-title-0').fill('ผู้พิจารณาตามเกณฑ์ใหม่');
	const limitsTab = page.getByRole('tab', { name: 'จำนวน / การนับ', exact: true });
	await limitsTab.focus();
	await page.keyboard.press('Enter');
	await expect(page.locator('#policy-max')).toHaveValue('7.5');
	await page.getByRole('button', { name: 'บันทึกเกณฑ์ฉบับใหม่', exact: true }).click();
	await expect(page.getByTestId('leave-policies')).toContainText('เกณฑ์ทดสอบการเก็บฉบับร่าง');
	expect(api.writes.at(-1)!.payload).toMatchObject({
		sourceReference: 'เกณฑ์ทดสอบการเก็บฉบับร่าง',
		maxPerRequest: 15,
		minimumNoticeDays: 2,
		steps: [expect.objectContaining({ title: 'ผู้พิจารณาตามเกณฑ์ใหม่' })]
	});
});

test('phone records open without horizontal scrolling and reviewer action precedes long criteria', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mockLeave(page, { role: 'reviewer', status: 'submitted' });
	await page.goto('/staff/leave');
	const records = page.getByRole('list', { name: 'รายการคำขอ', exact: true });
	await expect(records).toContainText('ลาป่วย');
	await expect(records).toContainText('รอพิจารณา');
	await expect(records).toContainText('ครู');
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await records.getByRole('link', { name: 'เปิดคำขอ', exact: true }).click();
	const review = page.getByRole('button', { name: 'อนุมัติ', exact: true });
	await expect(review).toBeVisible();
	const actionBox = await review.boundingBox();
	const criteriaBox = await page.getByText('เกณฑ์ที่ใช้กับคำขอ', { exact: true }).boundingBox();
	expect(actionBox!.y).toBeLessThan(criteriaBox!.y);
	await review.focus();
	await page.keyboard.press('Enter');
	await expect(page.getByTestId('leave-action-form')).toBeVisible();
});

test('empty list and report distinguish filtered emptiness without invented usage', async ({
	page
}) => {
	await mockLeave(page, { empty: true });
	await page.goto('/staff/leave?status=returned');
	await expect(page.getByTestId('leave-requests')).toContainText('ไม่พบคำขอตามสถานะที่เลือก');
	await navigate(page, '/staff/leave/reports');
	await expect(page.getByTestId('leave-usage')).toContainText('ยังไม่มีการใช้สิทธิ์ในช่วงที่เลือก');
	await expect(page.getByTestId('leave-usage').getByRole('table')).toHaveCount(0);
});

test('pending submit disables repeated mutations and keeps the request summary visible', async ({
	page
}) => {
	const api = await mockLeave(page, { hold: 'mutation' });
	await page.goto(leaveDetailPath);
	await expect(page.getByTestId('leave-detail')).toContainText('เข้ารับการตรวจรักษา');
	await page.getByRole('button', { name: 'ส่งคำขอพิจารณา', exact: true }).click();
	try {
		await expect(
			page.getByRole('button', { name: 'กำลังดำเนินการ...', exact: true })
		).toBeDisabled();
		await expect(page.getByRole('button', { name: 'แก้ไขฉบับร่าง', exact: true })).toBeDisabled();
		await expect(page.getByTestId('leave-detail')).toContainText('เข้ารับการตรวจรักษา');
		expect(api.writes.filter((write) => write.path.endsWith('/submit'))).toHaveLength(1);
	} finally {
		api.release();
	}
	await expect(page.getByTestId('leave-detail')).toContainText('รอพิจารณา');
	expect(api.writes.filter((write) => write.path.endsWith('/submit'))).toHaveLength(1);
});
