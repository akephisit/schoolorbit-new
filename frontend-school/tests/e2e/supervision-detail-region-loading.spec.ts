import { expect, test } from '@playwright/test';
import {
	id,
	year,
	firstCycle,
	templateId,
	mock,
	navigate
} from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const detailPath = (n = 30) => `/staff/academic/supervision/${id(n)}?academicYearId=${year}`;
const observationPath = `/api/supervision/observations/${id(30)}`;
const cyclePath = `/api/supervision/cycles/${firstCycle}`;
const summaryPath = `/api/supervision/templates/${templateId}/summary`;

for (const hold of ['selected-cycle', 'summary'] as const) {
	test(`detail identity renders before ${hold}`, async ({ page }) => {
		const api = await mock(page, { hold });
		await page.goto(detailPath());
		await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
		await expect(
			page
				.getByTestId(
					hold === 'summary' ? 'supervision-detail-template' : 'supervision-detail-cycle'
				)
				.getByRole('status')
		).toBeVisible();
		expect(api.count('/api/supervision/cycles')).toBe(0);
		expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
		expect(api.count(`${observationPath}/review`)).toBe(0);
		api.release();
		await expect(page.getByTestId('supervision-detail-cycle').getByRole('status')).toHaveCount(0);
	});
}
test('selected references retry independently', async ({ page }) => {
	const api = await mock(page, { fail: 'summary' });
	await page.goto(detailPath());
	const region = page.getByTestId('supervision-detail-template');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count(summaryPath)).toBe(2);
	expect(api.count(cyclePath)).toBe(1);
	expect(api.count(observationPath)).toBe(1);
});
test('identity retry restarts only references that failed its dependency', async ({ page }) => {
	const api = await mock(page, { fail: 'observation-detail' });
	await page.goto(detailPath());
	await page.getByTestId('supervision-detail').getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
	await expect.poll(() => api.count(cyclePath)).toBe(1);
	expect(api.count(summaryPath)).toBe(1);
});
test('detail refresh preserves content when identity fails', async ({ page }) => {
	const api = await mock(page, { fail: 'observation-detail', failAt: 2 });
	await page.goto(detailPath());
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
	await page.getByTestId('supervision-detail').getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count(observationPath)).toBe(3);
});
test('a delayed first ID cannot replace the next selected observation', async ({ page }) => {
	const api = await mock(page, { hold: 'observation-detail' });
	await page.goto(detailPath());
	await expect(page.getByTestId('supervision-detail').getByRole('status')).toBeVisible();
	await navigate(page, detailPath(40));
	await expect(page.getByRole('heading', { name: 'ครู รายการสอง', exact: true })).toBeVisible();
	api.release();
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toHaveCount(0);
});
test('review is lazy and retries without reloading identity', async ({ page }) => {
	const api = await mock(page, { fail: 'review', observationStatus: 'evaluators_submitted' });
	await page.goto(detailPath());
	await page.getByRole('button', { name: 'ดูผลประเมินรายข้อ' }).click();
	const region = page.getByTestId('supervision-review');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'โหลดผลอีกครั้ง' }).click();
	await expect(region.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด', { exact: true })).toBeVisible();
	expect(api.count(`${observationPath}/review`)).toBe(2);
	expect(api.count(observationPath)).toBe(1);
});
test('closing pending review aborts it and keeps it closed', async ({ page }) => {
	const api = await mock(page, { hold: 'review', observationStatus: 'evaluators_submitted' });
	await page.goto(detailPath());
	await page.getByRole('button', { name: 'ดูผลประเมินรายข้อ' }).click();
	await expect.poll(() => api.count(`${observationPath}/review`)).toBe(1);
	await page.getByRole('button', { name: 'ปิดผลรายข้อ' }).click();
	api.release();
	await expect(page.getByRole('button', { name: 'ดูผลประเมินรายข้อ' })).toBeVisible();
	await expect(page.locator('[data-supervision-review-rubric]')).toHaveCount(0);
});
for (const workflow of ['timetable-detail', 'evaluator'] as const) {
	test(`${workflow} options open lazily and retry locally`, async ({ page }) => {
		const api = await mock(page, { fail: workflow });
		await page.goto(detailPath());
		const resource = `${observationPath}/${workflow === 'evaluator' ? 'evaluator-availability' : 'timetable-options'}`;
		await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
		expect(api.count(resource)).toBe(0);
		await page
			.getByRole('button', {
				name: workflow === 'evaluator' ? 'แก้ผู้ประเมิน' : 'แก้คาบ/วันเวลา',
				exact: true
			})
			.click();
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByText('region ไม่พร้อม')).toBeVisible();
		await dialog.getByRole('button', { name: 'ลองใหม่' }).click();
		await expect(dialog.getByText('region ไม่พร้อม')).toHaveCount(0);
		expect(api.count(resource)).toBe(2);
		expect(api.count(observationPath)).toBe(1);
	});
}
test('cancel patches identity and beats a stale refresh', async ({ page }) => {
	const api = await mock(page, { hold: 'observation-detail', holdAt: 2 });
	await page.goto(detailPath());
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช', exact: true }).click();
	await expect.poll(() => api.count(observationPath)).toBe(2);
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).first().click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยันยกเลิก' }).click();
	await expect(page.getByText('ยกเลิกรายการนิเทศแล้ว', { exact: true })).toBeVisible();
	api.release();
	await expect(page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(2);
});
test('late old-ID mutation cannot alter the next detail', async ({ page }) => {
	const api = await mock(page, { hold: 'patch' });
	await page.goto(detailPath());
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).first().click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยันยกเลิก' }).click();
	await expect.poll(() => api.writes.some((value) => value.includes('/cancel'))).toBe(true);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await navigate(page, detailPath(40));
	await expect(page.getByRole('heading', { name: 'ครู รายการสอง', exact: true })).toBeVisible();
	api.release();
	await expect(page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true })).toBeVisible();
});
test('write-only route access issues no observation or references', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.evaluate.assigned'] });
	await page.goto(detailPath());
	await expect(page.getByText('ไม่พบรายการนิเทศ')).toBeVisible();
	expect(api.count(observationPath)).toBe(0);
	expect(api.count(cyclePath)).toBe(0);
	expect(api.count(summaryPath)).toBe(0);
});
