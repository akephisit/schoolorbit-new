import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableEntry, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function wholeSchoolUrl(): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${timetableIds.draftVersion}&view=wholeSchool`
	);
}

test('loads one read-only school day at a time and reuses the day cache', async ({ page }) => {
	const mock = await installTimetableMock(page, {
		entries: [makeTimetableEntry(timetableIds.entryA, timetableIds.period1)]
	});
	await page.goto(wholeSchoolUrl());

	await expect(page.getByText('ภาพรวมทั้งโรงเรียน · ดูอย่างเดียว')).toBeVisible();
	await expect.poll(() => mock.overviewRequestCount()).toBe(1);
	await expect(page.getByRole('button', { name: /ค21101/ })).toBeVisible();
	await expect(page.locator('[draggable="true"]')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ย้ายคาบ' })).toHaveCount(0);

	await page.getByRole('button', { name: 'อังคาร', exact: true }).click();
	await expect.poll(() => mock.overviewRequestCount()).toBe(2);
	await expect(page.getByText('0 คาบ')).toBeVisible();
	await page.getByRole('button', { name: 'จันทร์', exact: true }).click();
	await expect(page.getByRole('button', { name: /ค21101/ })).toBeVisible();
	await expect.poll(() => mock.overviewRequestCount()).toBe(2);
	expect(mock.overviewDays()).toEqual(['MON', 'TUE']);
	expect(mock.createRequestCount()).toBe(0);
	expect(mock.updateRequestCount()).toBe(0);
	expect(mock.swapRequestCount()).toBe(0);
});

test('opens the exact editable teacher projection from a typed issue', async ({ page }) => {
	await installTimetableMock(page, {
		entries: [
			makeTimetableEntry(timetableIds.entryA, timetableIds.period1, {
				instructorIds: [timetableIds.teacherA]
			})
		],
		overviewIssues: [
			{
				kind: 'instructor_conflict',
				severity: 'blocking',
				message: 'ครูคณิตศาสตร์ A มีคาบชนกัน',
				bellSchedulePeriodId: timetableIds.period1,
				entryIds: [timetableIds.entryA],
				homeroomIds: [timetableIds.homeroom],
				instructorIds: [timetableIds.teacherA],
				learningGroupId: timetableIds.groupA,
				roomId: null
			}
		]
	});
	await page.goto(wholeSchoolUrl());

	await page.getByRole('button', { name: /เปิดตารางครู/ }).click();
	await expect(page.getByRole('button', { name: 'ครูผู้สอน' })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page).toHaveURL(new RegExp(`view=teacher.*ownerId=${timetableIds.teacherA}`));
	await expect(page).toHaveURL(new RegExp(`timetableVersionId=${timetableIds.draftVersion}`));
	await expect(page.locator(`article[data-entry-id="${timetableIds.entryA}"]`)).toBeVisible();
});

test('opens the exact editable homeroom projection from a typed issue', async ({ page }) => {
	await installTimetableMock(page, {
		entries: [makeTimetableEntry(timetableIds.entryA, timetableIds.period1)],
		overviewIssues: [
			{
				kind: 'room_conflict',
				severity: 'blocking',
				message: 'ห้อง MATH-1 ถูกใช้ซ้ำ',
				bellSchedulePeriodId: timetableIds.period1,
				entryIds: [timetableIds.entryA],
				homeroomIds: [],
				instructorIds: [],
				learningGroupId: timetableIds.groupA,
				roomId: timetableIds.room
			}
		]
	});
	await page.goto(wholeSchoolUrl());

	await page.getByRole('button', { name: /แก้ในตารางห้อง/ }).click();
	await expect(page.getByRole('button', { name: 'ห้องประจำชั้น' })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page).toHaveURL(new RegExp(`view=homeroom.*ownerId=${timetableIds.homeroom}`));
	await expect(page).toHaveURL(new RegExp(`focusPeriodId=${timetableIds.period1}`));
});
