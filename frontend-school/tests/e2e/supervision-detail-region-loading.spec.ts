import { revalidatePage } from './fixtures/page-revalidation';
import { expect, test, type Page } from '@playwright/test';
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

async function controlledAuthRefresh(
	page: Page,
	event: 'reconcile_required' | 'permission_changed' = 'reconcile_required'
) {
	const gate = Promise.withResolvers<void>();
	await page.route('**/api/notifications/stream', async (route) => {
		await gate.promise;
		await route.fulfill({
			status: 200,
			contentType: 'text/event-stream',
			body: `event: ${event}\ndata: {}\n\n`
		});
	});
	return async () => {
		const auth = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/auth/me'
		);
		gate.resolve();
		await (await auth).finished();
		await expect(page.getByRole('button', { name: 'เปิดเมนูบัญชี', exact: true })).toContainText(
			'ตท'
		);
	};
}

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
test('detail revalidation retains identity and recovers each dependent error locally', async ({
	page
}) => {
	const api = await mock(page, { fail: 'observation-detail', failAt: 2 });
	const refreshSameIdentity = await controlledAuthRefresh(page);
	await page.goto(detailPath());
	const detail = page.getByTestId('supervision-detail');
	const cycle = page.getByTestId('supervision-detail-cycle');
	const template = page.getByTestId('supervision-detail-template');
	const teacher = page.getByRole('heading', { name: 'ครู ในคิว', exact: true });
	const identityError = detail
		.locator('[data-slot="card"]')
		.filter({ has: page.getByRole('heading', { name: 'อัปเดตรายการไม่สำเร็จ', exact: true }) });
	await expect(teacher).toBeVisible();
	await expect(cycle).toHaveAttribute('aria-busy', 'false');
	await expect(template).toHaveAttribute('aria-busy', 'false');
	expect(api.count(cyclePath)).toBe(1);
	expect(api.count(summaryPath)).toBe(1);
	await revalidatePage(page);
	await expect(identityError.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
	await expect(cycle.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
	await expect(template.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
	await expect(teacher).toBeVisible();
	expect(api.count(observationPath)).toBe(2);
	expect(api.count(cyclePath)).toBe(1);
	expect(api.count(summaryPath)).toBe(1);
	await detail
		.locator(':scope > button')
		.filter({ hasText: /^ลองใหม่$/ })
		.click();
	await expect.poll(() => api.count(observationPath)).toBe(3);
	await expect(detail).toHaveAttribute('aria-busy', 'false');
	await expect(identityError).toHaveCount(0);
	await expect(teacher).toBeVisible();
	await expect(cycle.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
	await expect(template.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
	expect(api.count(cyclePath)).toBe(1);
	expect(api.count(summaryPath)).toBe(1);
	await cycle.getByRole('button', { name: 'ลองใหม่', exact: true }).click();
	await expect.poll(() => api.count(cyclePath)).toBe(2);
	await expect(cycle).toHaveAttribute('aria-busy', 'false');
	await expect(cycle.getByText('region ไม่พร้อม', { exact: true })).toHaveCount(0);
	await expect(template.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
	expect(api.count(observationPath)).toBe(3);
	expect(api.count(summaryPath)).toBe(1);
	await template.getByRole('button', { name: 'ลองใหม่', exact: true }).click();
	await expect.poll(() => api.count(summaryPath)).toBe(2);
	await expect(template).toHaveAttribute('aria-busy', 'false');
	await expect(detail.getByText('region ไม่พร้อม', { exact: true })).toHaveCount(0);
	const authReads = api.count('/api/auth/me');
	api.setFirstName('ตรวจสอบ');
	await refreshSameIdentity();
	expect(api.count('/api/auth/me')).toBeGreaterThan(authReads);
	await expect(detail.getByText('region ไม่พร้อม', { exact: true })).toHaveCount(0);
	await expect(teacher).toBeVisible();
	expect(api.count(observationPath)).toBe(3);
	expect(api.count(cyclePath)).toBe(2);
	expect(api.count(summaryPath)).toBe(2);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
	expect(api.count(`${observationPath}/review`)).toBe(0);
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

test('a reopened evaluator no longer displays the historical submission time', async ({ page }) => {
	const evaluators = (
		[
			[20, 'ผู้ประเมินที่ต้องส่งใหม่', 'draft', '2026-10-01T09:00:00Z'],
			[22, 'ผู้ประเมินที่ส่งครบ', 'submitted', '2026-10-02T09:00:00Z']
		] as const
	).map(([userId, name, status, submittedAt], index) => ({
		id: id(31 + index),
		observationId: id(30),
		evaluatorUserId: id(userId),
		evaluatorDisplayName: name,
		isRequired: true,
		status,
		submittedAt,
		createdAt: '2026-10-01T00:00:00Z',
		updatedAt: '2026-10-02T09:00:00Z'
	}));
	await mock(page, { observationStatus: 'planned', observation: { evaluators } });
	await page.goto(detailPath());
	const evaluatorRows = page.getByTestId('supervision-detail').getByRole('listitem');
	const draft = evaluatorRows.filter({
		has: page.getByText('ผู้ประเมินที่ต้องส่งใหม่', { exact: true })
	});
	const submitted = evaluatorRows.filter({
		has: page.getByText('ผู้ประเมินที่ส่งครบ', { exact: true })
	});
	await expect(draft).toContainText('ยังไม่ส่ง');
	await expect(draft).not.toContainText('1 ต.ค. 2569');
	await expect(draft.locator('time')).toHaveCount(0);
	await expect(submitted).toContainText('ส่งผลแล้ว');
	await expect(submitted.locator('time')).toHaveAttribute('datetime', evaluators[1].submittedAt);
});

test('review is lazy and retries without reloading identity', async ({ page }) => {
	const api = await mock(page, { fail: 'review', observationStatus: 'published' });
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
	const api = await mock(page, { hold: 'review', observationStatus: 'published' });
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
				name: workflow === 'evaluator' ? 'ตรวจคำขอ' : 'แก้คาบ/วันเวลา',
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
	await revalidatePage(page);
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

test('unit manager reading another unit cannot manage or review its unreleased observation', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: ['supervision.read.school', 'supervision.manage.organization_unit'],
		observationStatus: 'in_progress',
		canManage: false,
		canReview: false,
		averageRating: 4.875
	});
	await page.goto(detailPath());
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: /แก้ไข|ยกเลิก|ดูผลประเมินรายข้อ/ })).toHaveCount(0);
	await expect(page.getByTestId('supervision-detail')).not.toContainText('4.88');
	expect(api.count(`${observationPath}/review`)).toBe(0);
	expect(api.count(`${observationPath}/timetable-options`)).toBe(0);
	expect(api.count(`${observationPath}/evaluator-availability`)).toBe(0);
});

async function heldIdentityRetry(page: Page, event: 'reconcile_required' | 'permission_changed') {
	const api = await mock(page, {
		fail: 'observation-detail',
		failAt: 2,
		hold: 'observation-detail',
		holdAt: 3
	});
	const refresh = await controlledAuthRefresh(page, event);
	await page.goto(detailPath());
	const detail = page.getByTestId('supervision-detail');
	await expect(page.getByTestId('supervision-detail-cycle')).toHaveAttribute('aria-busy', 'false');
	await expect(page.getByTestId('supervision-detail-template')).toHaveAttribute(
		'aria-busy',
		'false'
	);
	await revalidatePage(page);
	await expect(
		detail.getByRole('heading', { name: 'อัปเดตรายการไม่สำเร็จ', exact: true })
	).toBeVisible();
	await detail
		.locator(':scope > button')
		.filter({ hasText: /^ลองใหม่$/ })
		.click();
	await expect.poll(() => api.count(observationPath)).toBe(3);
	await expect(detail).toHaveAttribute('aria-busy', 'true');
	return { api, refresh, detail };
}

test('same-identity authentication reconciliation preserves a held local retry', async ({
	page
}) => {
	const { api, refresh, detail } = await heldIdentityRetry(page, 'reconcile_required');
	const authReads = api.count('/api/auth/me');
	api.setFirstName('ตรวจสอบ');
	await refresh();
	expect(api.count('/api/auth/me')).toBeGreaterThan(authReads);
	await expect(detail).toHaveAttribute('aria-busy', 'true');
	await expect(
		detail.getByRole('heading', { name: 'อัปเดตรายการไม่สำเร็จ', exact: true })
	).toHaveCount(0);
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toBeVisible();
	api.release();
	await expect(detail).toHaveAttribute('aria-busy', 'false');
	await expect(
		detail.getByRole('heading', { name: 'อัปเดตรายการไม่สำเร็จ', exact: true })
	).toHaveCount(0);
	expect(api.count(observationPath)).toBe(3);
	expect(api.count(cyclePath)).toBe(1);
	expect(api.count(summaryPath)).toBe(1);
});

test('changed read permissions clear identity data and reject a held response without denied reads', async ({
	page
}) => {
	const { api, refresh } = await heldIdentityRetry(page, 'permission_changed');
	api.setPermissions(['supervision.evaluate.assigned']);
	api.setFirstName('ตรวจสอบ');
	await refresh();
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toHaveCount(0);
	api.release();
	await navigate(page, detailPath(40));
	await expect(page.getByText('ไม่พบรายการนิเทศ', { exact: true })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'ครู ในคิว', exact: true })).toHaveCount(0);
	await expect(page.getByText('วิชาทดสอบ', { exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(3);
	expect(api.count(`/api/supervision/observations/${id(40)}`)).toBe(0);
	expect(api.count(cyclePath)).toBe(1);
	expect(api.count(summaryPath)).toBe(1);
});
