import { expect, test } from '@playwright/test';
import {
	addDeliveryLink,
	deliveryFrames,
	mockDeliveryLoading,
	recordDeliveryFrames
} from './fixtures/delivery-loading-route-data';
import { navigationOverviewPath } from './fixtures/delivery-navigation-route-data';
import { expectSilentBackground, revalidatePage } from './fixtures/page-revalidation';

test.use({ serviceWorkers: 'block' });

for (const width of [390, 1440]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`homeroom pending matches its workspace at ${width}px in ${theme}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 1000 });
			await page.addInitScript(
				(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
				theme
			);
			await recordDeliveryFrames(page);
			const api = await mockDeliveryLoading(page, { manager: true });
			const gate = api.hold('homerooms');
			await page.goto(navigationOverviewPath());
			const region = page.getByRole('region', { name: 'มุมมองรายห้อง', exact: true });
			await expect(region).toHaveAttribute('aria-busy', 'true');
			await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*dark)/);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: testInfo.outputPath('pending.png'),
				animations: 'disabled',
				fullPage: true
			});
			try {
				const summary = page.getByRole('region', { name: 'เทียบกับหลักสูตรปัจจุบัน', exact: true });
				const summaryWidth = await summary
					.locator(':scope > div')
					.first()
					.evaluate((element) => element.getBoundingClientRect().width);
				const parentWidth = await summary.evaluate((element) => element.clientWidth);
				expect(summaryWidth).toBeGreaterThan(parentWidth * 0.85);
				const skeleton = page.getByTestId('delivery-homerooms-skeleton');
				await expect(skeleton).toBeVisible();
				const metrics = skeleton.locator('[data-delivery-metric]');
				const boxes = await metrics.evaluateAll((elements) =>
					elements.map((element) => {
						const { x, y, width, height } = element.getBoundingClientRect();
						return { x, y, width, height };
					})
				);
				await expect(summary.locator('[data-slot="card"]')).toHaveCount(0);
				expect(boxes).toHaveLength(4);
				if (width >= 1280) expect(new Set(boxes.map((box) => box.y)).size).toBe(1);
				else expect(new Set(boxes.map((box) => box.x)).size).toBe(1);
				expect((await deliveryFrames(page))[0]).toMatchObject({ ready: false, skeleton: true });
				gate.release();
				await expect(page.getByTestId('delivery-homerooms-ready')).toBeVisible();
				const readyBoxes = await page
					.getByRole('region', { name: 'สรุปความพร้อมรายห้อง' })
					.locator(':scope > div')
					.evaluateAll((elements) =>
						elements.map((element) => {
							const { x, width } = element.getBoundingClientRect();
							return { x, width };
						})
					);
				for (const [index, box] of readyBoxes.entries()) {
					expect(Math.abs(box.x - boxes[index].x)).toBeLessThan(2);
					expect(Math.abs(box.width - boxes[index].width)).toBeLessThan(2);
				}
				await page.screenshot({
					path: testInfo.outputPath('ready.png'),
					animations: 'disabled',
					fullPage: true
				});
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				expect(api.writes).toEqual([]);
			} finally {
				gate.release();
			}
		});
	}
}

test('resolved hover preload renders ready on its first browser frame', async ({ page }) => {
	await recordDeliveryFrames(page);
	const api = await mockDeliveryLoading(page);
	await page.goto('/staff');
	await expect(page.getByRole('heading', { name: 'หน้าหลักของฉัน', exact: true })).toBeVisible();
	const link = await addDeliveryLink(page, navigationOverviewPath(), 'hover');
	const response = page.waitForResponse((response) =>
		response.url().includes('/delivery/homerooms')
	);
	await link.hover();
	await response;
	await expect.poll(() => api.counts.homerooms).toBe(1);
	await link.click();
	await expect(page.getByTestId('delivery-homerooms-ready')).toBeVisible();
	await expect.poll(async () => (await deliveryFrames(page)).length).toBeGreaterThan(0);
	expect((await deliveryFrames(page))[0]).toMatchObject({ ready: true, skeleton: false });
	expect(api.counts.homerooms).toBe(1);
});

test('same-context refresh retains usable content silently', async ({ page }) => {
	const api = await mockDeliveryLoading(page);
	await page.goto(navigationOverviewPath());
	const region = page.getByTestId('delivery-homerooms-ready');
	await expect(region).toBeVisible();
	const gate = api.hold('homerooms');
	try {
		await revalidatePage(page);
		await expect.poll(() => api.counts.homerooms).toBe(2);
		await expectSilentBackground(page, region);
		await expect(region.getByRole('heading', { name: 'ม.1/1', exact: true })).toBeVisible();
		await expect(page.locator('main [data-slot="skeleton"]')).toHaveCount(0);
		await expect(page.getByTestId('delivery-alignment-summary')).toBeVisible();
	} finally {
		gate.release();
	}
	await expect(region).toHaveAttribute('aria-busy', 'false');
	expect(api.writes).toEqual([]);
});
