import { test, expect, type Page } from '@playwright/test';
import {
	mockCorrespondence,
	detailPath,
	correspondenceId,
	signatureId
} from './fixtures/correspondence';
import type {
	ProcessActionRequest,
	SendCorrespondenceRequest
} from '../../src/lib/api/correspondence';
test.use({ serviceWorkers: 'block' });

async function draw(page: Page) {
	const surface = page.getByRole('img', { name: 'พื้นที่วาดลายมือชื่อ' });
	await surface.scrollIntoViewIfNeeded();
	const box = (await surface.boundingBox())!;
	await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.4);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.7, { steps: 6 });
	await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.3, { steps: 6 });
	await page.mouse.up();
}
async function openSource(page: Page) {
	await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
	await expect(page.getByRole('img', { name: 'ชั้นหมายเหตุบนเอกสาร' })).toBeVisible();
	await expect(page.locator('canvas')).toHaveAttribute('width', /\d+/);
}

test('create and send starts the actual document workflow without a manual work window', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await page.goto('/staff/correspondence/new');
	await page.getByLabel('เรื่อง *', { exact: true }).fill('หนังสือฉบับใหม่');
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('โรงเรียนทดสอบ');
	await page.getByLabel('เนื้อหา / ข้อความประกอบ').fill('ขอให้ดำเนินการตามหนังสือ');
	await page.getByRole('button', { name: 'สร้างฉบับร่าง', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	await expect(page.getByRole('heading', { name: 'หนังสือฉบับใหม่', exact: true })).toBeVisible();
	expect(api.count('/api/correspondence/options')).toBe(0);
	await page.getByRole('button', { name: 'เลือกผู้รับและส่งหนังสือ' }).click();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน' }).click();
	await page.getByRole('option', { name: 'เวียนหนังสือมาตรฐาน' }).click();
	await page.getByRole('button', { name: 'ผู้รับ', exact: true }).click();
	await page.getByRole('option', { name: 'ครูเจ้าของ ทดสอบ' }).click();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ', exact: true }).click();
	await page.getByRole('button', { name: 'ส่งหนังสือ', exact: true }).click();
	await expect(page.getByTestId('correspondence-detail')).toContainText('กำลังดำเนินการ');
	expect(api.writes.map((write) => write.path)).toEqual([
		'/api/correspondence',
		`/api/correspondence/${correspondenceId}/send`
	]);
	const sent = api.writes.at(-1)!.payload as SendCorrespondenceRequest;
	expect(sent.stepRecipients?.[0].recipients).toHaveLength(1);
	expect(api.count('/api/me/workflow-windows/manageable')).toBe(0);
});

test('reader opens detail without manager, signature or recipient option reads', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { permissions: ['correspondence.read.assigned'] });
	await page.goto(detailPath);
	await expect(page.getByTestId('correspondence-detail')).toContainText('ครูผู้รับคนที่สอง');
	expect(api.count('/api/correspondence/options')).toBe(0);
	expect(api.count('/api/correspondence/signatures')).toBe(0);
	expect(api.count(`/api/correspondence/${correspondenceId}/preview`)).toBe(0);
	await expect(page.getByTestId('correspondence-detail')).not.toContainText(correspondenceId);
});

for (const status of [503, 409])
	test(`drawn signature and annotation survive ${status} and confirm one revision-bound action`, async ({
		page
	}) => {
		const api = await mockCorrespondence(page, { failAction: status });
		await page.goto(detailPath);
		await openSource(page);
		await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
		await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('โปรดดำเนินการภายในวันศุกร์');
		await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		await page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true }).fill('ลงนามตรวจสอบแล้ว');
		await draw(page);
		await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect(page.getByTestId('correspondence-action')).toContainText(
			status === 409 ? 'เอกสารมีการเปลี่ยนแปลง' : 'บริการไม่พร้อม'
		);
		await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveValue(
			'ลงนามตรวจสอบแล้ว'
		);
		expect(await page.getByTestId('signature-pad').locator('path').count()).toBe(1);
		await expect(page.getByText('โปรดดำเนินการภายในวันศุกร์').first()).toBeVisible();
		if (status === 409) {
			await page.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด', exact: true }).click();
			await expect(
				page.getByText('รายการมีการเปลี่ยนแปลง กรุณาตรวจเอกสารและขั้นตอนล่าสุด')
			).toBeVisible();
			await expect(page.getByRole('button', { name: 'เปิดเอกสาร', exact: true })).toBeVisible();
			await openSource(page);
			await page
				.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง', exact: true })
				.click();
			await expect(
				page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ })
			).not.toBeChecked();
			await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveValue(
				'ลงนามตรวจสอบแล้ว'
			);
			expect(await page.getByTestId('signature-pad').locator('path').count()).toBe(1);
			await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
		}
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
		const payload = api.writes.at(-1)!.payload as ProcessActionRequest;
		expect(payload.signature?.method).toBe('drawn');
		expect(payload.signature?.documentRevision).toBe(status === 409 ? 2 : 1);
		expect(payload.signature?.contentHash).toBe((status === 409 ? 'c' : 'a').repeat(64));
		expect(payload.annotations?.[0].text).toBe('โปรดดำเนินการภายในวันศุกร์');
		expect(payload.signature?.strokes[0].points.length).toBeGreaterThan(2);
	});

test('saved private image signature works with keyboard selection and explicit confirmation', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกลายมือชื่อที่บันทึก' }).click();
	const picker = page.getByRole('button', { name: 'ลายมือชื่อของฉัน', exact: true });
	await picker.focus();
	await page.keyboard.press('Enter');
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(page.getByRole('img', { name: 'ภาพลายมือชื่อที่เลือก' })).toBeVisible();
	const confirm = page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ });
	await confirm.focus();
	await page.keyboard.press('Space');
	await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
	await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
	const payload = api.writes.at(-1)!.payload as ProcessActionRequest;
	expect(payload.signature?.method).toBe('image');
	expect(payload.signature?.assetId).toBe(signatureId);
	expect(payload.signature?.strokes).toEqual([]);
});

test('acknowledgement completes only this recipient while another recipient remains pending', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'acknowledge' });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'รับทราบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันรับทราบ', exact: true }).click();
	const step = page.getByTestId('correspondence-current-step');
	await expect(step).toContainText('ดำเนินการแล้ว');
	await expect(step).toContainText('ครูผู้รับคนที่สอง');
	await expect(step).toContainText('รอดำเนินการ');
	expect(api.detail().instance?.status).toBe('active');
	expect(
		api.detail().instance?.steps[0].recipients.filter((recipient) => recipient.completedAt)
	).toHaveLength(1);
});

test('manager-only and signer-only paths are denied before private reads', async ({ page }) => {
	const api = await mockCorrespondence(page, { permissions: ['correspondence.read.assigned'] });
	await page.goto('/staff/correspondence/templates');
	await expect(page).toHaveURL(/\/403/);
	expect(api.count('/api/process/templates')).toBe(0);
	await page.goto('/staff/correspondence/signatures');
	await expect(page).toHaveURL(/\/403/);
	expect(api.count('/api/correspondence/signatures')).toBe(0);
});

for (const theme of ['light', 'dark'])
	test(`mobile ${theme} has no page overflow and fullscreen returns to reachable action panel`, async ({
		page
	}) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.addInitScript((value) => {
			localStorage.setItem('theme', value);
			document.documentElement.classList.toggle('dark', value === 'dark');
		}, theme);
		await mockCorrespondence(page);
		await page.goto(detailPath);
		await openSource(page);
		await page.getByRole('button', { name: 'เต็มหน้าจอ', exact: true }).click();
		await expect(page.getByRole('dialog')).toBeVisible();
		await page.getByRole('button', { name: 'กลับสู่รายละเอียด' }).click();
		await expect(page.getByRole('dialog')).toHaveCount(0);
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		await expect(page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true })).toBeVisible();
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
	});

for (const rotation of [90, 270])
	test(`native PDF rotation ${rotation} renders native landscape page and canonical text layer`, async ({
		page
	}) => {
		await mockCorrespondence(page, { rotation });
		await page.goto(detailPath);
		await openSource(page);
		const dimensions = await page
			.locator('canvas')
			.evaluate((canvas) => ({ width: canvas.width, height: canvas.height }));
		expect(dimensions.width).toBeGreaterThan(dimensions.height);
		await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
		await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('ตรวจสอบ');
		await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
		await expect(page.locator('svg text').filter({ hasText: 'ตรวจสอบ' })).toHaveAttribute(
			'x',
			'100'
		);
		await page.getByRole('button', { name: 'หมุน 90°' }).click();
		await expect(page.locator('svg text').filter({ hasText: 'ตรวจสอบ' })).toHaveAttribute(
			'x',
			'100'
		);
	});

test('read stays distinct from signing and preserves an unsaved annotation draft', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	await page.goto(detailPath);
	await openSource(page);
	await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
	await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('หมายเหตุยังไม่ยืนยัน');
	await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
	await page.getByRole('button', { name: 'เปิดอ่าน', exact: true }).click();
	await expect(page.getByTestId('signature-pad')).toHaveCount(0);
	await expect(page.getByLabel('หลักฐานประกอบ', { exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'ยืนยันเปิดอ่าน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลงนาม', exact: true })).toBeVisible();
	await expect(page.locator('svg text').filter({ hasText: 'หมายเหตุยังไม่ยืนยัน' })).toBeVisible();
	const payload = api.writes.at(-1)!.payload as ProcessActionRequest;
	expect(payload.annotations).toEqual([]);
	expect(payload.signature).toBeUndefined();
	expect(
		api.detail().instance?.steps[0].recipients.every((recipient) => !recipient.completedAt)
	).toBe(true);
});

test('historical revision selection keeps current drafts and filters immutable annotation layers', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	await page.goto(detailPath);
	await openSource(page);
	await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
	await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('ร่างจากฉบับเดิม');
	await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
	api.advanceDocument();
	await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
	await page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true }).fill('เก็บข้อความในฟอร์ม');
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/work.ts';
		const { workStore } = await import(/* @vite-ignore */ modulePath);
		await workStore.refreshSilently();
	});
	await expect(
		page.getByText('รายการมีการเปลี่ยนแปลง กรุณาตรวจเอกสารและขั้นตอนล่าสุด')
	).toBeVisible();
	await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveValue(
		'เก็บข้อความในฟอร์ม'
	);
	await page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง' }).click();
	await page.getByRole('option', { name: /ฉบับ 1 ·/ }).click();
	await openSource(page);
	await expect(
		page.getByText('กำลังอ่านเอกสารย้อนหลัง เลือกฉบับปัจจุบันเพื่อดำเนินการหนังสือ')
	).toBeVisible();
	await expect(page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true })).toBeDisabled();
	await expect(page.locator('svg text').filter({ hasText: 'ร่างจากฉบับเดิม' })).toHaveCount(0);
	await page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง' }).click();
	await page.getByRole('option', { name: 'ฉบับปัจจุบัน (2)', exact: true }).click();
	await openSource(page);
	await expect(page.locator('svg text').filter({ hasText: 'ร่างจากฉบับเดิม' })).toBeVisible();
	await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveValue(
		'เก็บข้อความในฟอร์ม'
	);
});

test.describe('touch signing', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('live touch strokes are captured with normalized points and confirmation', async ({
		page,
		context
	}) => {
		const api = await mockCorrespondence(page);
		await page.goto(detailPath);
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		const pad = page.getByRole('img', { name: 'พื้นที่วาดลายมือชื่อ' });
		await pad.scrollIntoViewIfNeeded();
		const box = (await pad.boundingBox())!;
		const session = await context.newCDPSession(page);
		await session.send('Input.dispatchTouchEvent', {
			type: 'touchStart',
			touchPoints: [{ x: box.x + box.width * 0.2, y: box.y + box.height * 0.4 }]
		});
		await session.send('Input.dispatchTouchEvent', {
			type: 'touchMove',
			touchPoints: [{ x: box.x + box.width * 0.5, y: box.y + box.height * 0.7 }]
		});
		await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
		await expect(page.getByTestId('signature-pad').locator('path')).toHaveCount(1);
		await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
		const payload = api.writes.at(-1)!.payload as ProcessActionRequest;
		expect(payload.signature?.strokes[0].points.length).toBeGreaterThan(1);
		expect(
			payload.signature?.strokes[0].points.every(
				(point) => point.x >= 0 && point.x <= 1 && point.y >= 0 && point.y <= 1
			)
		).toBe(true);
	});
});

for (const theme of ['light', 'dark'])
	for (const width of [390, 1440])
		test(`visual signed document ${theme} ${width}`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 1000 });
			await page.addInitScript((value) => {
				localStorage.setItem('theme', value);
				document.documentElement.classList.toggle('dark', value === 'dark');
			}, theme);
			await mockCorrespondence(page);
			await page.goto(detailPath);
			await openSource(page);
			await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
			await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('ดำเนินการตามหนังสือ');
			await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
			await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
			await draw(page);
			await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
			await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
			await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
			await expect(
				page.locator('svg text').filter({ hasText: 'ดำเนินการตามหนังสือ' })
			).toBeVisible();
			const geometry = await page
				.getByTestId('document-workspace')
				.evaluate((element) => ({
					left: element.getBoundingClientRect().left,
					right: element.getBoundingClientRect().right,
					width: innerWidth,
					scroll: document.documentElement.scrollWidth
				}));
			expect(geometry.left).toBeGreaterThanOrEqual(0);
			expect(geometry.right).toBeLessThanOrEqual(geometry.width + 1);
			expect(geometry.scroll).toBeLessThanOrEqual(geometry.width);
			await page.screenshot({
				path: testInfo.outputPath(`signed-${theme}-${width}.png`),
				fullPage: true
			});
		});
