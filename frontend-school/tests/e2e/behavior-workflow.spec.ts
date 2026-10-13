import { expect, test } from '@playwright/test';
import { PERMISSIONS } from '../../src/lib/permissions/registry';
import {
	mockBehavior,
	behaviorPath,
	behaviorTerm,
	behaviorStudent,
	secondStudent,
	otherStudent,
	meritItem,
	entries
} from './fixtures/behavior-route-data';

test('reader loads history without settings or action-only selector reads', async ({ page }) => {
	const api = await mockBehavior(page, { permissions: [PERMISSIONS.BEHAVIOR_READ_SCHOOL] });
	await page.goto(behaviorPath());
	await expect(page.getByRole('heading', { name: 'พฤติกรรมนักเรียน', exact: true })).toBeVisible();
	await expect(page.getByText('ช่วยจัดห้อง', { exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: 'บันทึกพฤติกรรม', exact: true })).toHaveCount(0);
	expect(api.reads).not.toContain('/api/behavior/settings');
	expect(api.reads).not.toContain('/api/behavior/options');
	expect(api.reads).not.toContain('/api/behavior/students');
});

test('mixed room selection deduplicates and preserves draft after roster conflict', async ({
	page
}) => {
	const api = await mockBehavior(page, { failBatch: true });
	await page.goto(behaviorPath('/new'));
	await page.getByRole('button', { name: 'รายการพฤติกรรม', exact: true }).click();
	await page.getByRole('option', { name: 'ช่วยงานโรงเรียน · 5 คะแนน', exact: true }).click();
	await page.getByRole('button', { name: 'ห้องเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกทั้งหมดตามตัวกรอง 2 คน', exact: true }).click();
	await page.getByRole('button', { name: 'ห้องเรียน', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/2', exact: true }).click();
	await page.getByRole('checkbox', { name: 'เลือก นักเรียน สาม', exact: true }).check();
	await page.getByLabel('รายละเอียดเพิ่มเติม', { exact: true }).fill('รายละเอียดที่ครูพิมพ์');
	await page.getByRole('button', { name: 'บันทึกพฤติกรรม', exact: true }).click();
	await expect(page.getByText(/รายชื่อเปลี่ยน กรุณาตรวจใหม่/)).toBeVisible();
	await expect(page.getByLabel('รายละเอียดเพิ่มเติม', { exact: true })).toHaveValue(
		'รายละเอียดที่ครูพิมพ์'
	);
	expect(api.batches[0].studentIds.sort()).toEqual(
		[behaviorStudent, secondStudent, otherStudent].sort()
	);
	expect(api.batches[0].itemId).toBe(meritItem);
	expect(api.batches[0].itemRowVersion).toBe(1);
	expect(api.batches[0].detail).toBe('รายละเอียดที่ครูพิมพ์');
});

test('split cycle summary avoids showing mismatched current net', async ({ page }) => {
	await mockBehavior(page, { splitCycles: true });
	await page.goto(behaviorPath() + `&studentId=${behaviorStudent}`);
	await expect(page.getByText('ต่างช่วงเวลา', { exact: true })).toBeVisible();
	await expect(page.getByText('สุทธิในช่วงเดียวกัน', { exact: false })).toContainText('12');
});

test('teacher can cancel own manual entry while Attendance links open exact source', async ({
	page
}) => {
	const api = await mockBehavior(page);
	await page.goto(behaviorPath());
	await page
		.getByRole('checkbox', { name: 'เลือกยกเลิก นักเรียน หนึ่ง ช่วยงานโรงเรียน', exact: true })
		.check();
	await page.getByRole('button', { name: 'ยกเลิกที่เลือก 1 รายการ', exact: true }).click();
	await page.getByLabel('เหตุผล *', { exact: true }).fill('เลือกนักเรียนผิด');
	await page.getByRole('button', { name: 'ยืนยันยกเลิก', exact: true }).click();
	await expect(page.getByText('ยกเลิกแล้ว', { exact: true })).toBeVisible();
	expect(api.cancellations[0].entryIds).toEqual([entries[0].id]);
	await expect(page.getByRole('link', { name: 'เปิดเช็กชื่อต้นทาง', exact: true })).toHaveAttribute(
		'href',
		new RegExp(`/staff/attendance/sessions/${entries[1].sourceSessionId}`)
	);
});

test('cleanup requires a matching preview and follows durable job to completion', async ({
	page
}) => {
	const api = await mockBehavior(page, { pendingCleanup: true });
	await page.goto(behaviorPath('/cleanup'));
	await page.getByRole('button', { name: 'ตรวจผลกระทบก่อนดำเนินการ', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'ผลกระทบที่ตรวจแล้ว', exact: true })
	).toBeVisible();
	await page.getByLabel('เหตุผล *', { exact: true }).fill('ปิดรอบเดือน');
	await page.getByRole('button', { name: 'ดำเนินการตามผลที่ตรวจ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันดำเนินการ', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'จัดการข้อมูลสำเร็จ', exact: true })
	).toBeVisible();
	expect(api.cleanups[0].mode).toBe('archive');
	expect(api.cleanups[0].scope.academicTermId).toBe(behaviorTerm);
	expect(api.cleanups[0].reason).toBe('ปิดรอบเดือน');
});

for (const theme of ['light', 'dark'])
	test(`group form stays usable on mobile in ${theme}`, async ({ page }) => {
		await page.setViewportSize({ width: 375, height: 812 });
		await mockBehavior(page);
		await page.addInitScript((value) => localStorage.setItem('theme', value), theme);
		await page.goto(behaviorPath('/new'));
		await expect(page.getByRole('button', { name: 'บันทึกพฤติกรรม', exact: true })).toBeVisible();
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
	});
