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
	const closing = page.close({ runBeforeUnload: true });
	const unload = await dialog;
	expect(unload.type()).toBe('beforeunload');
	await unload.dismiss();
	await closing;
	expect(page.isClosed()).toBe(false);
	await expect(page.getByRole('list', { name: 'ผู้รับที่เพิ่มแล้ว' })).toContainText(
		'ครูเจ้าของ ทดสอบ'
	);
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'ครูเจ้าของ ทดสอบ'
	);
	expect(api.writes).toHaveLength(1);
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
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'ครูผู้รับคนที่สอง'
	);
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'หน่วยงาน งานบริหารทั่วไป'
	);
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'งานบริหารทั่วไป · หัวหน้างาน'
	);
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
		await expect
			.poll(() =>
				frame
					.getByRole('img', { name: 'ตัวอย่างลายมือชื่อบนเอกสาร' })
					.evaluate((element) => (element instanceof HTMLImageElement ? element.naturalWidth : 0))
			)
			.toBeGreaterThan(1);
		await frame.scrollIntoViewIfNeeded();
		await page.screenshot({
			path: '/tmp/correspondence-ux-390-light-private-image.png',
			fullPage: true
		});
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

for (const width of [390, 1543])
	for (const theme of ['light', 'dark'] as const)
		test(`visual workflow ${width}px ${theme} preserves reachable actions and document previews`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: width === 390 ? 844 : 888 });
			await page.addInitScript((theme) => {
				localStorage.setItem('ui-preferences', JSON.stringify({ theme }));
				document.documentElement.classList.toggle('dark', theme === 'dark');
			}, theme);
			await mockCorrespondence(page);
			const screenshot = async (surface: string) => {
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				await page.screenshot({
					path: `/tmp/correspondence-ux-${width}-${theme}-${surface}.png`,
					fullPage: true
				});
			};
			await page.goto('/staff/correspondence');
			await expect(page.getByTestId('correspondence-register')).toContainText('18/2569');
			await screenshot('list');
			await page.goto(detailPath);
			await expect(page.getByTestId('correspondence-current-step')).toContainText(
				'ขั้นตอนปัจจุบัน'
			);
			await screenshot('detail');
			await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
			await draw(page);
			await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
			await expect(page.getByTestId('signature-placement').locator('path')).toHaveCount(1);
			await page.getByTestId('signature-placement').scrollIntoViewIfNeeded();
			await expect(page.getByTestId('signature-placement')).toBeVisible();
			await screenshot('signature');
			await page
				.getByTestId('correspondence-action')
				.getByRole('button', { name: 'ยกเลิก', exact: true })
				.click();
			await fillDocument(page);
			await chooseRoute(page);
			await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
			await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toBeVisible();
			await screenshot('review');
			await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
			await page.getByRole('button', { name: 'ออกโดยไม่บันทึก' }).click();
			await page.goto('/staff/correspondence/templates');
			await page.getByRole('button', { name: 'แจ้งเพื่อทราบ', exact: true }).click();
			await expect(page.getByLabel('ชื่อแม่แบบ *')).toHaveValue('แจ้งเพื่อทราบ');
			await screenshot('preset');
		});

test('stale wizard draft requires reading and acknowledging the latest revision while retaining inputs', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', failRevision: 409 });
	await fillDocument(page);
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	await page.getByLabel('เนื้อหา / ข้อความประกอบ').fill('ข้อมูลที่ฉันกำลังแก้ไข');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ตรวจสอบฉบับร่างล่าสุด' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ตรวจสอบฉบับร่างล่าสุด' })).toContainText(
		'เอกสารฉบับใหม่จากผู้ส่ง'
	);
	await expect(page.getByLabel('เนื้อหา / ข้อความประกอบ')).toHaveValue('ข้อมูลที่ฉันกำลังแก้ไข');
	await page.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	const revisions = api.writes
		.filter((write) => write.path.endsWith('/revisions'))
		.map((write) => write.payload as ReviseCorrespondenceRequest);
	expect(revisions).toHaveLength(2);
	expect(revisions[1].expectedRevision).toBeGreaterThan(revisions[0].expectedRevision);
	expect(revisions[1].requestId).not.toBe(revisions[0].requestId);
	expect(api.detail().document.content).toBe('ข้อมูลที่ฉันกำลังแก้ไข');
	expect(api.count(`/api/correspondence/${correspondenceId}`)).toBe(1);
});

test('remove and reselect the same attachment path before a failed upload and retry', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', failUploadAt: 1 });
	await fillDocument(page);
	const file = {
		name: 'same-path.txt',
		mimeType: 'text/plain',
		buffer: Buffer.from('same synthetic attachment')
	};
	await page.getByLabel('ไฟล์แนบ', { exact: true }).setInputFiles(file);
	await page.getByRole('button', { name: 'นำไฟล์ same-path.txt ออก', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ไฟล์ที่เลือก' })).toHaveCount(0);
	await page.getByLabel('ไฟล์แนบ', { exact: true }).setInputFiles(file);
	await expect(page.getByRole('region', { name: 'ไฟล์ที่เลือก' })).toContainText('same-path.txt');
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลองอัปโหลดอีกครั้ง' })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอัปโหลดอีกครั้ง' }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	expect(api.uploads).toHaveLength(2);
	expect(api.detail().document.attachmentFileIds).toHaveLength(1);
	expect(api.writes.filter((write) => write.path === '/api/correspondence')).toHaveLength(1);
});

test('a selected position can be added after search no longer displays that position', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', narrowSearch: true });
	const remoteUserId = '55000000-0000-4000-8000-000000000856';
	let optionReads = 0;
	page.on('request', (request) => {
		if (new URL(request.url()).pathname === '/api/correspondence/options') optionReads += 1;
	});
	await page.route(
		(url) =>
			url.pathname === '/api/correspondence/options' && url.searchParams.get('search') === 'ครู',
		(route) =>
			route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({
					success: true,
					data: {
						users: [{ id: remoteUserId, displayName: 'ครูค้นหาเพิ่มเติม' }],
						organizations: [],
						positions: [],
						templates: []
					}
				})
			})
	);
	await fillDocument(page);
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน' }).click();
	await page.getByRole('option', { name: 'เวียนหนังสือมาตรฐาน' }).click();
	await page.getByRole('button', { name: 'กลุ่มผู้รับ', exact: true }).click();
	await page.getByRole('option', { name: 'ตำแหน่งในหน่วยงาน', exact: true }).click();
	await page.getByRole('checkbox', { name: 'งานบริหารทั่วไป · หัวหน้างาน' }).check();
	await page.getByText('ค้นหาผู้รับเพิ่มเติม', { exact: true }).click();
	await page.getByLabel('ชื่อบุคลากร / หน่วยงาน').fill('ครู');
	await page.getByRole('button', { name: 'ค้นหาผู้รับ', exact: true }).click();
	await expect(page.getByText('ไม่พบผู้รับ ลองค้นหาเพิ่มเติม')).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
	await page.getByRole('button', { name: 'กลุ่มผู้รับ', exact: true }).click();
	await page.getByRole('option', { name: 'บุคลากร', exact: true }).click();
	await page.getByRole('checkbox', { name: 'ครูค้นหาเพิ่มเติม', exact: true }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
	await page.getByRole('button', { name: 'ย้อนกลับ', exact: true }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByRole('list', { name: 'ผู้รับที่เพิ่มแล้ว' })).toContainText(
		'ครูค้นหาเพิ่มเติม'
	);
	expect(optionReads).toBe(2);

	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'งานบริหารทั่วไป · หัวหน้างาน'
	);
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'ครูค้นหาเพิ่มเติม'
	);
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	expect(
		(api.writes.at(-1)!.payload as SendCorrespondenceRequest).stepRecipients![0].recipients[0]
	).toMatchObject({ kind: 'organization_position', positionCode: 'HEAD' });
});

async function chooseDetailRoute(page: Page) {
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'เลือกผู้รับและส่งหนังสือ' }).click();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน' }).click();
	await page.getByRole('option', { name: 'เวียนหนังสือมาตรฐาน' }).click();
	await page.getByRole('checkbox', { name: 'ครูเจ้าของ ทดสอบ', exact: true }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
}

test('detail send conflict explicitly reads and rebases the current draft while retaining chosen recipients', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', failSend: 409 });
	await chooseDetailRoute(page);
	await page.getByRole('button', { name: 'ส่งหนังสือ', exact: true }).click();
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด', exact: true }).click();
	await expect(page.getByText('เอกสารเปลี่ยนแปลงแล้ว กรุณาตรวจสอบฉบับปัจจุบัน')).toBeVisible();
	await expect(page.getByRole('list', { name: 'ผู้รับที่เพิ่มแล้ว' })).toContainText(
		'ครูเจ้าของ ทดสอบ'
	);
	await page
		.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บผู้รับที่เลือก', exact: true })
		.click();
	await page.getByRole('button', { name: 'ส่งหนังสือ', exact: true }).click();
	await expect(page.getByTestId('correspondence-send')).toHaveCount(0);
	const sends = api.writes
		.filter((write) => write.path.endsWith('/send'))
		.map((write) => write.payload as SendCorrespondenceRequest);
	expect(sends).toHaveLength(2);
	expect(sends[1].stepRecipients).toEqual(sends[0].stepRecipients);
	expect(sends[1].expectedRevision).toBeGreaterThan(sends[0].expectedRevision);
	expect(api.count(`/api/correspondence/${correspondenceId}`)).toBe(2);
});

test('in-flight detail send prevents leaving until the server responds', async ({ page }) => {
	const api = await mockCorrespondence(page, { mode: 'draft', holdSend: true });
	await chooseDetailRoute(page);
	await page.getByRole('button', { name: 'ส่งหนังสือ', exact: true }).click();
	await expect
		.poll(() => api.writes.filter((write) => write.path.endsWith('/send')).length)
		.toBe(1);
	await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toContainText('กำลังบันทึกข้อมูล');
	await expect(page.getByRole('button', { name: 'ออกโดยไม่บันทึก' })).toHaveCount(0);
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	api.release();
	await expect(page.getByTestId('correspondence-send')).toHaveCount(0);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

for (const newer of [false, true])
	test(`accepted detail revision replay ${newer ? 'retains newer annotations' : 'skips an unchanged revision'}`, async ({
		page
	}) => {
		const api = await mockCorrespondence(page, { mode: 'draft', abortAcceptedRevision: true });
		await page.goto(detailPath);
		await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
		await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
		await page
			.getByLabel('ข้อความกำกับ', { exact: true })
			.fill('หมายเหตุฉบับที่เซิร์ฟเวอร์รับแล้ว');
		await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
		await page.getByRole('button', { name: /^บันทึกฉบับแก้ไข/ }).click();
		await expect(page.getByText('ข้อมูลฉบับร่างยังอยู่', { exact: true })).toBeVisible();
		if (newer) {
			await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('หมายเหตุใหม่ที่ยังไม่ยืนยัน');
			await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
		}
		await page.getByRole('button', { name: /^บันทึกฉบับแก้ไข/ }).click();
		await expect
			.poll(() => api.writes.filter((write) => write.path.endsWith('/revisions')).length)
			.toBe(newer ? 3 : 2);
		const revisions = api.writes.filter((write) => write.path.endsWith('/revisions'));
		expect(revisions[1].payload).toEqual(revisions[0].payload);
		await expect(page.getByRole('button', { name: /^บันทึกฉบับแก้ไข/ })).toBeEnabled();
		expect(api.detail().document.annotations.map((item) => item.text)).toEqual(
			newer
				? ['หมายเหตุฉบับที่เซิร์ฟเวอร์รับแล้ว', 'หมายเหตุใหม่ที่ยังไม่ยืนยัน']
				: ['หมายเหตุฉบับที่เซิร์ฟเวอร์รับแล้ว']
		);
		expect(new Set(api.detail().document.annotations.map((item) => item.id)).size).toBe(
			newer ? 2 : 1
		);
	});

test('metadata counts Unicode scalars in the browser and preserves over-limit drafts for correction', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await fillDocument(page);
	await page.getByLabel('เรื่อง *', { exact: true }).fill('😀'.repeat(301));
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText('เรื่องต้องไม่เกิน 300 ตัวอักษร', { exact: true })).toBeVisible();
	await expect(page.getByLabel('เรื่อง *', { exact: true })).toHaveValue('😀'.repeat(301));
	expect(api.writes).toHaveLength(0);
	await page.getByLabel('เรื่อง *', { exact: true }).fill('😀'.repeat(300));
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('😀'.repeat(301));
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText('ผู้ส่งต้องไม่เกิน 300 ตัวอักษร', { exact: true })).toBeVisible();
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('😀'.repeat(300));
	await page.getByLabel('เลขที่หนังสือต้นทาง', { exact: true }).fill('😀'.repeat(151));
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(
		page.getByText('เลขที่หนังสือต้นทางต้องไม่เกิน 150 ตัวอักษร', { exact: true })
	).toBeVisible();
	expect(api.writes).toHaveLength(0);
	await page.getByLabel('เลขที่หนังสือต้นทาง', { exact: true }).fill('😀'.repeat(150));
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	const payload = api.writes[0].payload as CreateCorrespondenceRequest;
	expect([...payload.title]).toHaveLength(300);
	expect([...payload.sender]).toHaveLength(300);
	expect([...payload.documentNumber!]).toHaveLength(150);
	expect(api.writes).toHaveLength(1);
});

test('step deadline survives wizard backtracking, clears with its control and resets with a different template', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', secondTemplate: true });
	await fillDocument(page);
	await chooseRoute(page);
	const chooseDate = async () => {
		await page.locator('#step-due-0').click();
		const day = page
			.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
			.filter({ hasText: /^15$/ });
		const value = await day.getAttribute('data-value');
		if (!value) throw new Error('ปฏิทินไม่มีวันที่ที่เลือก');
		await day.click();
		return value;
	};
	await chooseDate();
	const caption = await page.locator('#step-due-0').innerText();
	await page.getByRole('button', { name: 'ย้อนกลับ', exact: true }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.locator('#step-due-0')).toHaveText(caption);
	await page
		.locator('#step-due-0')
		.locator('..')
		.getByRole('button', { name: 'ล้างวันที่', exact: true })
		.click();
	await expect(page.locator('#step-due-0')).toContainText('เลือกวันที่');
	await chooseDate();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน' }).click();
	await page.getByRole('option', { name: 'แม่แบบทางเลือก', exact: true }).click();
	await expect(page.locator('#step-due-0')).toContainText('เลือกวันที่');
	await page.getByRole('checkbox', { name: 'ครูเจ้าของ ทดสอบ', exact: true }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
	const selectedDate = await chooseDate();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	expect((api.writes.at(-1)!.payload as SendCorrespondenceRequest).stepRecipients![0].dueAt).toBe(
		`${selectedDate}T23:59:59+07:00`
	);
});

test('optional-only edit prompts on leave, dismiss retains it and reverting to initial clears the guard', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await page.goto('/staff/correspondence/new');
	await page.getByRole('button', { name: 'ความเร่งด่วน', exact: true }).click();
	await page.getByRole('option', { name: 'ด่วน', exact: true }).click();
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toBeVisible();
	await page.getByRole('button', { name: 'อยู่หน้านี้ต่อ' }).click();
	await expect(page.getByRole('button', { name: 'ความเร่งด่วน', exact: true })).toContainText(
		'ด่วน'
	);
	await page.getByRole('button', { name: 'ความเร่งด่วน', exact: true }).click();
	await page.getByRole('option', { name: 'ปกติ', exact: true }).click();
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await expect(page).toHaveURL(/\/staff\/correspondence$/);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	expect(api.writes).toHaveLength(0);
	await fillDocument(page);
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	await page.getByLabel('เรื่อง *', { exact: true }).fill('');
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('');
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toBeVisible();
	await page.getByRole('button', { name: 'อยู่หน้านี้ต่อ' }).click();
	await expect(page.getByLabel('เรื่อง *', { exact: true })).toHaveValue('');
	await expect(page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *')).toHaveValue('');
	await page.getByLabel('เรื่อง *', { exact: true }).fill('หนังสือทดสอบประสบการณ์');
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('โรงเรียนตัวอย่าง');
	await page.getByRole('link', { name: 'ยกเลิก', exact: true }).click();
	await expect(page).toHaveURL(/\/staff\/correspondence$/);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	expect(api.writes).toHaveLength(1);
	expect(api.writes[0].path).toBe('/api/correspondence');
});

test('folded malformed source URL unfolds and focuses validation without losing its draft', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft' });
	await fillDocument(page);
	await page.getByText('แหล่งที่มาเพิ่มเติม', { exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มแหล่งที่มา', exact: true }).click();
	await page.getByLabel('ชื่อแหล่งที่มา 1', { exact: true }).fill('ระบบต้นทาง');
	const input = page.getByLabel('ลิงก์ 1', { exact: true });
	await input.fill('not-a-url');
	for (let attempt = 0; attempt < 2; attempt += 1) {
		await page.getByText('แหล่งที่มาเพิ่มเติม', { exact: true }).click();
		await expect(input).not.toBeVisible();
		await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
		await expect(input).toBeVisible();
		await expect(input).toBeFocused();
		await expect(input).toHaveValue('not-a-url');
		await expect(
			page.getByText('ระบุชื่อและลิงก์แหล่งที่มาแบบ https:// หรือ http://', { exact: true })
		).toBeVisible();
		expect(api.writes).toHaveLength(0);
	}
	await input.fill('ftp://school.example/source');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(
		page.getByText('ระบุชื่อและลิงก์แหล่งที่มาแบบ https:// หรือ http://', { exact: true })
	).toBeVisible();
	expect(api.writes).toHaveLength(0);
	await input.fill('https://school.example/source');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByRole('region', { name: 'เส้นทางผู้รับ' })).toBeVisible();
	expect(api.writes).toHaveLength(0);
});
