import { expect, test } from '@playwright/test';
import { mockStudentSchedules, scheduleRoute } from './fixtures/student-schedules-route-data';
import { mockParent, childRoute } from './fixtures/parent-route-data';
import { revalidatePage } from './fixtures/page-revalidation';
test.use({ serviceWorkers: 'block' });

for (const owner of ['student', 'parent'] as const)
	test(`${owner} accepts removed current lesson and exam sources without restoring an old schedule`, async ({
		page
	}) => {
		const retired: string[] = [];
		page.on('request', (request) => {
			if (
				/delivery-versions|timetable-versions|term-change-sets|\/publish|source-preview/.test(
					new URL(request.url()).pathname
				)
			)
				retired.push(request.url());
		});
		const fixture = owner === 'student' ? await mockStudentSchedules(page) : await mockParent(page);
		const href = (resource: 'timetable' | 'exams' | 'calendar') =>
			owner === 'student' ? scheduleRoute(resource) : childRoute(resource);
		await page.goto(href('timetable'));
		await expect(
			page
				.getByText(owner === 'student' ? 'วิชาภาคหนึ่ง' : 'วิชาคนแรกภาคหนึ่ง', { exact: true })
				.first()
		).toBeVisible();
		// The owning backend's composed deletion is verified by real HTTP/DB tests. This browser receives its committed current projection.
		fixture.removeSources();
		await revalidatePage(page);
		await expect(
			page.getByRole('heading', { name: 'ยังไม่มีตารางเรียน', exact: true })
		).toBeVisible();
		await expect(
			page.getByText(owner === 'student' ? 'วิชาภาคหนึ่ง' : 'วิชาคนแรกภาคหนึ่ง', { exact: true })
		).toHaveCount(0);
		await page.goto(href('exams'));
		await expect(
			page.getByRole('heading', { name: 'ยังไม่มีตารางสอบ', exact: true })
		).toBeVisible();
		await page.goto(href('calendar'));
		await expect(
			page.getByRole('heading', {
				name: owner === 'student' ? 'กิจกรรม 2026-10' : 'กิจกรรมคนแรก 2026-10',
				exact: true
			})
		).toBeVisible();
		expect(retired).toEqual([]);
		expect(fixture.writes).toEqual([]);
	});
