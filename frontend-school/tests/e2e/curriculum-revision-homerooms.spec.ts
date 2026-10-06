import { expect, test } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.setTimeout(60_000);

const yearId = '11000000-0000-4000-8000-000000000001';
const gradeId = '21000000-0000-4000-8000-000000000001';
const seniorGradeId = '21000000-0000-4000-8000-000000000004';
const planId = '31000000-0000-4000-8000-000000000001';

for (const mobile of [false, true]) {
	for (const dark of [false, true]) {
		test(`a room in 2572 explicitly chooses revision 2569 (${mobile ? 'mobile' : 'desktop'}, ${dark ? 'dark' : 'light'})`, async ({
			page
		}) => {
			await page.setViewportSize({ width: mobile ? 390 : 1440, height: 900 });
			let submitted: Record<string, unknown> | null = null;
			await page.route(
				(url) => url.pathname.startsWith('/api/'),
				async (route) => {
					const path = new URL(route.request().url()).pathname;
					let data: unknown = [];
					if (path === '/api/auth/me') {
						data = {
							id: '91000000-0000-4000-8000-000000000001',
							username: 'revision-fixture',
							firstName: 'ทดสอบ',
							lastName: 'วิชาการ',
							userType: 'staff',
							status: 'ACTIVE',
							permissions: ['*']
						};
					} else if (path === '/api/academic/context/options') {
						data = {
							activeAcademicYearId: yearId,
							activeAcademicTermId: null,
							years: [{ id: yearId, name: 'ปีการศึกษา 2572', year: 2572, status: 'planning' }],
							terms: []
						};
					} else if (path === '/api/menu/user') data = { groups: [] };
					else if (path === '/api/lookup/grade-levels') {
						data = [
							{
								id: gradeId,
								name: 'มัธยมศึกษาปีที่ 1',
								short_name: 'ม.1',
								code: 'M1',
								level_type: 'secondary',
								year: 1
							},
							{
								id: seniorGradeId,
								name: 'มัธยมศึกษาปีที่ 4',
								short_name: 'ม.4',
								code: 'M4',
								level_type: 'secondary',
								year: 4
							}
						];
					} else if (path === '/api/academic/study-program-options') {
						data = [
							{
								id: planId,
								name: 'วิทยาศาสตร์-คณิตศาสตร์',
								code: 'SCI-MATH',
								editionId: '41000000-0000-4000-8000-000000000001',
								levelName: 'ระดับมัธยมศึกษาตอนต้น',
								curriculumLevelId: '51000000-0000-4000-8000-000000000001',
								editionName: '2569',
								revisionYear: 2569,
								gradeLevelIds: [gradeId]
							},
							{
								id: '31000000-0000-4000-8000-000000000004',
								name: 'วิทยาศาสตร์-คณิตศาสตร์',
								code: 'SCI-MATH',
								editionId: '41000000-0000-4000-8000-000000000004',
								levelName: 'ระดับมัธยมศึกษาตอนปลาย',
								curriculumLevelId: '51000000-0000-4000-8000-000000000004',
								editionName: '2569',
								revisionYear: 2569,
								gradeLevelIds: [seniorGradeId]
							}
						];
					} else if (path === '/api/academic/homerooms' && route.request().method() === 'POST') {
						submitted = route.request().postDataJSON();
						data = {
							...submitted,
							id: '61000000-0000-4000-8000-000000000001',
							name: 'ม.1/1',
							code: 'M1-1',
							isActive: true,
							rowVersion: 1
						};
					}
					await route.fulfill({
						contentType: 'application/json',
						body: JSON.stringify({ success: true, data })
					});
				}
			);
			await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
			await expect(page.getByTestId('homerooms-ready')).toBeVisible({ timeout: 30_000 });
			if (dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
				await expect(page.locator('html')).toHaveClass(/dark/);
			}
			await page.getByRole('button', { name: 'เพิ่มห้องประจำชั้น' }).click();
			const dialog = page.getByRole('dialog');
			await dialog.getByLabel('ระดับชั้น', { exact: true }).click();
			await page.getByRole('option', { name: 'มัธยมศึกษาปีที่ 1', exact: true }).click();
			await dialog.getByLabel('ฉบับหลักสูตร', { exact: true }).click();
			await expect(page.getByRole('option')).toHaveCount(1);
			await page.getByRole('option', { name: '2569', exact: true }).click();
			await dialog.getByLabel('แผนการเรียน', { exact: true }).click();
			await expect(page.getByRole('option')).toHaveCount(1);
			await page.getByRole('option', { name: /ฉบับปรับปรุง พุทธศักราช 2569/ }).click();
			await dialog.getByLabel('เลขห้อง', { exact: true }).fill('1');
			await expect(page.getByRole('option')).toHaveCount(0);
			await expect
				.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth))
				.toBe(true);
			await page.screenshot({
				path: `test-results/curriculum-room-${mobile ? 'mobile' : 'desktop'}-${dark ? 'dark' : 'light'}.png`
			});
			await dialog.getByRole('button', { name: 'สร้างห้อง', exact: true }).click();
			await expect
				.poll(() => submitted)
				.toMatchObject({ academicYearId: yearId, gradeLevelId: gradeId, studyProgramId: planId });
			await expect(dialog).toBeHidden();
		});
	}
}
