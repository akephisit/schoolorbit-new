import { revalidatePage } from './fixtures/page-revalidation';
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
	await expect(page.getByTestId('supervision-progress')).toHaveCount(0);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(1);
	expect(api.reads.some((value) => value.endsWith('/progress'))).toBe(false);
	api.release();
	await expect(
		page
			.getByTestId('supervision-cycle-selector')
			.getByRole('combobox', { name: 'รอบนิเทศ', exact: true })
	).toHaveText('รอบแรก');
});

test('school progress loads independently of cycle options and retains published counts', async ({
	page
}) => {
	const api = await mock(page, { hold: 'cycles', permissions: ['supervision.read.school'] });
	await page.goto(path('overview', year, firstCycle));
	const progress = page.getByTestId('supervision-progress');
	const pendingAcknowledgement = progress.locator('dt', { hasText: /^รอครูรับทราบ$/ });
	const completed = progress.locator('dt', { hasText: /^เสร็จสิ้น$/ });
	await expect(
		pendingAcknowledgement.locator('xpath=following-sibling::*[1][self::dd]')
	).toHaveText('2');
	await expect(completed.locator('xpath=following-sibling::*[1][self::dd]')).toHaveText('3');
	await expect(progress).toContainText('4.25');
	await expect(page.getByTestId('supervision-cycle-selector').getByRole('status')).toBeVisible();
	await expect(page.getByText(/รับรองผล|อนุมัติผล|รับรอง\/อนุมัติ/)).toHaveCount(0);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/progress`)).toBe(1);
	expect(api.count('/api/supervision/observations')).toBe(0);
	api.release();
});

test('school progress retry leaves the cycle selector and teacher status intact', async ({
	page
}) => {
	const api = await mock(page, { fail: 'progress' });
	await page.goto(path('overview', year, firstCycle));
	const progress = page.getByTestId('supervision-progress');
	await expect(progress.getByText('region ไม่พร้อม')).toBeVisible();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	await progress.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(progress.getByText('ความคืบหน้ารวม')).toBeVisible();
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/progress`)).toBe(2);
	expect(api.count(`/api/supervision/reports/cycles/${firstCycle}/teacher-status`)).toBe(1);
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test('a full route navigation clears progress from the previous cycle while the next report loads', async ({
	page
}) => {
	const api = await mock(page, { hold: 'progress' });
	await page.goto(path('overview', year, firstCycle));
	const progress = page.getByTestId('supervision-progress');
	await expect(progress).toContainText('4.25');
	await navigate(page, path('overview', year, secondCycle));
	await expect(page.getByText('ครู รอบสอง')).toBeVisible();
	await expect(progress).toHaveAttribute('aria-busy', 'true');
	await expect(progress.getByRole('status')).toBeVisible();
	await expect(progress).not.toContainText('4.25');
	api.release();
	await expect(progress).toContainText('4.50');
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
	const progress = page.getByTestId('supervision-progress');
	await expect(progress).toContainText('4.25');
	await expect(
		page
			.getByTestId('supervision-cycle-selector')
			.getByRole('combobox', { name: 'รอบนิเทศ', exact: true })
	).toHaveText('รอบแรก');
	await page
		.getByTestId('supervision-cycle-selector')
		.getByRole('combobox', { name: 'รอบนิเทศ', exact: true })
		.click();
	await page.getByRole('option', { name: /รอบที่สอง/ }).click();
	await expect(page).toHaveURL(new RegExp(`cycleId=${secondCycle}`));
	await expect(page.getByText('ครู รอบแรก')).toHaveCount(0);
	await expect(progress).toContainText('4.50');
	await page.goBack();
	await expect(page.getByText('ครู รอบแรก')).toBeVisible();
	await expect(progress).toContainText('4.25');
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
		page.getByRole('dialog').getByRole('combobox', { name: 'แบบประเมิน', exact: true })
	).toHaveText('แบบประเมินทดสอบ');
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
	await page.getByRole('dialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	const cycle = page
		.getByTestId('supervision-cycles-ready')
		.getByRole('listitem')
		.filter({ hasText: 'รอบแรก' });
	await cycle.getByRole('button', { name: 'ปิดรับคำขอ' }).click();
	await expect(cycle.getByRole('button', { name: 'เปิดรับคำขอ' })).toBeVisible();
	expect(api.count('/api/supervision/cycles')).toBe(1);
});

test.describe('cycle management on a 320-pixel touch viewport', () => {
	test.use({ viewport: { width: 320, height: 800 }, hasTouch: true });

	test('the cycle list fits the page and its actions are reachable from the hub', async ({
		page
	}) => {
		const api = await mock(page);
		await page.goto(path(''));
		await page
			.getByTestId('supervision-hub')
			.getByRole('link', { name: 'รอบนิเทศ', exact: true })
			.tap();
		await expect(page).toHaveURL(path('cycles'));
		const navigation = page.getByTestId('supervision-workflow-nav');
		await expect(navigation.getByRole('link')).toHaveCount(1);
		await expect(
			navigation.getByRole('link', { name: 'หน้าหลักนิเทศการสอน', exact: true })
		).toHaveAttribute('href', path(''));
		const list = page
			.getByTestId('supervision-cycles-ready')
			.getByRole('list', { name: 'รายการรอบนิเทศ', exact: true });
		const cycle = list.getByRole('listitem').filter({ hasText: 'รอบแรก' });
		await expect(cycle.getByText('เปิดรับคำขอ', { exact: true })).toBeVisible();
		const date = cycle.getByRole('button', { name: 'กำหนดช่วงวันที่ รอบแรก', exact: true });
		await expect(date).toBeVisible();
		await expect(cycle.getByRole('button', { name: 'ปิดรับคำขอ', exact: true })).toBeVisible();
		await expect(
			cycle.getByRole('button', { name: 'ลบรอบ รอบแรก ถาวร', exact: true })
		).toBeVisible();
		const layout = await cycle.evaluate((item) => {
			const ready = item.closest('[data-testid="supervision-cycles-ready"]')!;
			const list = item.closest('ul')!;
			const main = item.closest('main')!;
			const bounds = item.getBoundingClientRect();
			const content = main.getBoundingClientRect();
			const viewportWidth = document.documentElement.clientWidth;
			const left = Math.max(bounds.left, content.left, 0);
			const right = Math.min(bounds.right, content.right, viewportWidth);
			const actions = Array.from(item.querySelectorAll('button'), (button) =>
				button.getBoundingClientRect()
			);

			return {
				fits: [document.documentElement, main, ready, list, item].every(
					(element) => element.scrollWidth <= element.clientWidth + 1
				),
				actionsFit: actions.every(
					(rect) =>
						rect.left >= left && rect.right <= right && rect.width >= 24 && rect.height >= 24
				),
				actionsDoNotOverlap: actions.every((rect, index) =>
					actions
						.slice(index + 1)
						.every(
							(other) =>
								rect.right <= other.left ||
								rect.left >= other.right ||
								rect.bottom <= other.top ||
								rect.top >= other.bottom
						)
				)
			};
		});
		expect(layout).toEqual({ fits: true, actionsFit: true, actionsDoNotOverlap: true });
		await date.tap();
		const dialog = page.getByRole('dialog', { name: 'กำหนดช่วงวันที่นิเทศ' });
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).tap();
		await expect(dialog).toHaveCount(0);
		expect(api.writes.filter((request) => request.includes('/api/supervision/'))).toHaveLength(0);
	});
});

test('cycles retains data through a failed refresh and retries only its list', async ({ page }) => {
	const api = await mock(page, { fail: 'cycles', failAt: 2 });
	await page.goto(path('cycles'));
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	await revalidatePage(page);
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
	expect(api.payloads[0]).not.toHaveProperty('steps');
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
	const cycles = page.getByTestId('supervision-cycles-ready').getByRole('listitem');
	await cycles.filter({ hasText: 'รอบแรก' }).getByRole('button', { name: 'ปิดรับคำขอ' }).click();
	await navigate(page, path('cycles', nextYear));
	await expect(page.getByText('รอบปีใหม่', { exact: true })).toBeVisible();
	await expect(
		cycles.filter({ hasText: 'รอบปีใหม่' }).getByRole('button', { name: 'ปิดรับคำขอ' })
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
