import { expect, test } from '@playwright/test';
import { mockYearManagement, yearUrl, id } from './fixtures/year-management';
test.use({ serviceWorkers: 'block' });
test('direct term close and activate use the same annual roster without result preparation', async ({
	page
}) => {
	const observed = await mockYearManagement(page, { active: true });
	await page.goto(yearUrl);
	await page.getByRole('button', { name: 'ปิดภาคเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันปิดภาคเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เปลี่ยนภาคเรียน', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText(
		'ภายในปีเดียวกันนักเรียนยังอยู่ชั้นและห้องเดิม'
	);
	await page.getByRole('button', { name: 'ยืนยันเปลี่ยนภาคเรียน', exact: true }).click();
	expect(observed.transitions.map((request) => request.action)).toEqual(['close', 'activate']);
	expect(observed.transitions.every((request) => request.academicYearId === id(1))).toBe(true);
	expect(observed.confirms).toEqual([]);
	expect(observed.reads.some((path) => /result|aggregate/.test(path))).toBe(false);
});
test('legacy term link preserves the year and leads to one canonical workflow', async ({
	page
}) => {
	await mockYearManagement(page);
	await page.goto(`/staff/academic/term-lifecycle?academicYearId=${id(1)}&academicTermId=${id(3)}`);
	await expect(page).toHaveURL(yearUrl);
	await expect(page.getByRole('heading', { name: 'จัดการปีการศึกษา', exact: true })).toBeVisible();
});
