import { expect, test } from '@playwright/test';
import { publicSchoolStatistics, startPublicSchoolServer } from './fixtures/public-school-server';

let fixture: Awaited<ReturnType<typeof startPublicSchoolServer>>;
test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	fixture = await startPublicSchoolServer();
});
test.afterAll(async () => {
	await fixture?.close();
});
test.beforeEach(async ({ page }) => {
	test.setTimeout(60000);
	fixture.setStatistics(publicSchoolStatistics);
	fixture.setFailure(false);
	await page.route('https://fonts.**', (route) => route.abort());
});

for (const width of [390, 1440])
	for (const dark of [false, true]) {
		test(`grade disclosures preserve totals and other grades: ${width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 900 });
			await page.goto(`${fixture.baseUrl}/#statistics`);
			if (dark) await page.evaluate(() => document.documentElement.classList.add('dark'));
			const table = page.getByTestId('school-student-summary');
			const grade = table.getByRole('button', { name: 'มัธยมศึกษาปีที่ 1', exact: true });
			await expect(grade).toHaveAttribute('aria-expanded', 'false');
			await expect(table.getByRole('rowheader', { name: 'ม.1/1', exact: true })).toHaveCount(0);
			await expect(table.locator('tfoot td')).toHaveText(['10', '12', '1', '23']);
			await expect(
				table.getByRole('rowheader', { name: 'รวมมัธยมศึกษาตอนต้น', exact: true })
			).toBeVisible();
			await expect(
				table.getByRole('rowheader', { name: 'รวมมัธยมศึกษาตอนปลาย', exact: true })
			).toBeVisible();
			await expect(
				table.getByRole('button', { name: 'มัธยมศึกษาปีที่ 2', exact: true })
			).toHaveCount(0);
			await table.screenshot({
				path: `/tmp/public-statistics-${width}-${dark ? 'dark' : 'light'}-collapsed.png`,
				animations: 'disabled'
			});
			const readCount = fixture.requests.filter((url) =>
				url.pathname.endsWith('/statistics')
			).length;
			await grade.click();
			await expect(grade).toHaveAttribute('aria-expanded', 'true');
			const panelId = await grade.getAttribute('aria-controls');
			const panel = table
				.locator('tbody')
				.filter({ has: page.getByRole('rowheader', { name: 'ม.1/1', exact: true }) });
			await expect(panel).toHaveAttribute('id', panelId!);
			await expect(panel.getByRole('rowheader')).toHaveText([
				'ม.1/1',
				'ม.1/2',
				'ม.1/10',
				'มัธยมศึกษาปีที่ 1 · ยังไม่ได้จัดห้อง'
			]);
			const primary = table.getByRole('button', { name: 'ประถมศึกษาปีที่ 1', exact: true });
			await primary.focus();
			await primary.press('Enter');
			await expect(table.getByRole('rowheader', { name: 'ป.1/1', exact: true })).toBeVisible();
			await expect(table.getByRole('rowheader', { name: 'ม.1/1', exact: true })).toBeVisible();
			await expect(table.locator('tfoot td')).toHaveText(['10', '12', '1', '23']);
			await table.screenshot({
				path: `/tmp/public-statistics-${width}-${dark ? 'dark' : 'light'}-expanded.png`,
				animations: 'disabled'
			});
			await grade.focus();
			await grade.press('Space');
			await expect(grade).toHaveAttribute('aria-expanded', 'false');
			await expect(table.getByRole('rowheader', { name: 'ม.1/1', exact: true })).toHaveCount(0);
			await expect(primary).toHaveAttribute('aria-expanded', 'true');
			expect(fixture.requests.filter((url) => url.pathname.endsWith('/statistics')).length).toBe(
				readCount
			);
			expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(
				false
			);
		});
	}

test('empty, no-year and failed statistics retain their focused states and retry', async ({
	page
}) => {
	fixture.setStatistics({ ...publicSchoolStatistics, grades: [] });
	await page.goto(`${fixture.baseUrl}/#statistics`);
	await expect(
		page.getByText('ยังไม่มีข้อมูลนักเรียนและห้องเรียนในปีนี้', { exact: true })
	).toBeVisible();
	fixture.setStatistics({ ...publicSchoolStatistics, academicYear: null });
	await page.reload();
	await expect(
		page.getByRole('heading', { name: 'ยังไม่มีปีการศึกษาที่เปิดใช้งาน', exact: true })
	).toBeVisible();
	fixture.setFailure(true);
	await page.reload();
	await expect(page.getByRole('alert')).toContainText('โหลดสถิติโรงเรียนไม่สำเร็จ');
	fixture.setStatistics(publicSchoolStatistics);
	fixture.setFailure(false);
	await page.getByRole('button', { name: 'ลองใหม่: สถิติโรงเรียน', exact: true }).click();
	await expect(page.getByTestId('school-student-summary')).toBeVisible();
});
