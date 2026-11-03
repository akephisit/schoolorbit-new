import { expect, test } from '@playwright/test';
import { join } from 'node:path';
import {
	mockYearManagement,
	chooseOption,
	openRoster,
	setAllDecisions,
	yearUrl,
	id
} from './fixtures/year-management';
test.use({ serviceWorkers: 'block' });

test('one complete preview confirms all manual outcomes and leaves the new year prepared', async ({
	page
}) => {
	const observed = await mockYearManagement(page);
	await openRoster(page);
	if (process.env.E2E_ARTIFACT_DIR)
		await page.screenshot({
			path: join(process.env.E2E_ARTIFACT_DIR, 'roster-preview-desktop.png'),
			fullPage: true
		});
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true })
	).toBeDisabled();
	await setAllDecisions(page);
	await dialog.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true }).click();
	await expect(
		page.getByText('เตรียมรายชื่อนักเรียนปีใหม่แล้ว 5 คน · ยังไม่เริ่มใช้', { exact: true })
	).toBeVisible();
	await expect(page.locator('[data-slot="dialog-content"]')).toHaveCount(0);
	if (process.env.E2E_ARTIFACT_DIR)
		await page.screenshot({
			path: join(process.env.E2E_ARTIFACT_DIR, 'prepared-year-desktop.png'),
			fullPage: true
		});
	expect(observed.confirms).toHaveLength(1);
	expect(observed.confirms[0].preview.decisions.map((row) => row.decision.outcome)).toEqual([
		'promote',
		'repeat',
		'graduate',
		'transfer_out',
		'hold'
	]);
	expect(observed.confirms[0].preview.decisions[0]).toMatchObject({
		studentAcademicYearId: id(40),
		decision: { targetGradeLevelId: id(11), targetStudyProgramId: id(22) }
	});
	expect(observed.transitions).toEqual([]);
	expect(observed.reads.some((path) => /result|aggregate|promotion-polic/.test(path))).toBe(false);
});
test('stale preview preserves decisions and requires a fresh preview before another confirmation', async ({
	page
}) => {
	const observed = await mockYearManagement(page, { conflict: true });
	await openRoster(page);
	await setAllDecisions(page);
	await page.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true }).click();
	await expect(
		page.getByText('ข้อมูลเปลี่ยนแล้ว กรุณาตรวจตัวอย่างล่าสุด ตัวเลือกที่แก้ไว้ยังอยู่', {
			exact: true
		})
	).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true })
	).toBeDisabled();
	await page.getByRole('button', { name: 'ตรวจตัวอย่างล่าสุด', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true }).click();
	await expect(
		page.getByText('เตรียมรายชื่อนักเรียนปีใหม่แล้ว 5 คน · ยังไม่เริ่มใช้')
	).toBeVisible();
	expect(observed.confirms[1].preview.decisions).toEqual(observed.confirms[0].preview.decisions);
	expect(observed.confirms[1].requestId).not.toEqual(observed.confirms[0].requestId);
});
test('ambiguous confirmation only retries on a click and reuses the exact request', async ({
	page
}) => {
	const observed = await mockYearManagement(page, { uncertain: true });
	await openRoster(page);
	await setAllDecisions(page);
	await page.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลองยืนยันอีกครั้ง', exact: true })).toBeVisible();
	expect(observed.confirms).toHaveLength(1);
	await page.getByRole('button', { name: 'ลองยืนยันอีกครั้ง', exact: true }).click();
	await expect(
		page.getByText('เตรียมรายชื่อนักเรียนปีใหม่แล้ว 5 คน · ยังไม่เริ่มใช้')
	).toBeVisible();
	expect(observed.confirms[1]).toEqual(observed.confirms[0]);
});
test('source closure does not discard an open roster draft', async ({ page }) => {
	const observed = await mockYearManagement(page, { active: true });
	await openRoster(page);
	const dialog = page.getByRole('dialog');
	const row = dialog.locator('section').filter({ hasText: 'ปอ เรียนต่อ' });
	await row.getByLabel('ผลปีใหม่', { exact: true }).click();
	await page.getByRole('option', { name: 'พักไว้', exact: true }).click();
	await dialog.getByRole('button', { name: 'กลับไปหน้าจัดการ', exact: true }).click();
	await page.getByRole('button', { name: 'ปิดปีการศึกษา', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันปิดปีการศึกษา', exact: true }).click();
	await page.getByRole('button', { name: 'ขึ้นปีการศึกษาใหม่', exact: true }).click();
	await expect(
		page
			.getByRole('dialog')
			.locator('section')
			.filter({ hasText: 'ปอ เรียนต่อ' })
			.getByLabel('ผลปีใหม่', { exact: true })
	).toContainText('พักไว้');
	expect(observed.confirms).toEqual([]);
});
test('legacy promotion entrypoints lead to the canonical workspace without old mutation UI', async ({
	page
}) => {
	await mockYearManagement(page);
	for (const suffix of ['promotion', 'promotion/policies']) {
		await page.goto(`/staff/academic/${suffix}?academicYearId=${id(1)}`);
		await expect(page).toHaveURL(yearUrl);
		await expect(
			page.getByRole('heading', { name: 'จัดการปีการศึกษา', exact: true })
		).toBeVisible();
		await expect(
			page.getByRole('button', { name: /คำนวณ|อนุมัติรอบ|สร้างรอบเลื่อนชั้น/ })
		).toHaveCount(0);
	}
});

test('bulk decisions include hidden and filtered pupils across roster pages', async ({ page }) => {
	const observed = await mockYearManagement(page, { studentCount: 26 });
	await openRoster(page);
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('heading', { name: 'นักเรียน 26 S26', exact: true })).toHaveCount(
		0
	);

	const bulk = dialog.getByRole('region', { name: 'กำหนดปลายทางทีละห้อง', exact: true });
	await chooseOption(page, bulk.getByLabel('ผลปีใหม่', { exact: true }), 'พักไว้');
	await bulk.getByRole('button', { name: 'ใช้กับนักเรียนในห้องนี้', exact: true }).click();
	const graduate = dialog.locator('section').filter({ hasText: 'ดาว จบการศึกษา' });
	await chooseOption(page, graduate.getByLabel('ผลปีใหม่', { exact: true }), 'จบการศึกษา');
	await graduate.getByRole('button', { name: 'เสร็จแล้ว', exact: true }).click();
	if (process.env.E2E_ARTIFACT_DIR) {
		await page
			.getByRole('heading', { name: 'ปอ เรียนต่อ S1', exact: true })
			.scrollIntoViewIfNeeded();
		await page.screenshot({
			path: join(process.env.E2E_ARTIFACT_DIR, 'roster-after-bulk-desktop.png'),
			fullPage: true
		});
	}
	await dialog.getByLabel('ค้นหานักเรียน', { exact: true }).fill('นักเรียน 26');
	const hidden = dialog.locator('section').filter({ hasText: 'นักเรียน 26' });
	await expect(hidden).toContainText('พักไว้');
	await expect(hidden.getByRole('button', { name: 'แก้ไข', exact: true })).toBeVisible();
	await dialog.getByRole('button', { name: 'ตรวจตัวอย่างล่าสุด', exact: true }).click();
	await dialog.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true }).click();
	await expect(
		page.getByText('เตรียมรายชื่อนักเรียนปีใหม่แล้ว 26 คน · ยังไม่เริ่มใช้', { exact: true })
	).toBeVisible();
	expect(observed.confirms[0].preview.decisions).toHaveLength(26);
	expect(
		observed.confirms[0].preview.decisions.filter((row) => row.decision.outcome === 'hold')
	).toHaveLength(25);
	expect(
		observed.confirms[0].preview.decisions.find((row) => row.studentAcademicYearId === id(65))
			?.decision.outcome
	).toBe('hold');
});
