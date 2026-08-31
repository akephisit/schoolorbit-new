import { expect, test, type Locator } from '@playwright/test';

import { installTimetableMock, makeTimetableEntry, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function teacherUrl(teacherId: string, versionId: string = timetableIds.draftVersion): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${versionId}&view=teacher&ownerId=${teacherId}`
	);
}

async function selectPeriod(dialog: Locator, periodName: string): Promise<void> {
	await dialog.getByRole('button', { name: /^คาบ \d+$/ }).click();
	await dialog
		.page()
		.getByRole('option', { name: new RegExp(periodName) })
		.click();
}

test('projects only exact instructor periods and moves one co-taught entry for the team', async ({
	page
}) => {
	const solo = makeTimetableEntry(timetableIds.entryA, timetableIds.period1, {
		instructorIds: [timetableIds.teacherA]
	});
	const coTaught = makeTimetableEntry(timetableIds.entryB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, {
		entries: [solo, coTaught],
		eligibleInstructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});

	await page.goto(teacherUrl(timetableIds.teacherA));
	await expect(page.locator('article[data-entry-id]')).toHaveCount(2);
	await expect(page.getByText('2 คาบต่อสัปดาห์').first()).toBeVisible();

	await page
		.locator(`article[data-entry-id="${timetableIds.entryB}"]`)
		.getByRole('button', { name: 'ย้ายคาบ' })
		.click();
	await expect(page.getByText('ย้ายรายการเดียวกันสำหรับครูทุกคนในทีม')).toBeVisible();
	const dialog = page.getByRole('dialog');
	await selectPeriod(dialog, 'คาบ 3');
	await dialog.getByRole('button', { name: 'ย้ายคาบ' }).click();
	await expect(page.getByText('บันทึกตำแหน่งคาบแล้ว')).toBeVisible();

	await page.goto(teacherUrl(timetableIds.teacherB));
	await expect(page.locator('article[data-entry-id]')).toHaveCount(1);
	await expect(
		page
			.locator('td[aria-label^="วันจันทร์ คาบ 3"]')
			.locator(`article[data-entry-id="${timetableIds.entryB}"]`)
	).toBeVisible();
	expect(mock.updateRequestCount()).toBe(1);
});

test('preselects the board teacher before creating an exact-instructor period', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		eligibleInstructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	await page.goto(teacherUrl(timetableIds.teacherA));

	await page.getByRole('button', { name: 'เลือกครูและคาบ' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: /ครูคณิตศาสตร์ A/ })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(dialog.getByRole('button', { name: /ครูคณิตศาสตร์ B/ })).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	await dialog.getByRole('button', { name: 'เริ่มวาง 1 คาบ' }).click();
	await page.getByRole('button', { name: 'วางคาบที่นี่' }).first().click();

	expect(mock.entries()[0]?.instructors.map((teacher) => teacher.userId)).toEqual([
		timetableIds.teacherA
	]);
});

test('keeps published teacher boards read-only', async ({ page }) => {
	await installTimetableMock(page, {
		status: 'published',
		entries: [
			makeTimetableEntry(timetableIds.entryA, timetableIds.period1, {
				versionId: timetableIds.publishedVersion,
				instructorIds: [timetableIds.teacherA]
			})
		]
	});
	await page.goto(teacherUrl(timetableIds.teacherA, timetableIds.publishedVersion));

	await expect(page.getByText('เผยแพร่แล้ว · อ่านอย่างเดียว')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ย้ายคาบ' })).toHaveCount(0);
	await expect(page.locator('article[data-entry-id]')).toHaveAttribute('draggable', 'false');
});
