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

async function chooseAction(page: Page, name: string) {
	const direct = page.getByRole('button', { name, exact: true });
	if (await direct.isVisible()) await direct.click();
	else {
		await page.getByRole('button', { name: 'เพิ่มเติม', exact: true }).click();
		await page.getByRole('menuitem', { name, exact: true }).click();
	}
}

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
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	expect(api.count('/api/correspondence/options')).toBe(1);
	await page.getByRole('radio', { name: 'ใช้แม่แบบลำดับงาน', exact: true }).click();
	await page.getByRole('button', { name: 'ลำดับการดำเนินงาน' }).click();
	await page.getByRole('option', { name: 'เวียนหนังสือมาตรฐาน' }).click();
	await page.getByRole('checkbox', { name: 'ครูเจ้าของ ทดสอบ', exact: true }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ตรวจสอบก่อนส่ง' })).toContainText(
		'หนังสือฉบับใหม่'
	);
	await page.getByRole('button', { name: 'ยืนยันส่งหนังสือ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(correspondenceId));
	await expect(page.getByRole('heading', { name: 'หนังสือฉบับใหม่', exact: true })).toBeVisible();
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
		expect(payload.signature?.strokes?.[0].points.length).toBeGreaterThan(2);
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

test('template edits create a new version with organization position recipients and retain running snapshot', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	await page.goto('/staff/correspondence/templates');
	await expect(page.getByTestId('process-templates')).toContainText('เวียนหนังสือมาตรฐาน');
	expect(api.count('/api/correspondence/options')).toBe(0);
	await page.getByRole('button', { name: 'แก้ไขแม่แบบ', exact: true }).click();
	await page.getByLabel('ชื่อแม่แบบ *', { exact: true }).fill('แม่แบบอนุมัติใหม่');
	await page.getByRole('button', { name: 'กลุ่มผู้รับ', exact: true }).click();
	await page.getByRole('option', { name: 'ตำแหน่งในหน่วยงาน', exact: true }).click();
	await page.getByRole('checkbox', { name: 'งานบริหารทั่วไป · หัวหน้างาน' }).check();
	await page.getByRole('button', { name: 'เพิ่มผู้รับ (1)', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกแม่แบบรุ่นใหม่', exact: true }).click();
	await expect(page.getByTestId('process-templates')).toContainText('แม่แบบอนุมัติใหม่');
	await expect(page.getByTestId('process-templates')).toContainText('รุ่น 2');
	await expect(page.getByTestId('process-templates')).not.toContainText('internal_fixture_code');
	expect(api.detail().instance?.templateVersion).toBe(1);
});

test('new template generates a hidden code accepted by the real backend character contract', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { permissions: ['process.manage.school'] });
	await page.goto('/staff/correspondence/templates');
	expect(api.count('/api/correspondence/options')).toBe(0);
	await page.getByRole('button', { name: 'สร้างแม่แบบ', exact: true }).click();
	await page.getByLabel('ชื่อแม่แบบ *', { exact: true }).fill('แม่แบบใหม่ของโรงเรียน');
	await page.getByRole('button', { name: 'บันทึกแม่แบบรุ่นใหม่', exact: true }).click();
	await expect(page.getByTestId('process-templates')).toContainText('แม่แบบใหม่ของโรงเรียน');
	expect(api.templateWrites[0].code).toMatch(/^flow_[a-f0-9]{32}$/);
	expect(api.count('/api/correspondence/options')).toBe(1);
});

test('personal drawn signature saves as a private owning-resource image for reuse', async ({
	page
}) => {
	const api = await mockCorrespondence(page);
	await page.goto('/staff/correspondence/signatures');
	await page.getByRole('button', { name: 'เพิ่มลายมือชื่อ', exact: true }).click();
	await page.getByLabel('ชื่อลายมือชื่อ *', { exact: true }).fill('ลายมือชื่อสำรอง');
	await draw(page);
	await page.getByRole('button', { name: 'บันทึกลายมือชื่อ', exact: true }).click();
	await expect(page.getByTestId('signature-library')).toContainText('ลายมือชื่อสำรอง');
	expect(api.uploads[0]).toContain('correspondence_signature');
	expect(api.uploads[0]).toContain('55000000-0000-4000-8000-000000000010');
	expect(api.assetWrites[0].label).toBe('ลายมือชื่อสำรอง');
});

test('create failure retains inputs and original uploads only after a resource exists', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'draft', failCreate: true });
	await page.goto('/staff/correspondence/new');
	await page.getByLabel('เรื่อง *', { exact: true }).fill('หนังสือพร้อมเอกสาร');
	await page.getByLabel('ผู้ส่ง / หน่วยงานต้นทาง *').fill('หน่วยงานต้นทาง');
	await page.getByLabel('เอกสารต้นฉบับ PDF หรือภาพ').setInputFiles({
		name: 'original.pdf',
		mimeType: 'application/pdf',
		buffer: Buffer.from('%PDF synthetic')
	});
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page.getByText('บันทึกไม่สำเร็จ กรุณาลองอีกครั้ง', { exact: true })).toBeVisible();
	await expect(page.getByLabel('เรื่อง *', { exact: true })).toHaveValue('หนังสือพร้อมเอกสาร');
	expect(api.uploads).toHaveLength(0);
	await page.getByRole('button', { name: 'บันทึกฉบับร่าง', exact: true }).click();
	await expect(page).toHaveURL(/correspondence\/new$/);
	await expect(page.getByText(/บันทึกฉบับร่างแล้ว ยังไม่ได้ส่ง/)).toBeVisible();
	expect(api.uploads).toHaveLength(1);
	expect(api.uploads[0]).toContain(correspondenceId);
	expect(api.uploads[0]).toContain('correspondence_document');
	expect(api.writes.at(-1)?.path).toMatch(/\/revisions$/);
});

test('revoked identity disposes a delayed detail and never shows stale private content', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { holdDetail: true });
	await page.goto(detailPath);
	await expect.poll(() => api.count(`/api/correspondence/${correspondenceId}`)).toBe(1);
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/auth.ts';
		const { authStore } = await import(modulePath);
		authStore.setUser(
			{
				id: '55000000-0000-4000-8000-000000000010',
				firstName: 'เจ้าของ',
				lastName: 'โปรไฟล์',
				role: 'staff',
				user_type: 'staff',
				status: 'active'
			},
			[]
		);
	});
	api.release();
	await expect(page).toHaveURL(/\/403/);
	await expect(page.getByTestId('correspondence-detail')).toHaveCount(0);
	await expect(page.getByText('หนังสือทดสอบการดำเนินงาน', { exact: true })).toHaveCount(0);
	expect(api.count('/api/correspondence/options')).toBe(0);
});

test('server-authorized manager recovery selects pending source and reason without acting as signer', async ({
	page
}) => {
	const api = await mockCorrespondence(page, {
		recovery: true,
		permissions: ['correspondence.manage.school']
	});
	await page.goto(detailPath);
	await chooseAction(page, 'ส่งต่อผู้รับผิดชอบ');
	await page.getByRole('button', { name: 'ผู้รับเดิมที่ต้องเปลี่ยน', exact: true }).click();
	await page.getByRole('option', { name: 'ครูผู้รับคนที่สอง', exact: true }).click();
	await page.getByRole('button', { name: 'ผู้รับผิดชอบใหม่', exact: true }).click();
	await page.getByRole('option', { name: 'ครูเจ้าของ ทดสอบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งต่อผู้รับผิดชอบ', exact: true }).click();
	await expect(page.getByTestId('correspondence-action')).toContainText('กรอกความคิดเห็น / เหตุผล');
	expect(api.writes).toHaveLength(0);
	await page
		.getByLabel('ความคิดเห็น / เหตุผล *', { exact: true })
		.fill('เปลี่ยนผู้รับเนื่องจากย้ายงาน');
	await page.getByRole('button', { name: 'ยืนยันส่งต่อผู้รับผิดชอบ', exact: true }).click();
	await expect(page.getByTestId('correspondence-action')).toHaveCount(0);
	const payload = api.writes.at(-1)!.payload as ProcessActionRequest;
	expect(payload.action).toBe('reassign');
	expect(payload.reassignFromUserId).toBe('55000000-0000-4000-8000-000000000020');
	expect(payload.reassignUserId).toBe('55000000-0000-4000-8000-000000000010');
	expect(payload.signature).toBeUndefined();
	expect(payload.annotations).toEqual([]);
	expect(api.count('/api/correspondence/options')).toBe(1);
});

test('reassignment A to B to A preserves completed snapshots and offers only pending user semantics', async ({
	page
}) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	const api = await mockCorrespondence(page, { mode: 'acknowledge' });
	api.applyReassignmentRoundTrip();
	await page.goto(detailPath);
	const step = page.getByTestId('correspondence-current-step');
	await expect(
		step.getByRole('list', { name: 'ผู้รับขั้นตอนปัจจุบัน' }).getByRole('listitem')
	).toHaveCount(3);
	await expect(step.getByText('ดำเนินการแล้ว', { exact: true })).toHaveCount(2);
	await expect(step.getByText('รอดำเนินการ', { exact: true })).toHaveCount(1);
	await expect(page.getByText('ส่งต่อและรับคืนหน้าที่', { exact: true })).toHaveCount(2);
	for (const label of ['แหล่งอ้างอิงแรก', 'แหล่งอ้างอิงอีกชื่อ'])
		await expect(page.getByRole('link', { name: label, exact: true })).toHaveAttribute(
			'href',
			'https://school.example/source'
		);
	for (const label of ['เอกสารแนบ 1', 'เอกสารแนบ 2', 'หลักฐาน 1', 'หลักฐาน 2'])
		await expect(page.getByRole('button', { name: label, exact: true })).toBeVisible();
	await chooseAction(page, 'ส่งต่อผู้รับผิดชอบ');
	await expect(
		page.getByRole('button', { name: 'ผู้รับเดิมที่ต้องเปลี่ยน', exact: true })
	).toHaveCount(0);
	await page.getByRole('button', { name: 'ผู้รับผิดชอบใหม่', exact: true }).click();
	await expect(page.getByRole('option', { name: 'ครูผู้รับคนที่สอง', exact: true })).toBeVisible();
	await expect(page.getByRole('option', { name: 'ครูเจ้าของ ทดสอบ', exact: true })).toHaveCount(0);
	expect(errors).toEqual([]);
});

for (const delivery of ['delayed', 'ambiguous'] as const)
	test(`same-revision accepted annotation reconciles ${delivery} response without losing draft or consent`, async ({
		page
	}) => {
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		const api = await mockCorrespondence(page, {
			holdActionResponse: delivery === 'delayed',
			abortAcceptedAction: delivery === 'ambiguous',
			acceptedAnnotationColor: '#b91c1c',
			nextStepAfterAccept: delivery === 'ambiguous'
		});
		await page.goto(detailPath);
		await openSource(page);
		await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
		await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('หมายเหตุที่เซิร์ฟเวอร์รับแล้ว');
		await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
		await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
		await page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true }).fill('ความคิดเห็นที่ยังเก็บไว้');
		await draw(page);
		await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
		await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
		await expect.poll(() => api.detail().record.revision).toBe(2);
		if (delivery === 'ambiguous') {
			await expect(
				page.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด', exact: true })
			).toBeVisible();
			await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('ร่างอื่นที่ยังไม่ยืนยัน');
			await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
			await page.getByRole('button', { name: 'ตรวจสอบข้อมูลล่าสุด', exact: true }).click();
		} else {
			await page.evaluate(async () => {
				const modulePath = '/src/lib/stores/work.ts';
				const { workStore } = await import(/* @vite-ignore */ modulePath);
				await workStore.refreshSilently();
			});
		}
		await expect(
			page.getByText('รายการมีการเปลี่ยนแปลง กรุณาตรวจเอกสารและขั้นตอนล่าสุด')
		).toBeVisible();
		await expect(
			page.locator('svg text').filter({ hasText: 'หมายเหตุที่เซิร์ฟเวอร์รับแล้ว' })
		).toHaveCount(1);
		await expect(
			page.locator('svg text').filter({ hasText: 'หมายเหตุที่เซิร์ฟเวอร์รับแล้ว' })
		).toHaveAttribute('fill', '#b91c1c');
		await expect(page.getByLabel('ความคิดเห็น / เหตุผล', { exact: true })).toHaveValue(
			'ความคิดเห็นที่ยังเก็บไว้'
		);
		await expect(page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ })).toBeChecked();
		expect(errors).toEqual([]);
		if (delivery === 'delayed') {
			api.release();
			await expect(page.getByTestId('correspondence-action')).toHaveCount(0);
			await expect(
				page.locator('svg text').filter({ hasText: 'หมายเหตุที่เซิร์ฟเวอร์รับแล้ว' })
			).toHaveCount(1);
		} else {
			await expect(
				page.locator('svg text').filter({ hasText: 'ร่างอื่นที่ยังไม่ยืนยัน' })
			).toBeVisible();
			await page.getByRole('button', { name: 'เลิกทำหมายเหตุล่าสุด', exact: true }).click();
			await expect(
				page.locator('svg text').filter({ hasText: 'ร่างอื่นที่ยังไม่ยืนยัน' })
			).toHaveCount(0);
			await expect(
				page.locator('svg text').filter({ hasText: 'หมายเหตุที่เซิร์ฟเวอร์รับแล้ว' })
			).toHaveCount(1);
			await expect(page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ })).toBeChecked();
			await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('ร่างอื่นที่ยังไม่ยืนยัน');
			await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
			await page
				.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง', exact: true })
				.click();
			await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
			await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
			await expect(page.getByTestId('correspondence-action')).toHaveCount(0);
			const next = api.writes.at(-1)!.payload as ProcessActionRequest;
			const prior = api.writes[0].payload as ProcessActionRequest;
			expect(next.expectedRevision).toBe(2);
			const nextAnnotations = next.annotations ?? [];
			const priorAnnotations = prior.annotations ?? [];
			expect(priorAnnotations).toHaveLength(1);
			expect(nextAnnotations.map((annotation) => annotation.text)).toEqual([
				'ร่างอื่นที่ยังไม่ยืนยัน'
			]);
			expect(nextAnnotations[0].id).not.toBe(priorAnnotations[0].id);
			await expect(
				page.locator('svg text').filter({ hasText: 'หมายเหตุที่เซิร์ฟเวอร์รับแล้ว' })
			).toHaveCount(1);
			await expect(
				page.locator('svg text').filter({ hasText: 'ร่างอื่นที่ยังไม่ยืนยัน' })
			).toHaveCount(1);
		}
	});

test('accepted signing request freezes related marks while preserving independent viewer controls', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { holdAction: true });
	await page.goto(detailPath);
	await openSource(page);
	await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
	await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('หมายเหตุที่ยืนยัน');
	await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
	await page.getByRole('button', { name: 'ลงนาม', exact: true }).click();
	await draw(page);
	await page.getByRole('checkbox', { name: /ยืนยันใช้ลายมือชื่อนี้/ }).check();
	await page.getByRole('button', { name: 'ยืนยันลงนาม', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await expect(page.getByRole('radio', { name: 'ปากกา', exact: true })).toBeDisabled();
	await expect(page.getByLabel('ข้อความกำกับ', { exact: true })).toBeDisabled();
	await expect(
		page.getByRole('button', { name: 'เลิกทำหมายเหตุล่าสุด', exact: true })
	).toBeDisabled();
	await expect(page.getByRole('button', { name: 'หมุน 90°', exact: true })).toBeEnabled();
	api.release();
	await expect(page.getByText('หลักฐานลงนาม', { exact: true })).toBeVisible();
	expect((api.writes[0].payload as ProcessActionRequest).annotations).toHaveLength(1);
});

test('delayed accepted action preserves a newer rendered document and clears its accepted draft', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { mode: 'acknowledge', holdActionResponse: true });
	await page.goto(detailPath);
	await openSource(page);
	await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
	await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('หมายเหตุที่รับทราบฉบับเดิม');
	await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
	await page.getByRole('button', { name: 'รับทราบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันรับทราบ', exact: true }).click();
	await expect.poll(() => api.detail().record.revision).toBe(2);
	api.advanceDocument();
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/work.ts';
		const { workStore } = await import(/* @vite-ignore */ modulePath);
		await workStore.refreshSilently();
	});
	await expect(page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง' })).toContainText(
		'ฉบับปัจจุบัน (2)'
	);
	api.release();
	await expect(page.getByTestId('correspondence-action')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง' })).toContainText(
		'ฉบับปัจจุบัน (2)'
	);
	await openSource(page);
	await expect(
		page.locator('svg text').filter({ hasText: 'หมายเหตุที่รับทราบฉบับเดิม' })
	).toHaveCount(0);
	await page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง' }).click();
	await page.getByRole('option', { name: /ฉบับ 1 ·/ }).click();
	await openSource(page);
	await expect(
		page.locator('svg text').filter({ hasText: 'หมายเหตุที่รับทราบฉบับเดิม' })
	).toBeVisible();
});

test('applied register filter clears a previous context before its delayed result and restores history', async ({
	page
}) => {
	const api = await mockCorrespondence(page, { holdListRefresh: true });
	await page.goto('/staff/correspondence');
	await expect(page.getByTestId('correspondence-register')).toContainText(
		'หนังสือทดสอบการดำเนินงาน'
	);
	await page.getByLabel('ค้นหาเรื่อง / เลขทะเบียน / เลขที่ต้นทาง / ผู้ส่ง').fill('ไม่มีเรื่องนี้');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect.poll(() => api.count('/api/correspondence')).toBe(2);
	await expect(page.getByTestId('correspondence-register')).toHaveAttribute('aria-busy', 'true');
	await expect(page.getByTestId('correspondence-register')).not.toContainText(
		'หนังสือทดสอบการดำเนินงาน'
	);
	api.release();
	await expect(page.getByTestId('correspondence-register')).toContainText(
		'ไม่พบหนังสือตามเงื่อนไข'
	);
	await expect(page).toHaveURL(/search=/);
	await page.goBack();
	await expect(page.getByLabel('ค้นหาเรื่อง / เลขทะเบียน / เลขที่ต้นทาง / ผู้ส่ง')).toHaveValue('');
	await expect(page.getByTestId('correspondence-register')).toContainText(
		'หนังสือทดสอบการดำเนินงาน'
	);
});

for (const theme of ['light', 'dark'])
	test(`mobile ${theme} has no page overflow and fullscreen returns to reachable action panel`, async ({
		page
	}) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.addInitScript((value) => {
			localStorage.setItem('ui-preferences', JSON.stringify({ theme: value }));
			document.documentElement.classList.toggle('dark', value === 'dark');
		}, theme);
		await mockCorrespondence(page);
		await page.goto(detailPath);
		await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
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
		const dimensions = await page.locator('canvas').evaluate((canvas) => {
			if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Expected PDF canvas');
			return { width: canvas.width, height: canvas.height };
		});
		expect(dimensions.width).toBeGreaterThan(dimensions.height);
		await page.getByRole('radio', { name: 'ข้อความ', exact: true }).click();
		await page.getByLabel('ข้อความกำกับ', { exact: true }).fill('ตรวจสอบ');
		await page.getByRole('button', { name: 'เพิ่มข้อความ', exact: true }).click();
		await expect(page.locator('svg text').filter({ hasText: 'ตรวจสอบ' })).toHaveAttribute(
			'x',
			'60'
		);
		await page.getByRole('button', { name: 'หมุน 90°' }).click();
		await expect(page.locator('svg text').filter({ hasText: 'ตรวจสอบ' })).toHaveAttribute(
			'x',
			'60'
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
	await chooseAction(page, 'เปิดอ่าน');
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
	api.addCommittedAnnotation('หมายเหตุฉบับเก่า');
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
	await expect(page.locator('svg text').filter({ hasText: 'หมายเหตุฉบับเก่า' })).toBeVisible();
	await expect(page.locator('svg text').filter({ hasText: 'ร่างจากฉบับเดิม' })).toHaveCount(0);
	await page.getByRole('button', { name: 'ฉบับเอกสารที่แสดง' }).click();
	await page.getByRole('option', { name: 'ฉบับปัจจุบัน (2)', exact: true }).click();
	await openSource(page);
	await expect(page.locator('svg text').filter({ hasText: 'ร่างจากฉบับเดิม' })).toBeVisible();
	await expect(page.locator('svg text').filter({ hasText: 'หมายเหตุฉบับเก่า' })).toHaveCount(0);
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
		expect(payload.signature?.strokes?.[0].points.length).toBeGreaterThan(1);
		expect(
			payload.signature?.strokes?.[0].points.every(
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
				localStorage.setItem('ui-preferences', JSON.stringify({ theme: value }));
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
			const geometry = await page.getByTestId('document-workspace').evaluate((element) => ({
				left: element.getBoundingClientRect().left,
				right: element.getBoundingClientRect().right,
				width: innerWidth,
				scroll: document.documentElement.scrollWidth
			}));
			expect(geometry.left).toBeGreaterThanOrEqual(0);
			expect(geometry.right).toBeLessThanOrEqual(geometry.width + 1);
			expect(geometry.scroll).toBeLessThanOrEqual(geometry.width);
			await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
			const brightness = await page
				.getByTestId('correspondence-current-step')
				.evaluate((element) => {
					const canvas = document.createElement('canvas');
					const context = canvas.getContext('2d')!;
					context.fillStyle = getComputedStyle(element).backgroundColor;
					context.fillRect(0, 0, 1, 1);
					const data = context.getImageData(0, 0, 1, 1).data;
					return data[0] + data[1] + data[2];
				});
			if (theme === 'dark') expect(brightness).toBeLessThan(400);
			else expect(brightness).toBeGreaterThan(500);
			await page.setViewportSize({ width, height: width === 1440 ? 1440 : 1000 });
			await page.evaluate(() => {
				for (const element of document.querySelectorAll<HTMLElement>('*')) {
					if (element.scrollTop > 0) element.scrollTop = 0;
				}
			});
			await page.screenshot({ path: testInfo.outputPath(`signed-${theme}-${width}.png`) });
			await page
				.getByTestId('document-page')
				.screenshot({ path: testInfo.outputPath(`document-${theme}-${width}.png`) });
		});
