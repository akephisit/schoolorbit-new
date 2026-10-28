import { expect, test } from '@playwright/test';
import { installTimetableMock, timetableIds } from './timetable-test-harness.js';
import { makeSortingFixture, sortingIds } from './timetable-sorting-fixtures.js';
import { setTimetableTheme, assertTimetableTheme } from './timetable-theme.js';

test.use({ serviceWorkers: 'block' });

for (const theme of ['light', 'dark'] as const)
	for (const width of [1920, 390]) {
		test(`orders each timetable context and preserves choices at ${width}px in ${theme}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: width === 390 ? 844 : 1080 });
			const fixture = makeSortingFixture();
			const mock = await installTimetableMock(page, fixture);
			await page.goto(
				`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${timetableIds.draftVersion}&view=homeroom&ownerId=${sortingIds.room2}`
			);
			await setTimetableTheme(page, theme);
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			const tray = page.getByRole('complementary', { name: 'คาบที่ยังไม่ได้จัด' });
			const cardOrder = () =>
				tray
					.locator('article[data-lesson-color]')
					.evaluateAll((cards) => cards.map((card) => card.getAttribute('data-lesson-color')));
			await expect(tray.locator('h3')).toHaveText([
				'รายวิชาพื้นฐาน',
				'รายวิชาเพิ่มเติม',
				'รายวิชาอื่น',
				'กิจกรรมพัฒนาผู้เรียน'
			]);
			expect(await cardOrder()).toEqual([
				`group:${timetableIds.groupA}`,
				`group:${timetableIds.groupB}`,
				`group:${sortingIds.additional}`,
				`group:${sortingIds.unknown}`,
				`group:${sortingIds.courseActivity}`,
				`offering:${sortingIds.sync}`,
				`group:${sortingIds.activity}`
			]);
			await expect(tray.getByText('พร้อมกัน', { exact: true })).toBeVisible();
			const card = tray.locator(`article[data-lesson-color="group:${timetableIds.groupA}"]`);
			const fill = await card.evaluate((element) => getComputedStyle(element).backgroundColor);
			await card.getByRole('combobox', { name: 'เลือกห้องเรียน' }).click();
			await page.getByRole('option', { name: 'ห้องปฏิบัติการ 2', exact: true }).click();
			await card.getByRole('button', { name: /เลือกครู/ }).click();
			await page.getByRole('button', { name: /ครูคณิตศาสตร์ B/ }).click();
			await page.keyboard.press('Escape');
			await expect(card.getByRole('button', { name: /เลือกครู/ })).toContainText('เลือกครู 2 คน');
			await page.getByRole('button', { name: 'ครู', exact: true }).click();
			await assertTimetableTheme(page, theme);
			await expect(tray.locator('h3')).toHaveText(['ป.2', 'ม.1', 'ม.2', 'ไม่ระบุระดับชั้น']);
			await expect(tray.locator('[data-timetable-grade-section="primary:2"] h4')).toHaveText([
				'รายวิชาพื้นฐาน',
				'รายวิชาเพิ่มเติม',
				'กิจกรรมพัฒนาผู้เรียน'
			]);
			expect(await cardOrder()).toEqual([
				`group:${sortingIds.multi}`,
				`group:${sortingIds.primaryBasic}`,
				`group:${sortingIds.primaryAdditional}`,
				`offering:${sortingIds.syncPrimary}`,
				`group:${timetableIds.groupA}`,
				`group:${sortingIds.room10Group}`,
				`group:${timetableIds.groupB}`,
				`group:${sortingIds.additional}`,
				`group:${sortingIds.unknown}`,
				`group:${sortingIds.courseActivity}`,
				`offering:${sortingIds.sync}`,
				`group:${sortingIds.activity}`,
				`offering:${sortingIds.syncGrade2}`,
				`group:${sortingIds.missing}`
			]);
			await expect(
				tray.locator(`article[data-lesson-color="group:${sortingIds.multi}"]`)
			).toHaveCount(1);
			await expect(
				tray.locator(
					`article[data-lesson-color="group:${sortingIds.multi}"] [data-timetable-demand-targets]`
				)
			).toHaveText('ม.2/1, ป.2/1');
			await expect(card.getByRole('combobox', { name: 'เลือกห้องเรียน' })).toContainText(
				'ห้องปฏิบัติการ 2'
			);
			await expect(card.getByRole('button', { name: /เลือกครู/ })).toContainText('เลือกครู 2 คน');
			expect(await card.evaluate((element) => getComputedStyle(element).backgroundColor)).toBe(
				fill
			);
			await assertTimetableTheme(page, theme);
			await page.screenshot({
				path: testInfo.outputPath(`teacher-sorting-${width}-${theme}.png`),
				fullPage: true
			});
			await page.getByRole('button', { name: 'กลุ่มเรียน', exact: true }).click();
			const owner = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
			await owner.click();
			await page
				.getByRole('option', {
					name: fixture.learningGroups.find((group) => group.id === timetableIds.groupA)!.name,
					exact: true
				})
				.click();
			expect(await cardOrder()).toEqual([
				`group:${timetableIds.groupA}`,
				`offering:${sortingIds.sync}`
			]);
			expect(new URL(page.url()).searchParams.get('ownerId')).toBe(timetableIds.groupA);
			await owner.click();
			await page
				.getByRole('option', {
					name: fixture.learningGroups.find((group) => group.id === sortingIds.multi)!.name,
					exact: true
				})
				.click();
			expect(await cardOrder()).toEqual([
				`group:${sortingIds.multi}`,
				`offering:${sortingIds.syncPrimary}`,
				`offering:${sortingIds.syncGrade2}`
			]);
			await owner.click();
			await page
				.getByRole('option', {
					name: fixture.learningGroups.find((group) => group.id === sortingIds.missing)!.name,
					exact: true
				})
				.click();
			expect(await cardOrder()).toEqual([`group:${sortingIds.missing}`]);
			await page.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
			await expect(tray).toHaveCount(0);
			const overview = page
				.locator('section')
				.filter({ has: page.getByRole('heading', { name: 'ภาพรวมทั้งโรงเรียน · วันจันทร์' }) });
			await expect(overview.locator('tbody th')).toHaveText(['ป.2/1', 'ม.1/2', 'ม.1/10', 'ม.2/1']);
			await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
			await owner.click();
			await expect(page.getByRole('option')).toHaveText(['ป.2/1', 'ม.1/2', 'ม.1/10', 'ม.2/1']);
			await page.getByRole('option', { name: 'ม.1/2', exact: true }).click();
			expect(await card.evaluate((element) => getComputedStyle(element).backgroundColor)).toBe(
				fill
			);
			await assertTimetableTheme(page, theme);
			expect(mock.workspaceRequestCount()).toBe(1);
			expect(
				mock.createRequestCount() +
					mock.synchronizedCreateRequestCount() +
					mock.updateRequestCount() +
					mock.swapRequestCount() +
					mock.deleteRequestCount()
			).toBe(0);
		});
	}
