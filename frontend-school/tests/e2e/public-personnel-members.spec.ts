import { expect, test } from '@playwright/test';
import { startPublicSchoolServer } from './fixtures/public-school-server';
import type { PublicSchoolOrganization } from '../../src/lib/api/school.js';

let fixture: Awaited<ReturnType<typeof startPublicSchoolServer>>;
const person = (personKey: string, positionCode = 'head') => ({
	personKey,
	name: personKey,
	positionCode,
	positionTitle: null,
	avatarUrl: null
});
const organization: PublicSchoolOrganization = {
	units: [
		{
			id: 'admin',
			parentId: null,
			name: 'กลุ่มบริหารทดสอบ',
			unitType: 'management_group',
			learningAreaId: null,
			members: [person('สมาชิกหลายงาน', 'member')]
		},
		{
			id: 'work',
			parentId: 'admin',
			name: 'งานต้นทาง',
			unitType: 'division',
			learningAreaId: null,
			members: [person('สมาชิกหลายงาน')]
		},
		{
			id: 'deep',
			parentId: 'work',
			name: 'งานชั้นลึก',
			unitType: 'division',
			learningAreaId: null,
			members: [person('สมาชิกหลายงาน'), person('สมาชิกชั้นลึก')]
		},
		...Array.from({ length: 11 }, (_, index) => ({
			id: `area-${index}`,
			parentId: null,
			name: `สาระกำหนดเอง ${index + 1}`,
			unitType: 'subject_group',
			learningAreaId: `registry-${index}`,
			members: []
		})),
		{
			id: 'activity',
			parentId: 'admin',
			name: 'กิจกรรมพัฒนาผู้เรียน',
			unitType: 'division',
			learningAreaId: 'activity-registry',
			members: [person('หัวหน้ากิจกรรม')]
		}
	]
};
test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
// Match the existing public-school fixture's cold local server budget.
test.beforeEach(() => test.setTimeout(60_000));
test.beforeAll(async () => {
	const previewUrl = process.env.E2E_PUBLIC_PREVIEW_URL;
	fixture = await startPublicSchoolServer(
		organization,
		previewUrl
			? { url: previewUrl, apiPort: Number(process.env.E2E_PUBLIC_API_PORT ?? 3000) }
			: undefined
	);
});
test.afterAll(async () => {
	await fixture?.close();
});

for (const width of [390, 1440])
	test(`public roster inherits every depth with one portrait per person and source names ${width}`, async ({
		page
	}, info) => {
		await page.setViewportSize({ width, height: 900 });
		await page.route('https://fonts.**', (route) => route.abort());
		await page.goto(`${fixture.baseUrl}/management-groups`);
		const parentPortraits = page.locator('#unit-admin > ul').getByTestId('public-person');
		await expect(parentPortraits).toHaveCount(3);
		const sharedPerson = parentPortraits.filter({
			has: page.getByText('สมาชิกหลายงาน', { exact: true })
		});
		await expect(sharedPerson).toHaveCount(1);
		await expect(sharedPerson).toContainText('งานต้นทาง');
		await expect(sharedPerson).toContainText('งานชั้นลึก');
		await expect(parentPortraits.getByText('หัวหน้ากิจกรรม', { exact: true })).toBeVisible();
		await expect(
			page
				.getByRole('list', { name: 'สมาชิก กลุ่มบริหารทดสอบ', exact: true })
				.getByTestId('public-person')
		).toHaveCount(2);
		await expect(
			page
				.getByRole('list', { name: 'ผู้รับผิดชอบ กลุ่มบริหารทดสอบ', exact: true })
				.getByTestId('public-person')
		).toHaveCount(1);
		await page.goto(`${fixture.baseUrl}/subject-groups`);
		const navigation = page.getByRole('navigation', { name: 'เลือกกลุ่มสาระ', exact: true });
		await expect(navigation.getByRole('link')).toHaveCount(12);
		await expect(navigation).toContainText('สาระกำหนดเอง 11');
		await expect(navigation).toContainText('กิจกรรมพัฒนาผู้เรียน');
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await page.screenshot({ path: info.outputPath('public-learning-areas.png'), fullPage: true });
	});
