import { test, expect, type Page } from '@playwright/test';
import { mockDirectedCorrespondence, assignmentId } from './fixtures/correspondence-directed';
import { correspondenceId, detailPath } from './fixtures/correspondence';
import { actor, firstStaff } from './fixtures/staff-home-route-data';
test.use({ serviceWorkers: 'block' });
async function fillDocument(page: Page) {
	await page.goto('/staff/correspondence/new');
	await page.getByLabel('เรื่อง *', { exact: true }).fill('หนังสือสั่งการตัวอย่าง');
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('โรงเรียนตัวอย่าง');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
}
async function choosePerson(page: Page, label: string, name: string) {
	await page.getByRole('button', { name: label, exact: true }).click();
	await page.getByRole('option', { name, exact: true }).click();
}
async function addRequiredRecipient(page: Page) {
	await page.getByRole('checkbox', { name: 'ครูผู้รับคนที่สอง', exact: true }).check();
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
	for (const dark of [false, true])
		test(`directed wizard ${width}px ${dark ? 'dark' : 'light'} defaults to one decision maker and retains backtracking`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 960 });
			const api = await mockDirectedCorrespondence(page, { scenario: 'draft' });
			await fillDocument(page);
			if (dark) await page.locator('html').evaluate((element) => element.classList.add('dark'));
			await expect(
				page.getByRole('radio', { name: 'เลือกผู้พิจารณาสั่งการ', exact: true })
			).toHaveAttribute('data-state', 'on');
			await choosePerson(page, 'ผู้พิจารณาสั่งการคนแรก *', 'ครูผู้รับคนที่สอง');
			await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
			await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
				'ครูผู้รับคนที่สอง'
			);
			await page
				.getByRole('button', { name: 'ย้อนกลับ', exact: true })
				.filter({ visible: true })
				.last()
				.click();
			await expect(
				page.getByRole('button', { name: 'ผู้พิจารณาสั่งการคนแรก *', exact: true })
			).toContainText('ครูผู้รับคนที่สอง');
			await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
			await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
			await expect(page).toHaveURL(new RegExp(correspondenceId));
			expect(api.sends).toHaveLength(1);
			expect(api.sends[0].firstRecipient).toEqual({ kind: 'user', userId: firstStaff });
			expect(api.sends[0]).not.toHaveProperty('templateId');
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
		});
test('directed send replays the frozen first assignee after an accepted response is lost', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page, {
		scenario: 'draft',
		abortSend: true,
		recommended: true
	});
	await fillDocument(page);
	await expect(
		page.getByRole('button', { name: 'ผู้พิจารณาสั่งการคนแรก *', exact: true })
	).toContainText('ครูผู้รับคนที่สอง');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page.getByText('ยังยืนยันผลการส่งครั้งก่อนไม่ได้')).toBeVisible();
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	expect(api.sends).toHaveLength(2);
	expect(api.sends[1]).toEqual(api.sends[0]);
});
test('instructions, drawing and signature submit once with branch recipients and retain required inherited review', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page, {
		scenario: 'decide',
		inheritedReview: true,
		failAction: true
	});
	await page.goto(detailPath);
	await expect(page.getByRole('list', { name: 'สาขาการดำเนินการ' })).toContainText(
		'รอพิจารณาสั่งการ'
	);
	await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
	await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
	await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('โปรดดำเนินการภายในวันศุกร์');
	await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
	await page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true }).click();
	await expect(page.getByText(/เงื่อนไขตรวจทานที่ได้รับมอบหมายยังคงใช้/)).toContainText(
		'ครูผู้รับคนที่สอง'
	);
	await expect(
		page.getByRole('checkbox', { name: 'ให้ผู้ตรวจทานตรวจผลหลังผู้รับที่จำเป็นดำเนินการครบ' })
	).not.toBeChecked();
	await addRequiredRecipient(page);
	await page
		.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })
		.fill('มอบหมายให้ดำเนินการตามหนังสือ');
	await page.getByRole('checkbox', { name: 'ลงนามพร้อมคำสั่ง', exact: true }).check();
	await draw(page);
	await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
	await page.getByRole('button', { name: 'ยืนยันพิจารณาสั่งการ', exact: true }).click();
	await expect(page.getByTestId('correspondence-action')).toContainText('บริการไม่พร้อม');
	await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveValue(
		'มอบหมายให้ดำเนินการตามหนังสือ'
	);
	await page.getByRole('button', { name: 'ยืนยันพิจารณาสั่งการ', exact: true }).click();
	await expect(page.getByRole('list', { name: 'สาขาการดำเนินการ' })).toContainText(
		'รอผู้รับที่จำเป็นดำเนินการ'
	);
	expect(api.actions).toHaveLength(2);
	expect(api.actions[1]).toEqual(api.actions[0]);
	expect(api.actions[0]).toMatchObject({
		assignmentId,
		action: 'route',
		routing: {
			requireReview: false,
			recipients: [
				{
					recipient: { kind: 'user', userId: firstStaff },
					purpose: 'execute',
					required: true,
					allowOnwardAssignment: false
				}
			]
		},
		signature: { method: 'drawn', documentRevision: 1 },
		annotations: [{ text: 'โปรดดำเนินการภายในวันศุกร์' }]
	});
});
test('the assigning actor can explicitly review others without forcing review by default', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page, { scenario: 'decide' });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true }).click();
	await addRequiredRecipient(page);
	await page
		.getByRole('checkbox', { name: 'ให้ผู้ตรวจทานตรวจผลหลังผู้รับที่จำเป็นดำเนินการครบ' })
		.check();
	await choosePerson(page, 'ผู้ตรวจทาน *', 'ครูเจ้าของ ทดสอบ');
	await page.getByRole('button', { name: 'ยืนยันพิจารณาสั่งการ', exact: true }).click();
	await expect.poll(() => api.actions.length).toBe(1);
	expect(api.actions[0].routing).toMatchObject({
		requireReview: true,
		reviewer: { kind: 'user', userId: actor }
	});
});
test('a copied recipient can acknowledge but cannot issue orders', async ({ page }) => {
	const api = await mockDirectedCorrespondence(page, { scenario: 'copy' });
	await page.goto(detailPath);
	await expect(page.getByRole('list', { name: 'สาขาการดำเนินการ' })).toContainText(
		'สำเนาเพื่อทราบ'
	);
	await expect(page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'รับทราบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันรับทราบ', exact: true }).click();
	await expect.poll(() => api.actions.length).toBe(1);
	expect(api.actions[0]).toMatchObject({ assignmentId, action: 'acknowledge' });
	expect(api.actions[0].routing).toBeUndefined();
});
test('the first decision maker can finish their consideration with a required comment', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page, { scenario: 'decide' });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'พิจารณาเสร็จและจบเรื่อง', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันพิจารณาเสร็จและจบเรื่อง', exact: true }).click();
	await expect(page.getByTestId('correspondence-action')).toContainText('กรอกความคิดเห็น / เหตุผล');
	expect(api.actions).toHaveLength(0);
	await page
		.getByLabel('ความคิดเห็น / เหตุผล *', { exact: true })
		.fill('ตรวจสอบแล้ว ดำเนินการได้ครบในหน้าที่');
	await page.getByRole('button', { name: 'ยืนยันพิจารณาเสร็จและจบเรื่อง', exact: true }).click();
	await expect(
		page.getByText('ผู้รับที่จำเป็นและการตรวจทานครบแล้ว เรื่องดำเนินการเสร็จสิ้น')
	).toBeVisible();
	expect(api.actions[0]).toMatchObject({ assignmentId, action: 'review' });
});
test('settings preserve separate register drafts and save exact configured defaults', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page);
	await page.goto('/staff/correspondence/settings');
	await expect(page.getByTestId('correspondence-routing-settings')).toContainText(
		'หากไม่จำกัดรายชื่อ'
	);
	await choosePerson(page, 'ผู้พิจารณาสั่งการคนแรกที่แนะนำ', 'ครูผู้รับคนที่สอง');
	await page.getByRole('radio', { name: 'หนังสือส่ง', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ผู้พิจารณาสั่งการคนแรกที่แนะนำ', exact: true })
	).toContainText('ให้ผู้ลงทะเบียนเลือกทุกครั้ง');
	await page.getByRole('radio', { name: 'หนังสือรับ', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ผู้พิจารณาสั่งการคนแรกที่แนะนำ', exact: true })
	).toContainText('ครูผู้รับคนที่สอง');
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.settingsWrites[0]).toMatchObject({
		register: 'incoming',
		defaultFirstRecipient: { kind: 'user', userId: firstStaff },
		defaultRequireReview: false,
		defaultReviewer: null
	});
});
