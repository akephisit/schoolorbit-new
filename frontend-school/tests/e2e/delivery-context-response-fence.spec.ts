import { expect, test } from '@playwright/test';
import {
	mockDeliveryNavigation,
	navigationIds as ids,
	navigationWorkspace,
	navigationDelivery
} from './fixtures/delivery-navigation-route-data';
import { deferred, deliveryReply } from './fixtures/current-delivery';
test.use({ serviceWorkers: 'block' });

for (const resource of ['homerooms', 'workspace'] as const)
	for (const stage of ['initial', 'refresh'] as const)
		test(`${resource} ${stage} rejects a delayed foreign context and retries only its owning read`, async ({
			page
		}) => {
			const fixture = await mockDeliveryNavigation(page, { manager: true }),
				held = deferred();
			let reads = 0,
				bad = true;
			const expected = resource === 'homerooms' ? navigationWorkspace() : navigationDelivery();
			const foreign = structuredClone(expected);
			if (stage === 'initial') foreign.academicTermId = ids.activeTerm;
			else foreign.academicYearId = ids.activeYear;
			if ('offerings' in foreign)
				for (const offering of foreign.offerings) offering.name = 'ข้อมูลต่างบริบทที่ห้ามนำมาใช้';
			else for (const room of foreign.homerooms) room.homeroom.name = 'ห้องต่างบริบทที่ห้ามนำมาใช้';
			await page.route(`**/api/academic/delivery/${resource}?*`, async (route) => {
				const url = new URL(route.request().url());
				expect(url.searchParams.get('academicYearId')).toBe(ids.year);
				expect(url.searchParams.get('academicTermId')).toBe(ids.term);
				reads++;
				const reject = bad && (stage === 'initial' ? reads === 1 : reads === 2);
				if (reject) {
					await held.promise;
					return deliveryReply(route, foreign);
				}
				return deliveryReply(route, expected);
			});
			const href = `/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&view=offerings`,
				region = page.getByRole('region', {
					name:
						resource === 'homerooms' ? 'ตามหลักสูตรที่ยังไม่เปิดสอน' : 'มุมมองรายวิชาและกิจกรรม',
					exact: true
				});
			try {
				await page.goto(href);
				if (stage === 'refresh') {
					await expect(page.getByTestId('delivery-offerings-ready')).toBeVisible();
					await expect(page.getByTestId('delivery-alignment-summary')).toBeVisible();
					await page.evaluate(() => window.dispatchEvent(new Event('focus')));
					await expect.poll(() => reads).toBe(2);
				} else await expect.poll(() => reads).toBe(1);
				held.release();
				await expect(region).toContainText('ข้อมูลไม่ตรงกับปีการศึกษาและภาคเรียนที่เลือก');
				await expect(page.getByText('ข้อมูลต่างบริบทที่ห้ามนำมาใช้', { exact: true })).toHaveCount(
					0
				);
				await expect(page.getByText('ห้องต่างบริบทที่ห้ามนำมาใช้', { exact: true })).toHaveCount(0);
				await expect(
					region.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
				).toHaveCount(0);
				expect(fixture.creations).toEqual([]);
				bad = false;
				const other =
						resource === 'homerooms'
							? '/api/academic/delivery/workspace'
							: '/api/academic/delivery/homerooms',
					otherReads = fixture.count(other);
				await region
					.getByRole('button', {
						name: resource === 'homerooms' ? 'ลองโหลดข้อกำหนดอีกครั้ง' : 'ลองอีกครั้ง',
						exact: true
					})
					.click();
				await expect(region).not.toContainText('ข้อมูลไม่ตรงกับปีการศึกษาและภาคเรียนที่เลือก');
				expect(reads).toBe(stage === 'initial' ? 2 : 3);
				expect(fixture.count(other)).toBe(otherReads);
				expect(fixture.writes).toEqual([]);
			} finally {
				held.release();
			}
		});
