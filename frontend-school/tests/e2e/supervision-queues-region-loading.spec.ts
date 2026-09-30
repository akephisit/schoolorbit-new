import { expect, test } from '@playwright/test';
import {
	id,
	year,
	nextYear,
	templateId,
	firstCycle,
	path,
	mock,
	navigate
} from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-09-30T09:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const assigned = ['supervision.read.assigned', 'supervision.evaluate.assigned'];
const own = ['supervision.read.own', 'supervision.request.own'];
const observationPath = '/api/supervision/observations';

for (const hold of ['cycles', 'summaries'] as const) {
	test(`requests list renders independently of delayed ${hold}`, async ({ page }) => {
		const api = await mock(page, { hold });
		await page.goto(path('requests'));
		await expect(
			page.getByTestId('supervision-observations').getByText('ครู ในคิว', { exact: true }).first()
		).toBeVisible();
		await expect(page.getByTestId('supervision-references').getByRole('status')).toBeVisible();
		expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(0);
		expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
		api.release();
		await expect(page.getByTestId('supervision-references').getByRole('status')).toHaveCount(0);
	});
}

test('requests first list is a skeleton, with no false empty state', async ({ page }) => {
	const api = await mock(page, { hold: 'observations' });
	await page.goto(path('requests'));
	await expect(page.getByTestId('supervision-observations').getByRole('status')).toBeVisible();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toHaveCount(0);
	api.release();
	await expect(page.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
});

test('queue retry rereads only its failed observation list', async ({ page }) => {
	const api = await mock(page, { fail: 'observations' });
	await page.goto(path('requests'));
	const region = page.getByTestId('supervision-observations');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	expect(api.count(observationPath)).toBe(2);
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('queue refresh retains usable rows through an error', async ({ page }) => {
	const api = await mock(page, { fail: 'observations', failAt: 2 });
	await page.goto(path('requests'));
	const region = page.getByTestId('supervision-observations');
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count(observationPath)).toBe(3);
	expect(api.count('/api/supervision/cycles')).toBe(2);
});

test('a late old-year list cannot overwrite the next year', async ({ page }) => {
	const api = await mock(page, { hold: 'observations' });
	await page.goto(path('requests'));
	await expect(page.getByTestId('supervision-observations').getByRole('status')).toBeVisible();
	await navigate(page, path('requests', nextYear));
	await expect(page.getByText('ครู ปีใหม่', { exact: true }).first()).toBeVisible();
	api.release();
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
});

test('returning a request supersedes a pending stale refresh', async ({ page }) => {
	const api = await mock(page, { hold: 'observations', holdAt: 2 });
	await page.goto(path('requests'));
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'true');
	await page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true }).click();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toBeVisible();
	api.release();
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(2);
});

test('evaluator picker loads on open and retries only its options', async ({ page }) => {
	const api = await mock(page, { fail: 'evaluator' });
	await page.goto(path('requests'));
	await page.getByRole('combobox', { name: /เลือกผู้ประเมิน/ }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ผู้ประเมินที่ว่าง', { exact: true })).toBeVisible();
	expect(api.count(observationPath)).toBe(1);
	expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(2);
});

test('evaluation rubric is lazy and a typed submit patches only the observation', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'scheduled',
		fail: 'detail'
	});
	await page.goto(path('evaluate'));
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('region ไม่พร้อม')).toBeVisible();
	await dialog.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(dialog.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	await dialog.getByRole('button', { name: '3', exact: true }).click();
	await dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(1);
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('closing evaluation supersedes its pending rubric', async ({ page }) => {
	const api = await mock(page, { permissions: assigned, hold: 'detail' });
	await page.goto(path('evaluate'));
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	await page.keyboard.press('Escape');
	api.release();
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('write-only evaluation permission does not grant observation reads', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.evaluate.assigned'] });
	await page.goto(path('evaluate'));
	await expect(page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ')).toBeVisible();
	expect(api.count(observationPath)).toBe(0);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
});

test('mine loads its list first and keeps booking timetable behind opening the workflow', async ({
	page
}) => {
	const api = await mock(page, { permissions: own, term: true, fail: 'timetable' });
	await page.goto(path('').replace('/supervision/?', '/supervision?'));
	await expect(
		page
			.getByTestId('supervision-observations')
			.getByRole('heading', { name: 'วิชาทดสอบ', exact: true })
	).toBeVisible();
	expect(api.count('/api/me/timetable')).toBe(0);
	await page.getByRole('button', { name: 'เปิดจองคาบนิเทศ', exact: true }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ไม่พบคาบสอนในภาคเรียนนี้')).toBeVisible();
	expect(api.count('/api/me/timetable')).toBe(2);
	expect(api.count(observationPath)).toBe(1);
});

test('organization approvals avoid school report reads and options', async ({ page }) => {
	const api = await mock(page, {
		permissions: ['supervision.manage.organization_unit'],
		observationStatus: 'evaluators_submitted'
	});
	await page.goto(path('approvals'));
	await expect(
		page.getByTestId('supervision-observations').getByText('ครู ในคิว', { exact: true })
	).toBeVisible();
	await expect(page.getByRole('button', { name: 'โหลดรายงาน', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'false');
	expect(api.count('/api/supervision/cycles')).toBe(0);
	expect(api.reads.some((value) => value.endsWith('/progress'))).toBe(false);
});

test('school progress stays lazy and has a focused report retry', async ({ page }) => {
	const api = await mock(page, { observationStatus: 'evaluators_submitted', fail: 'progress' });
	await page.goto(path('approvals'));
	await expect(page.getByRole('button', { name: 'โหลดรายงาน', exact: true })).toBeVisible();
	expect(api.reads.some((value) => value.endsWith('/progress'))).toBe(false);
	await page.getByRole('button', { name: 'โหลดรายงาน', exact: true }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ความคืบหน้ารวม')).toBeVisible();
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/progress`)).toBe(2);
	expect(api.count(observationPath)).toBe(1);
});

test('denied queue route cannot preload an otherwise readable list', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.read.own'] });
	await page.goto(path('requests', year));
	await expect(page).toHaveURL(/\/403/);
	expect(api.count(observationPath)).toBe(0);
});
