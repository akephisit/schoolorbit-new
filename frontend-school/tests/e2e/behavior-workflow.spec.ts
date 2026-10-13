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
	entries,
	summary
} from './fixtures/behavior-route-data';
import type { BehaviorDailyTotal } from '../../src/lib/api/behavior';

test('reader loads history without settings or action-only selector reads', async ({ page }) => {
	const api = await mockBehavior(page, { permissions: [PERMISSIONS.BEHAVIOR_READ_SCHOOL] });
	await page.goto(behaviorPath());
	await expect(page.getByRole('heading', { name: 'พฤติกรรมนักเรียน', exact: true })).toBeVisible();
	await expect(page.getByText('ช่วยจัดห้อง', { exact: true })).toBeVisible();
	await expect(page.getByText(/ระบบอัตโนมัติ/)).toBeVisible();
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
	const api = await mockBehavior(page, { splitCycles: true });
	await page.goto(behaviorPath() + `&studentId=${behaviorStudent}`);
	await expect(page.getByText('ต่างช่วงเวลา', { exact: true })).toBeVisible();
	await expect(page.locator('[aria-label="ประวัติรวมทุกภาคเรียน"]')).not.toContainText('สุทธิ');
	expect(
		api.summaryQueries.every((query) => !new URLSearchParams(query).has('academicTermId'))
	).toBe(true);
	await expect(page.getByText('สุทธิในช่วงเดียวกัน', { exact: false })).toContainText('12');
});

test('summary refresh waits for a historical adjustment before applying canonical totals', async ({
	page
}) => {
	await mockBehavior(page, {
		permissions: [
			PERMISSIONS.BEHAVIOR_READ_SCHOOL,
			PERMISSIONS.BEHAVIOR_CORRECT_SCHOOL,
			PERMISSIONS.BEHAVIOR_DELETE_OWN
		]
	});
	const daily: BehaviorDailyTotal = {
		id: entries[0].id,
		studentId: behaviorStudent,
		academicTermId: behaviorTerm,
		cycleId: summary.meritCycle.id,
		gradeLevelId: behaviorStudent,
		homeroomId: null,
		date: '2026-09-30',
		side: 'merit',
		source: 'manual',
		points: 10,
		rowVersion: 1
	};
	let summaryReads = 0;
	await page.route('**/api/behavior/students/*/summary*', (route) => {
		const updated = ++summaryReads > 1;
		return route.fulfill({
			json: {
				success: true,
				data: {
					...summary,
					history: { merit: updated ? 15 : 10, deduction: 20, net: null },
					lifetime: { merit: updated ? 35 : 30, deduction: 28, net: updated ? 7 : 2 },
					historyTotals: [{ ...daily, points: updated ? 15 : 10, rowVersion: updated ? 2 : 1 }]
				}
			}
		});
	});
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	let adjusting = false;
	await page.route('**/api/behavior/history/adjustments', async (route) => {
		adjusting = true;
		await held;
		await route.fulfill({
			json: { success: true, data: { id: daily.id, totalId: daily.id, points: 15, rowVersion: 2 } }
		});
	});
	await page.goto(behaviorPath() + `&studentId=${behaviorStudent}`);
	const score = page.locator('[aria-label="สรุปคะแนน"]');
	await score.getByText('ประวัติยอดรายวัน (1)', { exact: true }).click();
	await score.getByRole('button', { name: 'ปรับยอด', exact: true }).click();
	await score.getByLabel('จำนวนที่ปรับ (+ เพิ่ม / − ลด)', { exact: true }).fill('5');
	await score.getByLabel('เหตุผล *', { exact: true }).fill('แก้ยอดย้อนหลัง');
	await score.getByRole('button', { name: 'บันทึกการปรับยอด', exact: true }).click();
	await expect.poll(() => adjusting).toBe(true);
	const log = page.locator('[aria-label="ประวัติรายการคะแนน"]');
	await log
		.getByRole('checkbox', { name: 'เลือกยกเลิก นักเรียน หนึ่ง ช่วยงานโรงเรียน', exact: true })
		.check();
	await log.getByRole('button', { name: 'ยกเลิกที่เลือก 1 รายการ', exact: true }).click();
	await log.getByLabel('เหตุผล *', { exact: true }).fill('ยกเลิกรายการผิด');
	await log.getByRole('button', { name: 'ยืนยันยกเลิก', exact: true }).click();
	await expect(log.getByText('ยกเลิกแล้ว', { exact: true })).toBeVisible();
	expect(summaryReads).toBe(1);
	release();
	await expect.poll(() => summaryReads).toBe(2);
	await expect(page.locator('[aria-label="ประวัติรวมทุกภาคเรียน"] dd').first()).toHaveText('15');
	await expect(page.locator('[aria-label="สะสมทั้งหมด"] dd').first()).toHaveText('35');
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
		await page.emulateMedia({ colorScheme: theme === 'dark' ? 'dark' : 'light' });
		await mockBehavior(page);
		await page.addInitScript((value) => localStorage.setItem('theme', value), theme);
		await page.goto(behaviorPath('/new'));
		await expect(page.getByRole('button', { name: 'บันทึกพฤติกรรม', exact: true })).toBeVisible();
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await expect
			.poll(() => page.locator('html').evaluate((el) => el.classList.contains('dark')))
			.toBe(theme === 'dark');
		const choice = page.getByRole('button', { name: 'ประเภท', exact: true });
		await choice.click();
		const option = page.getByRole('option', { name: 'คะแนนความดี', exact: true });
		await expect
			.poll(async () => (await option.boundingBox())?.height ?? 0)
			.toBeGreaterThanOrEqual(44);
		await page.keyboard.press('Escape');
		const checkbox = page.getByRole('checkbox', { name: 'เลือก นักเรียน หนึ่ง', exact: true });
		expect((await checkbox.locator('..').boundingBox())!.height).toBeGreaterThanOrEqual(44);
		await checkbox.check();
		await page.getByText('ตรวจรายชื่อที่เลือก 1 คน', { exact: true }).click();
		for (const target of [
			choice,
			page.getByRole('button', { name: 'ค้นหา', exact: true }),
			page.getByRole('button', { name: 'ล้างการเลือก', exact: true }),
			page.getByRole('button', { name: 'นำ นักเรียน หนึ่ง ออก', exact: true }),
			page.getByRole('button', { name: 'ถัดไป', exact: true })
		]) {
			const bounds = (await target.boundingBox())!;
			expect(bounds.height).toBeGreaterThanOrEqual(44);
			expect(bounds.width).toBeGreaterThanOrEqual(44);
		}
		await page.screenshot({ path: `/tmp/schoolorbit-behavior-${theme}.png`, fullPage: true });
	});

test('pending cancellation freezes filter and selection until original result arrives', async ({
	page
}) => {
	const api = await mockBehavior(page);
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/behavior/entries/cancel', async (route) => {
		await held;
		await route.fallback();
	});
	await page.goto(behaviorPath());
	await page
		.getByRole('checkbox', { name: 'เลือกยกเลิก นักเรียน หนึ่ง ช่วยงานโรงเรียน', exact: true })
		.check();
	await page.getByRole('button', { name: 'ยกเลิกที่เลือก 1 รายการ', exact: true }).click();
	await page.getByLabel('เหตุผล *', { exact: true }).fill('แก้ไขรายการผิด');
	await page.getByRole('button', { name: 'ยืนยันยกเลิก', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ค้นหา', exact: true })).toBeDisabled();
	await expect(
		page.getByRole('checkbox', { name: 'เลือกยกเลิก นักเรียน หนึ่ง ช่วยงานโรงเรียน', exact: true })
	).toBeDisabled();
	release();
	await expect(page.getByText('ยกเลิกแล้ว', { exact: true })).toBeVisible();
	expect(api.cancellations[0].entryIds).toEqual([entries[0].id]);
});

test('a date change during roster reload never labels the older response as current', async ({
	page
}) => {
	const api = await mockBehavior(page);
	await page.goto(behaviorPath('/new'));
	await page.getByRole('button', { name: 'รายการพฤติกรรม', exact: true }).click();
	await page.getByRole('option', { name: 'ช่วยงานโรงเรียน · 5 คะแนน', exact: true }).click();
	await page.getByRole('checkbox', { name: 'เลือก นักเรียน หนึ่ง', exact: true }).check();
	const today = new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(new Date());
	const first = today.slice(0, 8) + (today.endsWith('10') ? '11' : '10'),
		second = today.slice(0, 8) + '12';
	const label = (date: string) =>
		new Intl.DateTimeFormat('th-TH', {
			timeZone: 'Asia/Bangkok',
			weekday: 'long',
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		}).format(new Date(date + 'T12:00:00+07:00'));
	await page.getByRole('button', { name: 'เลือกวันที่', exact: true }).click();
	await page.getByRole('button', { name: label(first), exact: true }).click();
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	let started = false;
	await page.route('**/api/behavior/items?**', async (route) => {
		started = true;
		await held;
		await route.fallback();
	});
	await page.getByRole('button', { name: 'โหลดเกณฑ์และรายชื่อใหม่', exact: true }).click();
	await expect.poll(() => started).toBe(true);
	await page.getByRole('button', { name: 'เลือกวันที่', exact: true }).click();
	await page.getByRole('button', { name: label(second), exact: true }).click();
	release();
	await expect(
		page.getByRole('button', { name: 'โหลดเกณฑ์และรายชื่อใหม่', exact: true })
	).toBeEnabled();
	await page.getByRole('button', { name: 'บันทึกพฤติกรรม', exact: true }).click();
	await expect(
		page.getByText('โหลดเกณฑ์และรายชื่อสำหรับวันที่ใหม่ก่อนบันทึก', { exact: true })
	).toBeVisible();
	expect(api.batches).toHaveLength(0);
});

for (const userType of ['student', 'parent'] as const)
	test(`${userType} reads cumulative scores across terms without admin data`, async ({ page }) => {
		const api = await mockBehavior(page, {
			userType,
			permissions: [PERMISSIONS.BEHAVIOR_READ_OWN]
		});
		await page.goto(`/${userType}/behavior?academicTermId=${behaviorTerm}`);
		await expect(page.getByText('สะสมทั้งหมด', { exact: true })).toBeVisible();
		await expect.poll(() => api.summaryQueries.length).toBeGreaterThan(0);
		expect(
			api.summaryQueries.every((query) => !new URLSearchParams(query).has('academicTermId'))
		).toBe(true);
		expect(api.reads).not.toContain('/api/behavior/settings');
		expect(api.reads).not.toContain('/api/behavior/items');
		await expect(page.getByRole('button', { name: 'ยืนยันยกเลิก', exact: true })).toHaveCount(0);
	});
