import { expect, test, type Page } from '@playwright/test';
import {
	mockCurrentDelivery,
	replaceDeliverySession,
	deliveryReply,
	deferred
} from './fixtures/current-delivery';
import { navigationOfferingIds } from './fixtures/delivery-navigation-route-data';

test.use({ serviceWorkers: 'block' });
async function turn(page: Page) {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}
for (const replacement of ['identity', 'resource'] as const)
	test(`a delayed missing-identity retry cannot install after ${replacement} replacement`, async ({
		page
	}) => {
		const fixture = await mockCurrentDelivery(page),
			held = deferred();
		let reads = 0;
		const fresh = fixture.resource();
		const stale = structuredClone(fresh);
		stale.nameSnapshot = 'ชื่อเก่าที่มาช้าจากคำขอลองใหม่';
		await page.route(`**/api/academic/offerings/${fresh.id}`, async (route) => {
			if (route.request().method() !== 'GET') return route.fallback();
			reads++;
			if (reads === 1) return deliveryReply(route, 'โหลดรายการครั้งแรกไม่สำเร็จ', 503);
			if (reads === 2) {
				await held.promise;
				return deliveryReply(route, stale);
			}
			return deliveryReply(route, fresh);
		});
		try {
			await page.goto(fixture.detail);
			await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
			await expect.poll(() => reads).toBe(2);
			if (replacement === 'identity') {
				fresh.nameSnapshot = 'รายการที่อ่านโดยตัวตนใหม่';
				fixture.setActor(fixture.ids.teacherB);
				await replaceDeliverySession(page, fixture);
				await expect(
					page.getByRole('heading', { name: 'รายการที่อ่านโดยตัวตนใหม่', level: 1, exact: true })
				).toBeVisible();
				expect(reads).toBe(3);
			} else {
				const href = `/staff/academic/delivery/${navigationOfferingIds.club}?academicYearId=${fixture.ids.year}&academicTermId=${fixture.ids.term}`;
				await page.evaluate((destination) => {
					const link = document.createElement('a');
					link.href = destination;
					link.textContent = 'เปิดรายการที่สอง';
					link.id = 'read-recovery-next-owner';
					link.dataset.sveltekitPreloadData = 'off';
					link.style.cssText = 'position:fixed;top:150px;left:500px;z-index:9999;background:white';
					document.body.append(link);
				}, href);
				await page.locator('#read-recovery-next-owner').click();
				await expect(
					page.getByRole('heading', { name: 'ชุมนุม', level: 1, exact: true })
				).toBeVisible();
				await expect(page).toHaveURL(new RegExp(`/delivery/${navigationOfferingIds.club}`));
			}
			held.release();
			await turn(page);
			await expect(
				page.getByRole('heading', { name: stale.nameSnapshot, level: 1, exact: true })
			).toHaveCount(0);
			await expect(
				page.getByRole('heading', {
					name: replacement === 'identity' ? 'รายการที่อ่านโดยตัวตนใหม่' : 'ชุมนุม',
					level: 1,
					exact: true
				})
			).toBeVisible();
			expect(fixture.writes).toEqual([]);
		} finally {
			held.release();
		}
	});
