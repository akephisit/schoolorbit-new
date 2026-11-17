import { expect, test, type Route } from '@playwright/test';
import { mock, path, firstCycle } from './fixtures/supervision-route-data';
import type {
	SupervisionCycleDeletionImpact,
	DeleteSupervisionCycleRequest
} from '../../src/lib/api/supervision';

test.use({ serviceWorkers: 'block' });
const cyclePath = `/api/supervision/cycles/${firstCycle}`;
const previewPath = `${cyclePath}/deletion-impact`;
const impact: SupervisionCycleDeletionImpact = {
	cycleId: firstCycle,
	revision: 'revision-1',
	counts: { targets: 1, observations: 2, periods: 4, evaluators: 3, responses: 9, actions: 6 }
};
const reply = (route: Route, data: unknown, status = 200) =>
	route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});

test('permanent cycle deletion loads full impact on demand, requires exact name, and patches only the removed cycle', async ({
	page
}) => {
	const api = await mock(page);
	let previews = 0;
	const payloads: DeleteSupervisionCycleRequest[] = [];
	await page.route(`**${previewPath}`, (route) => {
		previews++;
		return reply(route, impact);
	});
	await page.route(`**${cyclePath}`, (route) => {
		if (route.request().method() !== 'DELETE') return route.fallback();
		payloads.push(route.request().postDataJSON());
		return reply(route, impact);
	});
	await page.goto(path('cycles'));
	await expect(page.getByText('รอบแรก', { exact: true })).toBeVisible();
	expect(previews).toBe(0);
	await page.getByRole('button', { name: 'ลบรอบ รอบแรก ถาวร', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await expect(dialog.getByTestId('supervision-delete-impact')).toContainText('คำตอบประเมิน');
	for (const [label, count] of [
		['เป้าหมาย', 1],
		['คำขอ/รายการนิเทศ', 2],
		['คาบนิเทศ', 4],
		['ผู้ประเมินที่มอบหมาย', 3],
		['คำตอบประเมิน', 9],
		['บันทึกการดำเนินการ', 6]
	] as const) {
		await expect(dialog.locator('dl > div').filter({ hasText: label }).locator('dd')).toHaveText(
			String(count)
		);
	}
	const confirm = dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true });
	await expect(confirm).toBeDisabled();
	await dialog.getByRole('textbox').fill('รอบแรก ');
	await expect(confirm).toBeDisabled();
	await dialog.getByRole('textbox').fill('รอบแรก');
	await confirm.click();
	await expect(dialog).toHaveCount(0);
	await expect(
		page.getByTestId('supervision-cycles-ready').getByText('รอบแรก', { exact: true })
	).toHaveCount(0);
	await expect(page.getByText('รอบที่สอง', { exact: true })).toBeVisible();
	expect(payloads).toEqual([{ expectedRevision: 'revision-1' }]);
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.reads.some((value) => value.includes('/templates'))).toBe(false);
});

test('stale deletion preview refuses without removing rows and requires a new impact snapshot', async ({
	page
}) => {
	await mock(page);
	let previews = 0;
	const payloads: DeleteSupervisionCycleRequest[] = [];
	await page.route(`**${previewPath}`, (route) =>
		reply(route, {
			...impact,
			revision: `revision-${++previews}`,
			counts: { ...impact.counts, observations: previews + 1 }
		})
	);
	await page.route(`**${cyclePath}`, (route) => {
		if (route.request().method() !== 'DELETE') return route.fallback();
		payloads.push(route.request().postDataJSON());
		return payloads.length === 1
			? reply(route, 'ข้อมูลรอบเปลี่ยนแล้ว กรุณาตรวจผลกระทบใหม่', 409)
			: reply(route, { ...impact, revision: 'revision-2' });
	});
	await page.goto(path('cycles'));
	await page.getByRole('button', { name: 'ลบรอบ รอบแรก ถาวร', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await dialog.getByRole('textbox').fill('รอบแรก');
	await dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true }).click();
	await expect(
		dialog.getByText('ข้อมูลรอบเปลี่ยนแล้ว กรุณาตรวจผลกระทบใหม่', { exact: true })
	).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true })).toBeDisabled();
	await expect(
		page.getByTestId('supervision-cycles-ready').getByText('รอบแรก', { exact: true })
	).toBeVisible();
	await dialog.getByRole('button', { name: 'ตรวจผลกระทบอีกครั้ง', exact: true }).click();
	await expect(
		dialog.locator('dl > div').filter({ hasText: 'คำขอ/รายการนิเทศ' }).locator('dd')
	).toHaveText('3');
	await dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(payloads).toEqual([
		{ expectedRevision: 'revision-1' },
		{ expectedRevision: 'revision-2' }
	]);
});

for (const status of [403, 404, 503]) {
	test(`failed impact ${status} keeps permanent deletion disabled`, async ({ page }) => {
		const api = await mock(page);
		await page.route(`**${previewPath}`, (route) => reply(route, 'ยังตรวจผลกระทบไม่ได้', status));
		await page.goto(path('cycles'));
		await page.getByRole('button', { name: 'ลบรอบ รอบแรก ถาวร', exact: true }).click();
		const dialog = page.getByRole('alertdialog');
		await expect(dialog.getByText('ยังตรวจผลกระทบไม่ได้', { exact: true })).toBeVisible();
		await expect(dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true })).toBeDisabled();
		expect(api.writes).toEqual([]);
	});
}

test('organization-only managers cannot enter school-wide cycle deletion or preload impact', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: ['supervision.manage.organization_unit', 'supervision.read.organization_unit']
	});
	await page.goto(path('cycles'));
	await expect(page).toHaveURL(/\/403/);
	await expect(page.getByRole('button', { name: /ลบถาวร/ })).toHaveCount(0);
	expect(api.reads.filter((value) => value.startsWith('/api/supervision/'))).toHaveLength(0);
	expect(api.writes).toEqual([]);
});

test('closing a pending preview discards its late result', async ({ page }) => {
	await mock(page);
	const gate = Promise.withResolvers<void>();
	await page.route(`**${previewPath}`, async (route) => {
		await gate.promise;
		await reply(route, impact);
	});
	await page.goto(path('cycles'));
	await page.getByRole('button', { name: 'ลบรอบ รอบแรก ถาวร', exact: true }).click();
	await page.getByRole('alertdialog').getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	gate.resolve();
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	await expect(
		page.getByTestId('supervision-cycles-ready').getByText('รอบแรก', { exact: true })
	).toBeVisible();
});

test('an accepted deletion keeps its confirmation open and cannot rebase while pending', async ({
	page
}) => {
	await mock(page);
	const gate = Promise.withResolvers<void>();
	let deleting = false;
	let previews = 0;
	await page.route(`**${previewPath}`, (route) => {
		previews++;
		return reply(route, impact);
	});
	await page.route(`**${cyclePath}`, async (route) => {
		if (route.request().method() !== 'DELETE') return route.fallback();
		deleting = true;
		await gate.promise;
		return reply(route, impact);
	});
	await page.goto(path('cycles'));
	await page.getByRole('button', { name: 'ลบรอบ รอบแรก ถาวร', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await dialog.getByRole('textbox').fill('รอบแรก');
	await dialog.getByRole('button', { name: 'ยืนยันลบถาวร', exact: true }).click();
	await expect.poll(() => deleting).toBe(true);
	await expect(dialog.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
	await page.keyboard.press('Escape');
	await expect(dialog).toBeVisible();
	expect(previews).toBe(1);
	gate.resolve();
	await expect(dialog).toHaveCount(0);
});
