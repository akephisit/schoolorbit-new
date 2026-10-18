import type { ProcessActionRequest } from '../../src/lib/api/correspondence.js';
import { expect, test, type Page } from '@playwright/test';
import { mockCorrespondence, detailPath, openAnnotationTools } from './fixtures/correspondence';
import { mockDirectedCorrespondence } from './fixtures/correspondence-directed';

test.use({ serviceWorkers: 'block' });

async function stampDialog(page: Page) {
	await expect(page.locator('canvas')).toBeVisible();
	await openAnnotationTools(page);
	await page.getByRole('button', { name: 'แม่แบบข้อความ / ตรารับ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'แม่แบบข้อความ', exact: true })).toContainText(
		'ตรารับหนังสือ'
	);
	return dialog;
}
async function draw(page: Page) {
	const surface = page.getByRole('img', { name: 'พื้นที่วาดลายมือชื่อ' });
	await surface.scrollIntoViewIfNeeded();
	const box = (await surface.boundingBox())!;
	await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.3);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.7, { steps: 6 });
	await page.mouse.up();
}

for (const rotation of [0, 90]) {
	test(`bordered receipt text moves and resizes at ${rotation} degrees and changing it revokes signature consent`, async ({
		page
	}, info) => {
		const api = await mockCorrespondence(page, { rotation });
		await page.goto(detailPath);
		const dialog = await stampDialog(page);
		await expect(dialog.getByLabel('ชื่อโรงเรียน *', { exact: true })).toHaveValue(
			'โรงเรียนตัวอย่าง'
		);
		await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).click();
		await expect(dialog.getByLabel('ข้อความที่จะวาง *', { exact: true })).toHaveValue(
			new RegExp('18/2569')
		);
		await dialog.getByRole('button', { name: 'วางบนเอกสาร', exact: true }).click();
		const frame = page.getByTestId('text-annotation-placement');
		await expect(frame).toBeVisible();
		await expect(page.locator('svg rect[stroke-width="0.75"]')).toHaveCount(1);
		if (rotation === 90) await page.getByRole('button', { name: 'หมุน 90°', exact: true }).click();
		await frame.scrollIntoViewIfNeeded();
		const before = (await frame.boundingBox())!;
		await frame.getByRole('button', { name: /^เลื่อนกรอบข้อความ/ }).focus();
		await page.keyboard.press('ArrowRight');
		const moved = (await frame.boundingBox())!;
		expect(Math.abs(moved.x - before.x) + Math.abs(moved.y - before.y)).toBeGreaterThan(1);
		await frame.getByRole('button', { name: /^ปรับขนาดกรอบข้อความ/ }).focus();
		await page.keyboard.press('ArrowDown');
		const resized = (await frame.boundingBox())!;
		expect(
			Math.abs(resized.width - moved.width) + Math.abs(resized.height - moved.height)
		).toBeGreaterThan(1);
		await page.getByRole('button', { name: 'แก้ไขข้อความ', exact: true }).click();
		await page.getByLabel('ข้อความ *', { exact: true }).fill('ข้อความ'.repeat(1000));
		await page.getByRole('button', { name: 'ใช้ข้อความนี้', exact: true }).click();
		await expect(page.locator('#edit-annotation-text')).toBeFocused();
		await expect(page.locator('#edit-annotation-text-error')).toBeVisible();
		await page
			.getByLabel('ข้อความ *', { exact: true })
			.fill('โรงเรียนตัวอย่าง\nเลขรับ 18/2569\nตรวจสอบตรารับ');
		await page.getByRole('button', { name: 'ใช้ข้อความนี้', exact: true }).click();
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		await draw(page);
		const confirmation = page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ });
		await confirmation.check();
		await frame.getByRole('button', { name: /^เลื่อนกรอบข้อความ/ }).focus();
		await page.keyboard.press('ArrowLeft');
		await expect(confirmation).not.toBeChecked();
		await confirmation.check();
		await page.screenshot({
			path: info.outputPath('bordered-text.png'),
			fullPage: true,
			animations: 'disabled'
		});
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
		const annotation = (api.writes[0].payload as ProcessActionRequest).annotations![0];
		expect(annotation).toMatchObject({
			kind: 'text',
			border: true,
			text: 'โรงเรียนตัวอย่าง\nเลขรับ 18/2569\nตรวจสอบตรารับ',
			page: 1
		});
		expect(annotation.id).toMatch(/^[\da-f]{8}-[\da-f]{4}-7[\da-f]{3}-[89ab]/);
		expect(annotation.x + annotation.width).toBeLessThanOrEqual(1);
		expect(annotation.y + annotation.height).toBeLessThanOrEqual(1);
	});
}

test('instruction and unknown position need explicit input before concrete text review', async ({
	page
}) => {
	const api = await mockCorrespondence(page, {
		stampContext: {
			schoolName: 'โรงเรียนปัจจุบัน',
			actorDisplayName: 'ผู้ดำเนินการปัจจุบัน',
			signerTitle: null
		}
	});
	await page.goto(detailPath);
	const dialog = await stampDialog(page);
	await dialog.getByRole('button', { name: 'แม่แบบข้อความ', exact: true }).click();
	await page.getByRole('option', { name: 'ข้อความสั่งการ', exact: true }).click();
	await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).click();
	await expect(dialog.locator('#stamp-instruction')).toBeFocused();
	await dialog.getByLabel('ข้อความสั่งการ *', { exact: true }).fill('เห็นควรแจ้งผู้เกี่ยวข้อง');
	await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).click();
	await expect(dialog.locator('#stamp-title')).toBeFocused();
	await dialog.getByLabel('ตำแหน่งที่แสดง *', { exact: true }).fill('ตำแหน่งที่ผู้ใช้ระบุ');
	await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).click();
	await expect(dialog.getByLabel('ข้อความที่จะวาง *', { exact: true })).toHaveValue(
		new RegExp('ผู้ดำเนินการปัจจุบัน')
	);
	await expect(dialog.getByLabel('ข้อความที่จะวาง *', { exact: true })).toHaveValue(
		new RegExp('ตำแหน่งที่ผู้ใช้ระบุ')
	);
	await dialog.getByRole('button', { name: 'วางบนเอกสาร', exact: true }).click();
	await expect(
		page.locator('svg text').filter({ hasText: 'เห็นควรแจ้งผู้เกี่ยวข้อง' })
	).toBeVisible();
	expect(api.writes).toHaveLength(0);
	await expect.poll(() => api.readRequests.length).toBe(1);
});

test('missing school name is filled in a simple field without editing token syntax', async ({
	page
}) => {
	await mockCorrespondence(page, {
		stampContext: { schoolName: null, actorDisplayName: 'ผู้ดำเนินการปัจจุบัน', signerTitle: null }
	});
	await page.goto(detailPath);
	const dialog = await stampDialog(page);
	await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).click();
	await expect(dialog.locator('#stamp-school')).toBeFocused();
	await dialog.getByLabel('ชื่อโรงเรียน *', { exact: true }).fill('โรงเรียนที่ผู้ใช้ระบุ');
	await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).click();
	await expect(dialog.getByLabel('ข้อความที่จะวาง *', { exact: true })).toHaveValue(
		new RegExp('โรงเรียนที่ผู้ใช้ระบุ')
	);
});

test('number presets and editable template tokens preserve per-register drafts and configured routing preferences', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page);
	await page.goto('/staff/correspondence/settings');
	await page.getByRole('button', { name: 'รูปแบบที่ใช้', exact: true }).click();
	await page.getByRole('option', { name: 'รับ เลขลำดับ / พ.ศ.', exact: true }).click();
	await page.getByRole('button', { name: 'จำนวนหลักเลขลำดับ', exact: true }).click();
	await page.getByRole('option', { name: '3 หลัก', exact: true }).click();
	await expect(page.getByLabel('ตัวอย่างเลขทะเบียน', { exact: true })).toHaveText('รับ 001/2569');
	await page.getByRole('radio', { name: 'หนังสือส่ง', exact: true }).click();
	await expect(page.getByLabel('ตัวอย่างเลขทะเบียน', { exact: true })).toHaveText('1/2569');
	await page.getByRole('radio', { name: 'หนังสือรับ', exact: true }).click();
	await expect(page.getByLabel('ตัวอย่างเลขทะเบียน', { exact: true })).toHaveText('รับ 001/2569');
	await page.getByRole('button', { name: 'เพิ่มแม่แบบข้อความ', exact: true }).click();
	const name = page.getByLabel('ชื่อแม่แบบข้อความ *', { exact: true }).last();
	await expect(name).toBeFocused();
	await name.fill('ตราฝ่ายงาน');
	await page.getByLabel('ข้อความแม่แบบ *', { exact: true }).last().fill('ลงทะเบียนโดย ');
	await page.getByRole('button', { name: 'ชื่อผู้ดำเนินการ', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	const saved = api.settingsWrites[0];
	expect(saved).toMatchObject({
		register: 'incoming',
		numbering: { format: 'รับ {number}/{year}', minimumDigits: 3 },
		defaultPurpose: 'execute',
		defaultAllowOnwardAssignment: false
	});
	expect(saved.textStampTemplates).toHaveLength(4);
	expect(saved.textStampTemplates![3]).toMatchObject({
		name: 'ตราฝ่ายงาน',
		body: 'ลงทะเบียนโดย {signerName}'
	});
	expect(saved.textStampTemplates![3].id).toMatch(/^[\da-f]{8}-[\da-f]{4}-7[\da-f]{3}-[89ab]/);
});

test.describe('touch text placement', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('a rotated receipt box moves and resizes by touch without confirming a workflow', async ({
		page
	}) => {
		const api = await mockCorrespondence(page, { rotation: 90 });
		await page.goto(detailPath);
		const dialog = await stampDialog(page);
		await dialog.getByRole('button', { name: 'ตรวจข้อความ', exact: true }).tap();
		await dialog.getByRole('button', { name: 'วางบนเอกสาร', exact: true }).tap();
		await page.getByRole('button', { name: 'หมุน 90°', exact: true }).tap();
		const frame = page.getByTestId('text-annotation-placement');
		const session = await page.context().newCDPSession(page);
		const before = (await frame.boundingBox())!;
		for (const name of [/^เลื่อนกรอบข้อความ/, /^ปรับขนาดกรอบข้อความ/]) {
			const target = frame.getByRole('button', { name });
			await target.scrollIntoViewIfNeeded();
			const box = (await target.boundingBox())!,
				x = box.x + box.width / 2,
				y = box.y + box.height / 2;
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchStart',
				touchPoints: [{ x, y }]
			});
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchMove',
				touchPoints: [{ x: x + 12, y: y - 10 }]
			});
			await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
		}
		const after = (await frame.boundingBox())!;
		expect(
			Math.abs(after.width - before.width) + Math.abs(after.height - before.height)
		).toBeGreaterThan(1);
		expect(api.writes).toHaveLength(0);
		await expect.poll(() => api.readRequests.length).toBe(1);
	});
});
