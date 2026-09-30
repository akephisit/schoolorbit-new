import { expect, test } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

import {
	year,
	nextYear,
	firstCycle,
	secondCycle,
	templateId,
	path,
	mock,
	navigate
} from './fixtures/supervision-route-data';

test('explicit overview renders teacher status while cycle options are pending', async ({
	page
}) => {
	const api = await mock(page, {
		hold: 'cycles',
		permissions: ['supervision.manage.organization_unit']
	});
	await page.goto(path('overview', year, firstCycle));
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	await expect(page.getByRole('link', { name: 'คำขอจอง', exact: true })).toHaveAttribute(
		'href',
		path('requests')
	);
	await expect(page.getByTestId('supervision-cycle-selector').getByRole('status')).toBeVisible();
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(1);
	expect(api.reads.some((value) => value.endsWith('/progress'))).toBe(false);
	api.release();
	await expect(
		page
			.getByTestId('supervision-cycle-selector')
			.getByRole('button', { name: 'รอบแรก', exact: true })
	).toBeVisible();
});

test('default overview waits only for cycles to select the first ID', async ({ page }) => {
	const api = await mock(page, { hold: 'cycles' });
	await page.goto(path('overview'));
	await expect(page.getByTestId('supervision-teacher-status').getByRole('status')).toBeVisible();
	expect(api.reads.filter((value) => value.endsWith('/teacher-status'))).toHaveLength(0);
	api.release();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('overview retries status without rereading cycles', async ({ page }) => {
	const api = await mock(page, {
		fail: 'teacher-status',
		permissions: ['supervision.read.school']
	});
	await page.goto(path('overview', year, firstCycle));
	const region = page.getByTestId('supervision-teacher-status');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(2);
});

test('overview cycle selection and Back supersede pending status without broad reads', async ({
	page
}) => {
	const api = await mock(page, { hold: 'teacher-status' });
	await page.goto(path('overview', year, firstCycle));
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	await page
		.getByTestId('supervision-cycle-selector')
		.getByRole('button', { name: 'รอบแรก', exact: true })
		.click();
	await page.getByRole('option', { name: /รอบที่สอง/ }).click();
	await expect(page).toHaveURL(new RegExp(`cycleId=${secondCycle}`));
	await expect(page.getByText('ครู รอบแรก')).toHaveCount(0);
	await page.goBack();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	api.release();
	await expect(page.getByText('ครู รอบสอง')).toHaveCount(0);
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('cycles starts with its own skeleton, keeps template choices lazy, and patches status', async ({
	page
}) => {
	const api = await mock(page, { hold: 'cycles' });
	await page.goto(path('cycles'));
	await expect(page.getByTestId('supervision-cycles').getByRole('status')).toBeVisible();
	await expect(page.getByText('ยังไม่มีรอบนิเทศ')).toHaveCount(0);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
	api.release();
	await expect(page.getByTestId('supervision-cycles-ready')).toBeVisible();
	await page.getByRole('button', { name: 'สร้างรอบนิเทศ', exact: true }).first().click();
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'แบบประเมินทดสอบ', exact: true })
	).toBeVisible();
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
	await page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page
		.getByRole('row', { name: /รอบแรก/ })
		.getByRole('button', { name: 'ปิดรอบ' })
		.click();
	await expect(
		page.getByRole('row', { name: /รอบแรก/ }).getByRole('button', { name: 'เปิดอีกครั้ง' })
	).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('cycles retains data through a failed refresh and retries only its list', async ({ page }) => {
	const api = await mock(page, { fail: 'cycles', failAt: 2 });
	await page.goto(path('cycles'));
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByTestId('supervision-cycles').getByText('region ไม่พร้อม')).toBeVisible();
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	await page.getByTestId('supervision-cycles').getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count('/api/supervision/cycles')).toBe(3);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
});

test('cycles ignores a late old-year list', async ({ page }) => {
	const api = await mock(page, { hold: 'cycles' });
	await page.goto(path('cycles'));
	await expect(page.getByTestId('supervision-cycles').getByRole('status')).toBeVisible();
	await navigate(page, path('cycles', nextYear));
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('รอบแรก', { exact: true })).toHaveCount(0);
});

test('templates loads summaries, then retrieves the selected rubric only on preview', async ({
	page
}) => {
	const api = await mock(page, { hold: 'detail' });
	await page.goto(path('templates'));
	await expect(page.getByTestId('supervision-templates-ready')).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	await page.getByRole('button', { name: 'ดูตัวอย่าง', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	api.release();
	await expect(page.getByRole('dialog').getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(1);
});

test('template preview has a focused retry', async ({ page }) => {
	const api = await mock(page, { fail: 'detail' });
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'ดูตัวอย่าง', exact: true }).click();
	await expect(page.getByRole('dialog').getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByRole('dialog').getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('template edit patches the returned summary without rereading any list', async ({ page }) => {
	const api = await mock(page);
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByPlaceholder('ชื่อแบบประเมิน')).toHaveValue('แบบประเมินทดสอบ');
	await dialog.getByPlaceholder('ชื่อแบบประเมิน').fill('แบบประเมินที่แก้แล้ว');
	await dialog.getByRole('button', { name: 'บันทึกแบบประเมิน', exact: true }).click();
	await expect(
		page.getByTestId('supervision-templates-ready').getByText('แบบประเมินที่แก้แล้ว')
	).toBeVisible();
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
	expect(api.count('/api/supervision/cycles')).toBe(0);
});

test('denied management access does not preload a protected collection', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.request.own'] });
	await page.goto(path('cycles'));
	await expect(page).toHaveURL(/\/403/);
	expect(api.reads.filter((value) => value.startsWith('/api/supervision/'))).toHaveLength(0);
});

test('a closed template editor stays closed after its delayed detail arrives', async ({ page }) => {
	const api = await mock(page, { hold: 'detail' });
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	api.release();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'สร้างแบบประเมิน', exact: true }).first().click();
	await expect(page.getByRole('dialog').getByPlaceholder('ชื่อแบบประเมิน')).toHaveValue(
		'แบบนิเทศการจัดการเรียนรู้'
	);
});

test('default overview retry recovers the failed cycle dependency before status', async ({
	page
}) => {
	const api = await mock(page, { fail: 'cycles' });
	await page.goto(path('overview'));
	const region = page.getByTestId('supervision-teacher-status');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('ครู รอบแรก')).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(2);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(1);
});

test('a pending cycle mutation does not block or patch the next academic year', async ({
	page
}) => {
	const api = await mock(page, { hold: 'patch' });
	await page.goto(path('cycles'));
	await page
		.getByRole('row', { name: /รอบแรก/ })
		.getByRole('button', { name: 'ปิดรอบ' })
		.click();
	await navigate(page, path('cycles', nextYear));
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('row', { name: /รอบปีใหม่/ }).getByRole('button', { name: 'ปิดรอบ' })
	).toBeEnabled();
	api.release();
	await expect(page.getByText('รอบแรก', { exact: true })).toHaveCount(0);
});

test('a delayed template save preserves a newly opened draft', async ({ page }) => {
	const api = await mock(page, { hold: 'patch' });
	await page.goto(path('templates'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('dialog').getByPlaceholder('ชื่อแบบประเมิน').fill('แบบประเมินที่บันทึก');
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'บันทึกแบบประเมิน', exact: true })
		.click();
	await expect
		.poll(() => api.writes.includes(`PATCH /api/supervision/templates/${templateId}`))
		.toBe(true);
	await page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('button', { name: 'สร้างแบบประเมิน', exact: true }).first().click();
	await page.getByRole('dialog').getByPlaceholder('ชื่อแบบประเมิน').fill('ร่างใหม่ที่ยังไม่บันทึก');
	api.release();
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'สร้างแบบประเมิน', exact: true })
	).toBeEnabled();
	await expect(page.getByRole('dialog').getByPlaceholder('ชื่อแบบประเมิน')).toHaveValue(
		'ร่างใหม่ที่ยังไม่บันทึก'
	);
});
