import { expect, test } from '@playwright/test';
import { mockCorrespondence, correspondenceId, detailPath } from './fixtures/correspondence';
import { mockDirectedCorrespondence } from './fixtures/correspondence-directed';

test.use({ serviceWorkers: 'block' });

test('current PDF opens automatically and a failed preview has a focused retry', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	let attempts = 0;
	await page.route('**/api/correspondence/*/preview**', (route) => {
		attempts++;
		return attempts === 1
			? route.fulfill({
					status: 503,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'PDF ยังไม่พร้อม' })
				})
			: route.fallback();
	});
	await page.goto(detailPath);
	await expect(page.getByRole('button', { name: 'เปิดเอกสาร', exact: true })).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: 'ลองเปิดเอกสารอีกครั้ง', exact: true })
	).toBeVisible();
	await expect(page.locator('canvas')).toHaveCount(0);
	expect(api.readRequests).toHaveLength(0);
	await page.getByRole('button', { name: 'ลองเปิดเอกสารอีกครั้ง', exact: true }).click();
	await expect(page.locator('canvas')).toBeVisible();
	expect(attempts).toBe(2);
	await expect.poll(() => api.readRequests.length).toBe(1);
	expect(api.writes).toHaveLength(0);
});

test('leaving while preview is pending discards the old document response', async ({ page }) => {
	const api = await mockCorrespondence(page);
	let release = () => {};
	const pending = new Promise<void>((resolve) => (release = resolve));
	let started = false;
	await page.route('**/api/correspondence/*/preview**', async (route) => {
		started = true;
		await pending;
		await route.fallback();
	});
	await page.goto(detailPath);
	await expect.poll(() => started).toBe(true);
	await expect(page.getByRole('status', { name: 'กำลังเปิดเอกสาร', exact: true })).toBeVisible();
	expect(api.readRequests).toHaveLength(0);
	await page.goto('/staff/correspondence');
	release();
	await expect(page.getByTestId('correspondence-register')).toBeVisible();
	await expect(page.locator('canvas')).toHaveCount(0);
	expect(api.readRequests).toHaveLength(0);
	expect(api.writes).toHaveLength(0);
});

test('metadata validation reveals the exact field and preserves its value', async ({ page }) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await page.goto('/staff/correspondence/new');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.locator('#create-title')).toBeFocused();
	await expect(page.locator('#create-title')).toHaveAttribute('aria-invalid', 'true');
	await expect(page.locator('#create-title-error')).toContainText('กรอกเรื่อง');
	await page.locator('#create-title').fill('เก็บเรื่องที่กรอก');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.locator('#create-sender')).toBeFocused();
	await page.locator('#create-sender').fill('หน่วยงานต้นทาง');
	await page
		.getByText('ข้อมูลเพิ่มเติม (เลขที่ต้นทาง วันที่ และหน่วยงาน)', { exact: true })
		.click();
	await page.locator('#create-number').fill('ก'.repeat(151));
	await page
		.getByText('ข้อมูลเพิ่มเติม (เลขที่ต้นทาง วันที่ และหน่วยงาน)', { exact: true })
		.click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.locator('#create-number')).toBeFocused();
	await expect(page.locator('#create-number')).toHaveValue('ก'.repeat(151));
	await expect(page.locator('#create-number-error')).toContainText('150');
	await expect(page.locator('#create-title')).toHaveValue('เก็บเรื่องที่กรอก');
	expect(api.writes).toHaveLength(0);
});

test('action validation focuses comments then the missing signature without submitting', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	api.detail().instance!.steps[0].definition.requiresComment = true;
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
	await expect(page.locator('#action-comment')).toBeFocused();
	await expect(page.locator('#action-comment-error')).toContainText('ความคิดเห็น');
	await page.locator('#action-comment').fill('เก็บความเห็นของฉัน');
	await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
	await expect(page.locator('#signature-input')).toBeFocused();
	await expect(page.locator('#signature-input-error')).toContainText('ลายมือชื่อ');
	await expect(page.locator('#action-comment')).toHaveValue('เก็บความเห็นของฉัน');
	expect(api.writes).toHaveLength(0);
});

test('send validation focuses the missing decision maker and exact missing recipient step', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'เลือกผู้รับและส่งหนังสือ', exact: true }).click();
	await page.getByRole('button', { name: 'ส่งหนังสือ', exact: true }).click();
	await expect(page.locator('#send-first-decisionmaker')).toBeFocused();
	await expect(page.locator('#send-first-decisionmaker')).toHaveAttribute('aria-invalid', 'true');
	await page.getByRole('radio', { name: 'ใช้แม่แบบลำดับงาน', exact: true }).click();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน', exact: true }).click();
	await page.getByRole('option', { name: 'เวียนหนังสือมาตรฐาน', exact: true }).click();
	await page.getByRole('button', { name: 'ส่งหนังสือ', exact: true }).click();
	await expect(page.locator('#send-step-0-recipients')).toBeFocused();
	await expect(page.locator('#send-step-0-recipients-error')).toContainText('ผู้รับ');
	expect(api.writes).toHaveLength(0);
});

test('routing settings focus the missing reviewer while keeping the selected register', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page);
	await page.goto('/staff/correspondence/settings');
	await page
		.getByRole('checkbox', { name: 'เสนอให้ตรวจทานผลเป็นค่าเริ่มต้น', exact: true })
		.check();
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.locator('#routing-default-reviewer')).toBeFocused();
	await expect(page.locator('#routing-default-reviewer-error')).toContainText('ผู้ตรวจทาน');
	expect(api.settingsWrites).toHaveLength(0);
});

for (const width of [390, 1440])
	for (const theme of ['light', 'dark'] as const) {
		test(`reader document precedes actions ${width}px ${theme}`, async ({ page }, info) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme: theme });
			const api = await mockCorrespondence(page);
			await page.goto('/staff/correspondence');
			await page
				.getByRole('link', { name: 'เปิดอ่าน', exact: true })
				.filter({ visible: true })
				.click();
			await expect(page).toHaveURL(new RegExp(correspondenceId));
			await expect(page.locator('canvas')).toBeVisible();
			await expect(
				page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง', exact: true })
			).toHaveCount(0);
			await expect.poll(() => api.readRequests.length).toBe(1);
			await expect(page.getByRole('radio', { name: 'ข้อความ', exact: true })).toHaveCount(0);
			const documentBox = await page.getByTestId('document-workspace').boundingBox();
			const action = await page.getByTestId('correspondence-current-step').boundingBox();
			if (width === 390) expect(documentBox!.y).toBeLessThan(action!.y);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: info.outputPath('reader.png'),
				fullPage: true,
				animations: 'disabled'
			});
			expect(api.writes).toHaveLength(0);
		});
	}

test('an invalid PDF never records reading before the renderer succeeds', async ({ page }) => {
	const api = await mockCorrespondence(page);
	await page.route('**/api/correspondence/*/preview**', (route) =>
		route.fulfill({ contentType: 'application/pdf', body: 'invalid synthetic PDF' })
	);
	await page.goto(detailPath);
	await expect(
		page.getByRole('button', { name: 'ลองเปิดเอกสารอีกครั้ง', exact: true })
	).toBeVisible();
	expect(api.readRequests).toHaveLength(0);
	expect(api.writes).toHaveLength(0);
});

test('read failure retries independently and keeps first receipt separate from acknowledgement', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'acknowledge', failRead: true });
	await page.goto(detailPath);
	await expect(page.locator('canvas')).toBeVisible();
	await expect(page.getByRole('button', { name: 'รับทราบ', exact: true })).toBeEnabled();
	await page.getByRole('button', { name: 'ลองบันทึกการเปิดอ่านอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('correspondence-read-receipt')).toBeVisible();
	expect(api.readRequests).toEqual([{ documentRevision: 1 }, { documentRevision: 1 }]);
	expect(api.detail().record.revision).toBe(1);
	expect(api.detail().history).toHaveLength(0);
	expect(
		api.detail().instance?.steps[0].recipients.every((recipient) => !recipient.completedAt)
	).toBe(true);
	expect(api.writes).toHaveLength(0);
});

test('an existing receipt still projects a newly active assignment without changing its first read time', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'acknowledge' });
	const receipt = api.recordRead(1);
	await page.goto(detailPath);
	await expect.poll(() => api.readRequests.length).toBe(1);
	await expect(page.getByTestId('correspondence-read-receipt')).toBeVisible();
	expect(api.detail().myReadReceipt).toEqual(receipt);
	expect(api.writes).toHaveLength(0);
});

test('a newly active slot supersedes a pending read on the same rendered document', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'acknowledge', nextStepAfterAccept: true });
	let attempts = 0,
		release = () => {};
	const pending = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/correspondence/*/read', async (route) => {
		attempts++;
		if (attempts === 1) await pending;
		await route.fallback();
	});
	await page.goto(detailPath);
	await expect.poll(() => attempts).toBe(1);
	await page.getByRole('button', { name: 'รับทราบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันรับทราบ', exact: true }).click();
	await expect.poll(() => attempts).toBe(2);
	await expect(page.getByTestId('correspondence-read-receipt')).toBeVisible();
	release();
	expect(api.count(`/api/correspondence/${correspondenceId}/preview`)).toBe(1);
	expect(api.detail().instance?.currentStep).toBe(1);
	expect(api.writes).toHaveLength(1);
	expect(api.writes[0].payload).toMatchObject({ action: 'acknowledge' });
});
