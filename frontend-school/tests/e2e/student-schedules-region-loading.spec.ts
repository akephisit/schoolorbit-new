import { test, expect, type Page } from '@playwright/test';
import {
	mockStudentSchedules,
	scheduleRoute,
	resources,
	term,
	year,
	type Resource
} from './fixtures/student-schedules-route-data';
// Route fixtures must own requests instead of the app's network-only service worker.
test.use({ serviceWorkers: 'block' });
const ready = {
	timetable: 'วิชาภาคหนึ่ง',
	exams: 'รอบภาคหนึ่ง',
	calendar: 'กิจกรรม 2026-10',
	activities: 'กิจกรรมภาคหนึ่ง'
};
function readyRegion(page: Page, resource: Resource) {
	return resource === 'calendar'
		? page.getByRole('heading', { name: ready.calendar, exact: true })
		: page.getByText(ready[resource], { exact: true }).first();
}
const titles = {
	timetable: 'ตารางเรียน',
	exams: 'ตารางสอบ',
	calendar: 'ปฏิทิน',
	activities: 'ลงทะเบียนกิจกรรม'
};
for (const resource of Object.keys(resources) as Resource[]) {
	test(`${resource}: first skeleton and one route-owned primary`, async ({ page }) => {
		const api = await mockStudentSchedules(page, { hold: resource });
		await page.goto(scheduleRoute(resource));
		await expect(
			page.getByRole('status', { name: `กำลังโหลด${titles[resource]}`, exact: true })
		).toBeVisible();
		await expect.poll(() => api.count(resource)).toBe(1);
		expect(api.count('context')).toBe(resource === 'calendar' ? 0 : 1);
		expect(api.selfProfileCount()).toBe(0);
		expect(api.writes).toEqual([]);
		api.release();
		await expect(readyRegion(page, resource)).toBeVisible();
	});
	test(`${resource}: focused failure retry preserves scoped context`, async ({ page }) => {
		const api = await mockStudentSchedules(page, { fail: resource });
		await page.goto(scheduleRoute(resource));
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(readyRegion(page, resource)).toBeVisible();
		expect(api.count(resource)).toBe(2);
		expect(api.count('context')).toBe(resource === 'calendar' ? 0 : 1);
	});
	test(`${resource}: retained refresh error keeps useful region`, async ({ page }) => {
		const api = await mockStudentSchedules(page, { fail: resource, failAt: 2 });
		await page.goto(scheduleRoute(resource));
		await expect(readyRegion(page, resource)).toBeVisible();
		await page
			.getByRole('button', {
				name: resource === 'calendar' ? 'รีเฟรช' : 'โหลดข้อมูลใหม่',
				exact: true
			})
			.click();
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeVisible();
		await expect(readyRegion(page, resource)).toBeVisible();
		expect(api.count('context')).toBe(resource === 'calendar' ? 0 : 1);
	});
	test(`${resource}: retained refresh exposes local updating status`, async ({ page }) => {
		const api = await mockStudentSchedules(page, { hold: resource, holdAt: 2 });
		await page.goto(scheduleRoute(resource));
		await expect(readyRegion(page, resource)).toBeVisible();
		await page
			.getByRole('button', {
				name: resource === 'calendar' ? 'รีเฟรช' : 'โหลดข้อมูลใหม่',
				exact: true
			})
			.click();
		await expect.poll(() => api.count(resource)).toBe(2);
		await expect(
			page.getByRole('status', { name: 'กำลังอัปเดตข้อมูล', exact: true })
		).toBeVisible();
		await expect(readyRegion(page, resource)).toBeVisible();
		await expect(page.getByTestId(`student-${resource}-region`)).toHaveAttribute(
			'aria-busy',
			'true'
		);
		expect(api.count('context')).toBe(resource === 'calendar' ? 0 : 1);
		api.release();
		await expect(page.getByRole('status', { name: 'กำลังอัปเดตข้อมูล', exact: true })).toHaveCount(
			0
		);
		await expect(page.getByTestId(`student-${resource}-region`)).toHaveAttribute(
			'aria-busy',
			'false'
		);
	});
	if (resource !== 'calendar')
		test(`${resource}: failed prerequisite does not issue primary or paint false empty`, async ({
			page
		}) => {
			const api = await mockStudentSchedules(page, { fail: 'context' });
			await page.goto(scheduleRoute(resource));
			await expect(
				page.getByRole('button', { name: 'ลองบริบทอีกครั้ง', exact: true })
			).toBeVisible();
			expect(api.count(resource)).toBe(0);
			await expect(page.getByText('ยังไม่มีประวัติปีการศึกษา', { exact: true })).toHaveCount(0);
			await expect(page.getByText('ปีการศึกษานี้ยังไม่มีภาคเรียน', { exact: true })).toHaveCount(0);
			await page.getByRole('button', { name: 'ลองบริบทอีกครั้ง', exact: true }).click();
			await expect(readyRegion(page, resource)).toBeVisible();
			expect(api.count(resource)).toBe(1);
		});
	if (resource !== 'calendar')
		test(`${resource}: term supersession and Back own primary once`, async ({ page }) => {
			const api = await mockStudentSchedules(page, { hold: resource });
			const pending = page.waitForResponse((response) => {
				const url = new URL(response.url());
				return (
					url.pathname === resources[resource] && url.searchParams.get('academicTermId') === term
				);
			});
			await page.goto(scheduleRoute(resource));
			await page.getByRole('button', { name: 'ภาคเรียน', exact: true }).click();
			await page.getByRole('option', { name: 'ภาคสอง', exact: true }).click();
			const next = ready[resource].replace('ภาคหนึ่ง', 'ภาคสอง');
			await expect(page.getByText(next, { exact: true }).first()).toBeVisible();
			api.release();
			await pending;
			await page.evaluate(
				() =>
					new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
			);
			await expect(page.getByText(ready[resource], { exact: true })).toHaveCount(0);
			await page.goBack();
			await expect(readyRegion(page, resource)).toBeVisible();
			expect(api.count(resource)).toBe(3);
		});
}
test('calendar month shallow history refreshes events only and Back restores month', async ({
	page
}) => {
	const api = await mockStudentSchedules(page);
	await page.goto(scheduleRoute('calendar'));
	await expect(readyRegion(page, 'calendar')).toBeVisible();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page).toHaveURL(/month=2026-11/);
	await expect(page.getByRole('heading', { name: 'กิจกรรม 2026-11', exact: true })).toBeVisible();
	await page.goBack();
	await expect(readyRegion(page, 'calendar')).toBeVisible();
	expect(api.count('calendar')).toBe(3);
	expect(api.count('context')).toBe(0);
});
test('missing required term repairs URL once; foreign term never reaches primary', async ({
	page
}) => {
	const api = await mockStudentSchedules(page);
	await page.goto(`/student/exams?academicYearId=${year}&academicTermId=foreign-term`);
	await expect(page).toHaveURL(new RegExp(`academicTermId=${term}`));
	await expect(page.getByText(ready.exams, { exact: true })).toBeVisible();
	expect(api.count('context')).toBe(1);
	expect(api.count('exams')).toBe(1);
	expect(
		api.reads.find((url) => url.pathname === resources.exams)?.searchParams.get('academicTermId')
	).toBe(term);
});
test('typed registration and withdrawal patch only affected offering', async ({ page }) => {
	const api = await mockStudentSchedules(page);
	await page.goto(scheduleRoute('activities'));
	await page.getByRole('button', { name: 'ลงทะเบียนกลุ่มนี้', exact: true }).click();
	await expect(page.getByText('ลงทะเบียนแล้ว', { exact: true })).toBeVisible();
	page.once('dialog', (dialog) => dialog.accept());
	await page.getByRole('button', { name: 'ยกเลิกการลงทะเบียน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลงทะเบียนกลุ่มนี้', exact: true })).toBeVisible();
	expect(api.count('activities')).toBe(1);
	expect(api.count('context')).toBe(1);
	expect(api.count('save')).toBe(2);
});
