import { test, expect, type Page } from '@playwright/test';
import { mockCorrespondence, correspondenceId, detailPath } from './fixtures/correspondence';
import type {
	CreateCorrespondenceRequest,
	ReviseCorrespondenceRequest,
	SendCorrespondenceRequest
} from '../../src/lib/api/correspondence';

test.use({ serviceWorkers: 'block' });
async function fillDocument(page: Page) {
	await page.goto('/staff/correspondence/new');
	await page.getByLabel('เรื่อง *', { exact: true }).fill('หนังสือทดสอบประสบการณ์');
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('โรงเรียนตัวอย่าง');
}
async function chooseRoute(page: Page) {
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน' }).click();
	await page.getByRole('option', { name: 'เวียนหนังสือมาตรฐาน' }).click();
	await page.getByRole('checkbox', { name: 'ครูเจ้าของ ทดสอบ', exact: true }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
}
async function draw(page: Page) {
	const surface = page.getByRole('img', { name: 'พื้นที่วาดลายมือชื่อ' });
	await surface.scrollIntoViewIfNeeded();
	const box = (await surface.boundingBox())!;
	await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.3);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.7, { steps: 8 });
	await page.mouse.up();
}

for (const width of [390, 1440])
	test(`register ${width}px shows server totals and snapshot progress with URL history`, async ({
		page
	}) => {
		await page.setViewportSize({ width, height: 960 });
		const api = await mockCorrespondence(page, {
			listCounts: { actionRequired: 38, sent: 71, drafts: 4, all: 113 }
		});
		await page.goto('/staff/correspondence');
		const views = page.getByRole('navigation', { name: 'มุมมองทะเบียนหนังสือ' });
		await expect(views.getByRole('link', { name: /รอฉันดำเนินการ 38/ })).toHaveAttribute(
			'aria-current',
			'page'
		);
		await expect(views).toContainText('113');
		const register = page.getByTestId('correspondence-register');
		await expect(register).toContainText('18/2569');
		await expect(register).toContainText('ศธ 001/2569');
		await expect(register).toContainText('ครูผู้รับคนที่สอง');
		expect(api.count(`/api/correspondence/${correspondenceId}`)).toBe(0);
		await views.getByRole('link', { name: /ฉันส่งติดตาม/ }).click();
		await expect(page).toHaveURL(/view=sent/);
		await page.getByRole('link', { name: 'ลงนาม', exact: true }).filter({ visible: true }).click();
		await expect(page.getByTestId('correspondence-detail')).toBeVisible();
		await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
		await expect(page).toHaveURL(/view=sent/);
		await views.getByRole('link', { name: /ฉบับร่าง/ }).click();
		await expect(register).toContainText('ยังไม่มีหนังสือในมุมมองนี้');
		await page.getByRole('button', { name: 'ดูหนังสือทั้งหมด' }).click();
		await expect(page).toHaveURL(/view=all/);
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
	});

test('save draft keeps the wizard and warns that unsent routing would be lost', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await fillDocument(page);
	await chooseRoute(page);
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	await expect(page).toHaveURL(/correspondence\/new$/);
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toContainText('เส้นทางผู้รับ');
	await page.getByRole('button', { name: 'อยู่หน้านี้ต่อ' }).click();
	await expect(page.getByRole('list', { name: 'ผู้รับที่เพิ่มแล้ว' })).toContainText(
		'ครูเจ้าของ ทดสอบ'
	);
	expect(api.writes).toHaveLength(1);
	expect(
		await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))
	).not.toContain('หนังสือทดสอบประสบการณ์');
	const dialog = page.waitForEvent('dialog');
	const reload = page.reload({ waitUntil: 'commit' }).catch(() => {});
	const unload = await dialog;
	expect(unload.type()).toBe('beforeunload');
	await unload.dismiss();
	await reload;
});

test('individual upload failure retries only the failed file and never recreates the draft', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', failUploadAt: 2 });
	await fillDocument(page);
	await page.getByLabel('เอกสารต้นฉบับ PDF หรือภาพ').setInputFiles({
		name: 'original.pdf',
		mimeType: 'application/pdf',
		buffer: Buffer.from('%PDF original')
	});
	await page.getByLabel('ไฟล์แนบ', { exact: true }).setInputFiles({
		name: 'attachment.txt',
		mimeType: 'text/plain',
		buffer: Buffer.from('synthetic evidence')
	});
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ไฟล์ที่เลือก' })).toContainText(
		'อัปโหลดไม่สำเร็จ'
	);
	await page.getByRole('button', { name: 'ลองอัปโหลดอีกครั้ง' }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	expect(api.writes.filter((write) => write.path === '/api/correspondence')).toHaveLength(1);
	expect(api.uploads).toHaveLength(3);
	expect(api.uploads.filter((body) => body.includes('original.pdf'))).toHaveLength(1);
	expect(api.detail().document.attachmentFileIds).toHaveLength(1);
});

test('lost create response replays the frozen command then saves later metadata edits', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', abortAcceptedCreate: true });
	await fillDocument(page);
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText('ข้อมูลที่กรอกยังอยู่ กรุณาตรวจสอบแล้วลองอีกครั้ง')).toBeVisible();
	await page.getByLabel('เรื่อง *', { exact: true }).fill('เปลี่ยนเรื่องหลังเครือข่ายขาด');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	const creates = api.writes
		.filter((write) => write.path === '/api/correspondence')
		.map((write) => write.payload as CreateCorrespondenceRequest);
	expect(creates).toHaveLength(2);
	expect(creates[1]).toEqual(creates[0]);
	expect(api.detail().record.title).toBe('เปลี่ยนเรื่องหลังเครือข่ายขาด');
	expect((api.writes.at(-1)!.payload as ReviseCorrespondenceRequest).metadata?.title).toBe(
		'เปลี่ยนเรื่องหลังเครือข่ายขาด'
	);
});

test('lost revision response keeps upload IDs and applies later edits after exact replay', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', abortAcceptedRevision: true });
	await fillDocument(page);
	await page.getByLabel('เอกสารต้นฉบับ PDF หรือภาพ').setInputFiles({
		name: 'original.pdf',
		mimeType: 'application/pdf',
		buffer: Buffer.from('%PDF original')
	});
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText('ข้อมูลที่กรอกยังอยู่ กรุณาตรวจสอบแล้วลองอีกครั้ง')).toBeVisible();
	await page.getByLabel('เนื้อหา / ข้อความประกอบ').fill('เนื้อหาที่แก้ไขหลังเครือข่ายขาด');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	const revisions = api.writes.filter((write) => write.path.endsWith('/revisions'));
	expect(revisions).toHaveLength(3);
	expect(revisions[1].payload).toEqual(revisions[0].payload);
	expect(revisions[2].payload.requestId).not.toBe(revisions[0].payload.requestId);
	expect(api.detail().document.content).toBe('เนื้อหาที่แก้ไขหลังเครือข่ายขาด');
	expect(api.uploads).toHaveLength(1);
});

test('lost send response keeps the exact recipients and request without another create or upload', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', abortAcceptedSend: true });
	await fillDocument(page);
	await chooseRoute(page);
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page.getByText('ยังยืนยันผลการส่งครั้งก่อนไม่ได้')).toBeVisible();
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	const sends = api.writes
		.filter((write) => write.path.endsWith('/send'))
		.map((write) => write.payload as SendCorrespondenceRequest);
	expect(sends).toHaveLength(2);
	expect(sends[1]).toEqual(sends[0]);
	expect(api.writes.filter((write) => write.path === '/api/correspondence')).toHaveLength(1);
});

test('batch recipients preserve user, organization and position semantics and deduplicate exact choices', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await fillDocument(page);
	await chooseRoute(page);
	await page.getByRole('checkbox', { name: 'ครูเจ้าของ ทดสอบ', exact: true }).check();
	await page.getByRole('checkbox', { name: 'ครูผู้รับคนที่สอง', exact: true }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (2)', exact: true }).click();
	for (const [kind, label] of [
		['หน่วยงาน', 'งานบริหารทั่วไป'],
		['ตำแหน่งในหน่วยงาน', 'งานบริหารทั่วไป · หัวหน้างาน']
	]) {
		await page.getByRole('button', { name: 'กลุ่มผู้รับ', exact: true }).click();
		await page.getByRole('option', { name: kind, exact: true }).click();
		await page.getByRole('checkbox', { name: label, exact: true }).check();
		await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
	}
	await expect(
		page.getByRole('list', { name: 'ผู้รับที่เพิ่มแล้ว' }).getByRole('listitem')
	).toHaveCount(4);
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	const recipients = (api.writes.at(-1)!.payload as SendCorrespondenceRequest).stepRecipients![0]
		.recipients;
	expect(recipients.map((item) => item.kind)).toEqual([
		'user',
		'user',
		'organization_unit',
		'organization_position'
	]);
	expect(recipients[3].positionCode).toBe('HEAD');
});

test('presets and copy remain local until an explicit versioned save', async ({ page }) => {
	const api = await mockCorrespondence(page, { permissions: ['process.manage.school'] });
	await page.goto('/staff/correspondence/templates');
	await expect(page.getByRole('region', { name: 'แม่แบบเริ่มต้น' })).toBeVisible();
	expect(api.templateWrites).toHaveLength(0);
	await page.getByRole('button', { name: 'เสนออนุมัติ', exact: true }).click();
	await expect(page.getByLabel('ชื่อแม่แบบ *', { exact: true })).toHaveValue('เสนออนุมัติ');
	await expect(page.getByTestId('process-template-editor')).toContainText('ขั้นตอน 2');
	expect(api.templateWrites).toHaveLength(0);
	await page.getByRole('button', { name: 'บันทึกแม่แบบรุ่นใหม่' }).click();
	await expect(page.getByTestId('process-templates')).toContainText('เสนออนุมัติ');
	await page.getByRole('button', { name: 'ทำสำเนาแม่แบบ' }).click();
	await expect(page.getByLabel('ชื่อแม่แบบ *', { exact: true })).toHaveValue('เสนออนุมัติ (สำเนา)');
	expect(api.templateWrites).toHaveLength(1);
	await page.getByRole('button', { name: 'บันทึกแม่แบบรุ่นใหม่' }).click();
	await expect.poll(() => api.templateWrites.length).toBe(2);
	expect(api.templateWrites[1].expectedRevision).toBeUndefined();
	expect(api.templateWrites[1].code).not.toBe(api.templateWrites[0].code);
});

for (const rotation of [0, 90])
	test(`drawn preview ${rotation} degrees supports drag resize and keyboard with explicit reconfirmation`, async ({
		page
	}) => {
		const api = await mockCorrespondence(page, { rotation });
		await page.emulateMedia({ colorScheme: 'dark' });
		await page.addInitScript(() => {
			document.documentElement.classList.add('dark');
			localStorage.setItem('ui-preferences', JSON.stringify({ theme: 'dark' }));
		});
		await page.goto(detailPath);
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		await draw(page);
		await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
		if (rotation === 90) await page.getByRole('button', { name: 'หมุน 90°' }).click();
		const frame = page.getByTestId('signature-placement');
		await expect(frame.locator('svg path')).toHaveCount(1);
		await expect(frame.locator('svg path')).toHaveAttribute('stroke', '#111827');
		const confirmation = page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ });
		await confirmation.check();
		const move = frame.getByRole('button', { name: /^เลื่อนลายมือชื่อ/ });
		await move.focus();
		await page.keyboard.press('ArrowLeft');
		await expect(confirmation).not.toBeChecked();
		const size = frame.getByRole('button', { name: /^ปรับขนาดลายมือชื่อ/ });
		const before = (await frame.boundingBox())!;
		await size.focus();
		await page.keyboard.press('ArrowRight');
		const after = (await frame.boundingBox())!;
		expect(
			Math.abs(after.width - before.width) + Math.abs(after.height - before.height)
		).toBeGreaterThan(1);
		const box = (await move.boundingBox())!;
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + box.width / 2 - 15, box.y + box.height / 2 - 20, { steps: 5 });
		await page.mouse.up();
		await confirmation.check();
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
		const signature = api.detail().signatures.at(-1)!.signature;
		expect(signature.x + signature.width).toBeLessThanOrEqual(1);
		expect(signature.y + signature.height).toBeLessThanOrEqual(1);
		expect(signature.documentRevision).toBe(1);
		expect(signature.contentHash).toBe('a'.repeat(64));
	});

test.describe('touch signature placement', () => {
	test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
	test('saved image preview moves and resizes with touch while retaining private ownership', async ({
		page
	}) => {
		const api = await mockCorrespondence(page, { rotation: 90 });
		await page.goto(detailPath);
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		await page.getByRole('button', { name: 'เลือกลายมือชื่อที่บันทึก' }).click();
		await page.getByRole('button', { name: 'ลายมือชื่อของฉัน', exact: true }).click();
		await page.getByRole('option', { name: 'ลายมือชื่อส่วนตัว' }).click();
		await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
		await page.getByRole('button', { name: 'หมุน 90°' }).click();
		const frame = page.getByTestId('signature-placement');
		await expect(frame.getByRole('img', { name: 'ตัวอย่างลายมือชื่อบนเอกสาร' })).toBeVisible();
		const session = await page.context().newCDPSession(page);
		for (const name of [/^เลื่อนลายมือชื่อ/, /^ปรับขนาดลายมือชื่อ/]) {
			const target = frame.getByRole('button', { name });
			await target.scrollIntoViewIfNeeded();
			const box = (await target.boundingBox())!;
			const x = box.x + box.width / 2,
				y = box.y + box.height / 2;
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchStart',
				touchPoints: [{ x, y }]
			});
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchMove',
				touchPoints: [{ x: x + 10, y: y - 10 }]
			});
			await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
		}
		await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
		expect(api.detail().signatures.at(-1)?.signature.method).toBe('image');
		expect(api.count('/api/correspondence/signatures')).toBe(1);
		await session.detach();
	});
});
