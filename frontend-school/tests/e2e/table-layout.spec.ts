import { expect, test } from '@playwright/test';
import { installSubjectCatalog, subjectFixture, subjectPath } from './fixtures/subject-catalog';
import { mockStudentSchedules, scheduleRoute } from './fixtures/student-schedules-route-data';

test.use({ serviceWorkers: 'block' });
for (const width of [390, 1029]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`ordinary table edges align and specialized mobile spacing stays usable at ${width}px in ${theme}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 888 });
			await installSubjectCatalog(page, subjectFixture(2), { mode: theme });
			await page.goto(subjectPath);
			await expect(page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true })).toBeVisible();
			await expect
				.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
				.toBe(theme === 'dark');
			const table = page.locator('table[aria-label="ทะเบียนรายวิชา"]');
			await expect(table.locator('tbody tr')).toHaveCount(2);
			const geometry = await table.evaluate((element) => {
				const head = element.querySelector('thead tr')!,
					body = element.querySelector('tbody tr')!;
				const measure = (cell: Element) => {
					const style = getComputedStyle(cell);
					return {
						start: parseFloat(style.paddingInlineStart),
						end: parseFloat(style.paddingInlineEnd),
						contentStart: cell.getBoundingClientRect().x + parseFloat(style.paddingLeft)
					};
				};
				return {
					root: parseFloat(getComputedStyle(document.documentElement).fontSize),
					firstHead: measure(head.firstElementChild!),
					firstBody: measure(body.firstElementChild!),
					innerHead: measure(head.children[1]),
					innerBody: measure(body.children[1]),
					lastHead: measure(head.lastElementChild!),
					lastBody: measure(body.lastElementChild!)
				};
			});
			for (const cell of [geometry.firstHead, geometry.firstBody])
				expect(cell.start).toBeCloseTo(geometry.root * 1.25, 2);
			for (const cell of [geometry.lastHead, geometry.lastBody])
				expect(cell.end).toBeCloseTo(geometry.root * 1.25, 2);
			for (const cell of [geometry.innerHead, geometry.innerBody])
				expect(cell.start).toBeCloseTo(geometry.root * 0.5, 2);
			if (width >= 768)
				expect(geometry.firstBody.contentStart).toBeCloseTo(geometry.firstHead.contentStart, 1);
			expect(
				await page.evaluate(
					() => document.documentElement.scrollWidth - document.documentElement.clientWidth
				)
			).toBeLessThanOrEqual(1);
			if (width >= 768) {
				const overflow = await table.evaluate((element) => ({
					content: element.scrollWidth,
					viewport: element.parentElement!.clientWidth
				}));
				expect(overflow.content).toBeGreaterThan(overflow.viewport);
			}
			await page.screenshot({
				path: `/workspace/artifacts/facility-room-names/table-edges-${width}-${theme}.png`,
				fullPage: true,
				animations: 'disabled'
			});
		});
		test(`personal timetable overrides shared table padding at ${width}px in ${theme}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 888 });
			await page.addInitScript(
				(mode) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: mode })),
				theme
			);
			await mockStudentSchedules(page);
			await page.goto(scheduleRoute('timetable'));
			const timetable = page.getByTestId('personal-timetable');
			await expect(timetable).toBeVisible();
			const geometry = await timetable
				.locator('tbody tr')
				.first()
				.evaluate((row) => {
					const measure = (element: Element) => ({
						start: parseFloat(getComputedStyle(element).paddingInlineStart),
						end: parseFloat(getComputedStyle(element).paddingInlineEnd)
					});
					return {
						root: parseFloat(getComputedStyle(document.documentElement).fontSize),
						head: measure(row.querySelector('th')!),
						cell: measure(row.querySelector('td')!)
					};
				});
			const expected = width < 768 ? 0 : geometry.root * 0.5;
			for (const cell of [geometry.head, geometry.cell]) {
				expect(cell.start).toBeCloseTo(expected, 2);
				expect(cell.end).toBeCloseTo(expected, 2);
			}
			await expect
				.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
				.toBe(theme === 'dark');
			expect(
				await page.evaluate(
					() => document.documentElement.scrollWidth - document.documentElement.clientWidth
				)
			).toBeLessThanOrEqual(1);
		});
	}
}
