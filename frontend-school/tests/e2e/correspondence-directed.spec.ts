import { test, expect, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import {
	mockDirectedCorrespondence,
	assignmentId,
	parentAssignmentId,
	childAssignmentId
} from './fixtures/correspondence-directed';
import { correspondenceId, detailPath } from './fixtures/correspondence';
import { actor, firstStaff } from './fixtures/staff-home-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ baseURL }) => {
	expect(baseURL).toBe(process.env.E2E_BASE_URL);
	expect(new URL(baseURL ?? '').hostname).toBe('127.0.0.1');
});
test.afterEach(async ({ page, baseURL }) => {
	if (!page.isClosed()) expect(new URL(page.url()).origin).toBe(new URL(baseURL ?? '').origin);
});
async function capture(page: Page, filename: string) {
	const directory = process.env.CORRESPONDENCE_VISUAL_DIR;
	if (!directory) return;
	await mkdir(directory, { recursive: true });
	await page
		.locator('main')
		.evaluate((element) => element.scrollTo({ top: 0, behavior: 'instant' }));
	await page.screenshot({ path: join(directory, filename), fullPage: true });
}

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
			if (width === 1440 && !dark) await capture(page, '01-first-decision-maker-desktop.png');
			if (width === 390 && dark) await capture(page, '02-first-decision-maker-mobile-dark.png');
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
	await page.setViewportSize({ width: 1440, height: 1800 });
	const api = await mockDirectedCorrespondence(page, {
		scenario: 'decide',
		inheritedReview: true,
		failAction: true
	});
	await page.goto(detailPath);
	await expect(page.getByRole('list', { name: 'สาขาการดำเนินการ' })).toContainText('รอดำเนินการ');
	await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
	await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
	await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('โปรดดำเนินการภายในวันศุกร์');
	await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
	await page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true }).click();
	await expect(page.getByText(/เงื่อนไขตรวจทานที่ได้รับมอบหมายยังคงใช้/)).toContainText(
		'ครูสำรอง ทดสอบ'
	);
	await expect(
		page.getByRole('checkbox', { name: 'ให้ผู้ตรวจทานตรวจผลหลังผู้รับที่จำเป็นดำเนินการครบ' })
	).not.toBeChecked();
	await addRequiredRecipient(page);
	await page
		.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })
		.fill('มอบหมายให้ดำเนินการตามหนังสือ');
	await page.getByLabel('หลักฐานประกอบ', { exact: true }).setInputFiles({
		name: 'proof.txt',
		mimeType: 'text/plain',
		buffer: Buffer.from('synthetic proof')
	});
	await capture(page, '03-instructions-signature-and-inherited-review.png');
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
	expect(api.detail().instance?.directed?.assignments[0]).toMatchObject({
		id: parentAssignmentId,
		status: 'waiting_children',
		required: true,
		reviewer: { userId: '55000000-0000-4000-8000-000000000908' }
	});
	expect(api.actions[0].routing?.recipients[0].recipient.userId).not.toBe(
		api.detail().instance?.directed?.assignments[0].reviewer?.userId
	);
	expect(api.uploads).toHaveLength(1);
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
	await page.setViewportSize({ width: 1440, height: 1600 });
	const api = await mockDirectedCorrespondence(page, { scenario: 'decide' });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true }).click();
	await addRequiredRecipient(page);
	await page
		.getByRole('checkbox', { name: 'ให้ผู้ตรวจทานตรวจผลหลังผู้รับที่จำเป็นดำเนินการครบ' })
		.check();
	await choosePerson(page, 'ผู้ตรวจทาน *', 'ครูเจ้าของ ทดสอบ');
	await capture(page, '04-explicit-review.png');
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
		page.getByText(
			'ผู้รับที่จำเป็นและการตรวจทานครบแล้ว เรื่องดำเนินการเสร็จสิ้น ไม่ต้องส่งกลับตรวจรับ'
		)
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
	await capture(page, '06-register-routing-settings.png');
	await page.getByRole('button', { name: 'บันทึกการตั้งค่า', exact: true }).click();
	await expect(page.getByText('บันทึกการตั้งค่าแล้ว', { exact: true })).toBeVisible();
	expect(api.settingsWrites[0]).toMatchObject({
		register: 'incoming',
		defaultFirstRecipient: { kind: 'user', userId: firstStaff },
		defaultRequireReview: false,
		defaultReviewer: null
	});
});

test('finishing one leaf keeps other required branches and the whole book pending', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 960 });
	const api = await mockDirectedCorrespondence(page, { scenario: 'branches' });
	await page.goto(detailPath);
	await page.locator('html').evaluate((element) => element.classList.add('dark'));
	const branches = page.getByRole('list', { name: 'สาขาการดำเนินการ' });
	await expect(branches.getByRole('listitem')).toHaveCount(4);
	await page.getByRole('button', { name: 'ดำเนินการแล้ว', exact: true }).click();
	await page
		.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })
		.fill('รายงานผลของสาขาที่ได้รับมอบหมาย');
	await page.getByRole('button', { name: 'ยืนยันดำเนินการแล้ว', exact: true }).click();
	await expect(branches).toContainText('งานรายการนี้เสร็จแล้ว');
	await expect(branches).toContainText('รอผู้รับที่จำเป็นดำเนินการ');
	await expect(branches).toContainText('รอดำเนินการ');
	await expect(
		page.getByText(
			'ผู้รับที่จำเป็นและการตรวจทานครบแล้ว เรื่องดำเนินการเสร็จสิ้น ไม่ต้องส่งกลับตรวจรับ'
		)
	).toHaveCount(0);
	expect(api.actions[0]).toMatchObject({ assignmentId: childAssignmentId, action: 'execute' });
	await expect(page.getByText('รายการของ ครูเจ้าของ ทดสอบ', { exact: true })).toBeVisible();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await capture(page, '05-parallel-branch-status-mobile-dark.png');
});

test('directed register progress keeps branch tracking distinct from numbered template steps', async ({
	page
}) => {
	await mockDirectedCorrespondence(page, { scenario: 'branches' });
	await page.goto('/staff/correspondence');
	const register = page.getByTestId('correspondence-register');
	await expect(register).toContainText('กำลังดำเนินการตามการสั่งการ');
	await expect(register).not.toContainText('ขั้น 1/');
	await expect(register).toContainText('ครูเจ้าของ ทดสอบ');
});

test('mobile instructions keep recipient type and review controls readable and reachable', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 960 });
	const api = await mockDirectedCorrespondence(page, { scenario: 'decide' });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true }).click();
	const kind = page.getByRole('button', { name: 'กลุ่มผู้รับ', exact: true });
	await expect(kind).toContainText('บุคลากร');
	await expect(kind).toBeVisible();
	await expect(
		page.getByRole('checkbox', { name: 'บุคลากรอ่านอย่างเดียว', exact: true })
	).toHaveCount(0);
	const width = await kind.evaluate((element) => element.getBoundingClientRect().width);
	expect(width).toBeGreaterThan(200);
	await page.getByRole('checkbox', { name: 'อนุญาตให้มอบหมายต่อ', exact: true }).check();
	await addRequiredRecipient(page);
	await expect(page.getByRole('list', { name: 'รายการมอบหมาย' })).toContainText('มอบหมายต่อได้');
	await page
		.getByRole('checkbox', { name: 'ให้ผู้ตรวจทานตรวจผลหลังผู้รับที่จำเป็นดำเนินการครบ' })
		.check();
	await choosePerson(page, 'ผู้ตรวจทาน *', 'ครูเจ้าของ ทดสอบ');
	await page.getByRole('button', { name: 'ยืนยันพิจารณาสั่งการ', exact: true }).click();
	await expect.poll(() => api.actions.length).toBe(1);
	expect(api.actions[0].routing?.recipients[0]).toMatchObject({
		required: true,
		allowOnwardAssignment: true
	});
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

for (const recoverReviewer of [false, true])
	test(`manager recovery replaces only the server-selected ${recoverReviewer ? 'reviewer' : 'recipient'} slot with a required reason`, async ({
		page
	}) => {
		const api = await mockDirectedCorrespondence(page, { scenario: 'recovery', recoverReviewer });
		await page.goto(detailPath);
		const label = recoverReviewer ? 'เปลี่ยนผู้ตรวจทาน' : 'เปลี่ยนผู้รับผิดชอบ';
		await expect(page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'ดำเนินการแล้ว', exact: true })).toHaveCount(0);
		await page.getByRole('button', { name: label, exact: true }).click();
		await expect(
			page.getByRole('button', { name: 'ผู้รับเดิมที่ต้องเปลี่ยน', exact: true })
		).toHaveCount(0);
		await choosePerson(page, 'ผู้รับผิดชอบใหม่', 'ครูสำรอง ทดสอบ');
		await page.getByRole('button', { name: `ยืนยัน${label}`, exact: true }).click();
		await expect(page.getByTestId('correspondence-action')).toContainText(
			'กรอกความคิดเห็น / เหตุผล'
		);
		expect(api.actions).toHaveLength(0);
		await page
			.getByLabel('ความคิดเห็น / เหตุผล *', { exact: true })
			.fill('ผู้รับเดิมไม่ปฏิบัติหน้าที่ เปลี่ยนผู้รับผิดชอบตามคำสั่ง');
		await page.getByRole('button', { name: `ยืนยัน${label}`, exact: true }).click();
		await expect.poll(() => api.actions.length).toBe(1);
		expect(api.actions[0]).toMatchObject({
			assignmentId,
			action: 'reassign',
			reassignUserId: '55000000-0000-4000-8000-000000000908',
			comment: 'ผู้รับเดิมไม่ปฏิบัติหน้าที่ เปลี่ยนผู้รับผิดชอบตามคำสั่ง'
		});
		expect(api.actions[0].reassignFromUserId).toBeUndefined();
		await expect(
			page.getByText('เปลี่ยนผู้รับจาก ครูเดิม ณ วันมอบหมาย เป็น ครูสำรอง ณ วันมอบหมาย', {
				exact: true
			})
		).toBeVisible();
		expect(api.actions[0].routing).toBeUndefined();
		await expect(page.getByRole('button', { name: 'พิจารณาสั่งการ', exact: true })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'ดำเนินการแล้ว', exact: true })).toHaveCount(0);
	});

test('a completed book permits optional copied acknowledgement without evidence or reopening work', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page, { scenario: 'copy', terminalCopy: true });
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'เปิดเอกสาร', exact: true }).click();
	await expect(page.getByRole('radio', { name: 'ข้อความ', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'รับทราบ', exact: true }).click();
	await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveCount(0);
	await expect(page.getByLabel('หลักฐานประกอบ', { exact: true })).toHaveCount(0);
	await expect(page.getByTestId('signature-pad')).toHaveCount(0);
	await page.getByRole('button', { name: 'ยืนยันรับทราบ', exact: true }).click();
	await expect.poll(() => api.actions.length).toBe(1);
	expect(api.actions[0]).toMatchObject({
		assignmentId,
		action: 'acknowledge',
		annotations: [],
		evidenceFileIds: []
	});
	expect(api.actions[0].comment).toBeUndefined();
	expect(api.actions[0].signature).toBeUndefined();
	expect(api.detail().record.status).toBe('completed');
});

test('a root decision-maker assignment waiting for its reviewer is labelled as review of results', async ({
	page
}) => {
	const api = await mockDirectedCorrespondence(page, { scenario: 'review' });
	await page.goto(detailPath);
	await expect(
		page.getByRole('button', { name: 'พิจารณาเสร็จและจบเรื่อง', exact: true })
	).toHaveCount(0);
	await page.getByRole('button', { name: 'ตรวจทานผล', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ตรวจทานผล', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ยืนยันตรวจทานผล', exact: true }).click();
	await expect.poll(() => api.actions.length).toBe(1);
	expect(api.actions[0]).toMatchObject({ assignmentId, action: 'review' });
});
test('accepted branch and routed recipient names remain readable after resend replaces the graph', async ({
	page
}) => {
	await mockDirectedCorrespondence(page, { scenario: 'decide', historicalContext: true });
	await page.goto(detailPath);
	await expect(
		page.getByText('รายการของ หัวหน้างานเดิม ณ วันสั่งการ', { exact: true })
	).toBeVisible();
	await expect(
		page.getByText('มอบหมายถึง ผู้รับคำสั่งเดิม ณ วันส่ง', { exact: true })
	).toBeVisible();
	await expect(page.getByText('คำสั่งก่อนส่งกลับแก้ไข', { exact: true })).toBeVisible();
});
