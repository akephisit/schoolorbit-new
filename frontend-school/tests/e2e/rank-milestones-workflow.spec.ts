import { expect, test, type Page } from '@playwright/test';
import { mockStaffDirectory, staffPath, firstStaff } from './fixtures/staff-directory-route-data';
import { rankMilestone, rankOverview } from './fixtures/staff-career-route-data';
import type { components } from '../../src/lib/api/generated/school-api';
test.use({ serviceWorkers: 'block' });
const region = (page: Page) =>
	page.getByRole('region', { name: 'ภาพรวมกำหนดเวลาวิทยฐานะ', exact: true });
const ordinary = () =>
	rankMilestone({
		status: 'due_soon',
		reasons: [],
		reasonLabels: [],
		ordinaryDate: '2026-12-01',
		conditionalReducedDate: '2025-12-01',
		recordedStartDate: '2022-12-01',
		daysUntilOrdinaryDate: 59
	});
async function setup(page: Page, options: { fail?: boolean; hold?: boolean } = {}) {
	await mockStaffDirectory(page);
	await page.route('**/api/staff/personnel-overview**', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: {
					asOf: '2026-10-03T00:00:00Z',
					total: 3,
					active: 2,
					otherStatuses: 1,
					filteredTotal: 2,
					statuses: [{ key: 'active', label: 'ปฏิบัติงาน', count: 2 }],
					subjectGroups: [],
					jobPositions: [],
					academicRanks: [],
					educationLevels: []
				}
			}
		})
	);
	let reads = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route('**/api/staff/personnel-rank-milestones**', async (route) => {
		reads++;
		const query = new URL(route.request().url()).searchParams;
		if (options.hold && reads === 1) await held;
		if (options.fail && reads === 1)
			return route.fulfill({
				status: 503,
				json: { success: false, error: 'synthetic milestone failure' }
			});
		const bucket = (query.get('bucket') ??
			'due_soon') as components['schemas']['RankMilestoneStatus'];
		const milestone = bucket === 'incomplete' ? rankMilestone() : ordinary();
		const pageNumber = Number(query.get('page') ?? 1);
		const data = rankOverview({
			bucket,
			counts: {
				dueSoon: 51,
				future: 1,
				timeReachedPendingReview: 2,
				incomplete: 1,
				unsupported: 1,
				noNextRank: 1
			},
			filteredTotal: 57,
			total: bucket === 'due_soon' ? 51 : 1,
			page: pageNumber,
			items: [
				{
					staffId: firstStaff,
					displayName:
						query.get('status') === 'inactive'
							? 'บุคลากรสถานะอื่น'
							: pageNumber === 2
								? 'บุคลากรหน้าถัดไป'
								: 'ครูทดสอบกำหนดเวลา',
					milestone
				}
			]
		});
		await route.fulfill({ json: { success: true, data } });
	});
	return { release, reads: () => reads };
}
test('dashboard milestone cards drill into missing dates and paginate chronological planning rows', async ({
	page
}) => {
	await setup(page);
	await page.goto('/staff/manage/overview');
	await expect(region(page).getByText('ครูทดสอบกำหนดเวลา', { exact: true })).toBeVisible();
	await expect(region(page)).toContainText('2569');
	await expect(
		region(page).getByRole('link', { name: 'ดูประวัติ ครูทดสอบกำหนดเวลา' })
	).toHaveAttribute('href', new RegExp(firstStaff));
	await region(page).getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(region(page).getByText('บุคลากรหน้าถัดไป', { exact: true })).toBeVisible();
	await region(page)
		.getByRole('button', { name: /ข้อมูลยังไม่ครบ/ })
		.click();
	await expect(region(page)).toContainText('ยังไม่ระบุวันที่มีผลของตำแหน่งครู');
	await expect(region(page).getByRole('button', { name: 'ถัดไป', exact: true })).toHaveCount(0);
	await expect(region(page)).not.toContainText('คุณสมบัติครบสำหรับยื่น');
});
test('milestone failure retries independently while existing charts remain usable', async ({
	page
}) => {
	await setup(page, { fail: true });
	await page.goto('/staff/manage/overview');
	await expect(
		page.getByRole('region', { name: 'สถานะบุคลากรทั้งหมด', exact: true })
	).toBeVisible();
	await expect(region(page)).toContainText('โหลดกำหนดเวลาวิทยฐานะไม่สำเร็จ');
	await region(page).getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(region(page).getByText('ครูทดสอบกำหนดเวลา', { exact: true })).toBeVisible();
	await expect(
		region(page).getByRole('heading', { name: 'โหลดกำหนดเวลาวิทยฐานะไม่สำเร็จ' })
	).toHaveCount(0);
});
test('a delayed old milestone status cannot replace the newly selected status', async ({
	page
}) => {
	const api = await setup(page, { hold: true });
	await page.goto('/staff/manage/overview');
	await expect(
		region(page).getByRole('button', { name: 'รีเฟรชกำหนดเวลาวิทยฐานะ' })
	).toBeDisabled();
	await page.getByRole('button', { name: 'สถานะสำหรับกราฟ', exact: true }).click();
	await page.getByRole('option', { name: 'ปิดการใช้งาน', exact: true }).click();
	await expect(region(page).getByText('บุคลากรสถานะอื่น', { exact: true })).toBeVisible();
	api.release();
	await expect(region(page).getByText('ครูทดสอบกำหนดเวลา', { exact: true })).toHaveCount(0);
});
test('profile displays ordinary and conditional dates with source and review wording', async ({
	page
}, testInfo) => {
	await mockStaffDirectory(page);
	await page.route('**/career-history**', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: {
					items: [],
					nextCursor: null,
					current: { personnelType: null, jobPosition: null, academicRank: null },
					rankMilestone: ordinary()
				}
			}
		})
	);
	await page.goto(staffPath());
	const card = page.getByTestId('rank-milestone-card');
	await expect(card).toContainText('ครบระยะเวลาตามเกณฑ์ปกติ 4 ปี');
	await expect(card).toContainText('2569');
	await expect(card).toContainText('2568');
	await expect(card).toContainText('ต้องตรวจหลักฐานสิทธิลดระยะเวลา');
	await expect(card).toContainText('ผล PA');
	await card.getByText(/หลักเกณฑ์ที่ใช้อ้างอิง/).click();
	await expect(card.getByRole('link', { name: '1932/2567' })).toHaveAttribute(
		'href',
		/^https:\/\/otepc.go.th\//
	);
	for (const width of [390, 1440])
		for (const theme of ['light', 'dark']) {
			await page.setViewportSize({ width, height: 1000 });
			await page.evaluate(
				(mode) => document.documentElement.classList.toggle('dark', mode === 'dark'),
				theme
			);
			await card.scrollIntoViewIfNeeded();
			expect(await card.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBeTruthy();
			await card.screenshot({ path: testInfo.outputPath(`rank-profile-${width}-${theme}.png`) });
		}
});
test('milestone planning stays readable on mobile and desktop in both themes', async ({
	page
}, testInfo) => {
	await setup(page);
	await page.goto('/staff/manage/overview');
	await expect(region(page).getByText('ครูทดสอบกำหนดเวลา', { exact: true })).toBeVisible();
	for (const width of [390, 1440])
		for (const theme of ['light', 'dark']) {
			await page.setViewportSize({ width, height: 1000 });
			await page.evaluate(
				(mode) => document.documentElement.classList.toggle('dark', mode === 'dark'),
				theme
			);
			await region(page).scrollIntoViewIfNeeded();
			expect(
				await region(page).evaluate((el) => el.scrollWidth <= el.clientWidth + 1)
			).toBeTruthy();
			await expect(
				region(page).getByRole('link', { name: 'ดูประวัติ ครูทดสอบกำหนดเวลา' })
			).toBeVisible();
			await region(page).screenshot({
				path: testInfo.outputPath(`rank-milestones-${width}-${theme}.png`)
			});
		}
});
