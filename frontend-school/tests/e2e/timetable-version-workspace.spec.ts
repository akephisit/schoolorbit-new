import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableEntry, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function timetableUrl(versionId: string): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${versionId}` +
		`&view=homeroom&ownerId=${timetableIds.homeroom}`
	);
}

test('loads one bounded draft workspace and preserves exact attached teachers', async ({
	page
}) => {
	const entry = makeTimetableEntry(timetableIds.entryA, timetableIds.period1, {
		instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, {
		entries: [entry],
		eligibleInstructorIds: [timetableIds.teacherA]
	});

	await page.goto(timetableUrl(timetableIds.draftVersion));

	await expect(page.getByRole('heading', { name: 'จัดตารางสอน' })).toBeVisible();
	await expect(page.getByText('แบบร่าง · แก้ไขได้')).toBeVisible();
	await expect(page.getByText('ครูคณิตศาสตร์ A, ครูคณิตศาสตร์ B')).toBeVisible();
	expect(mock.workspaceRequestCount()).toBe(1);

	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	await expect(page.getByRole('dialog').getByText('ครูคณิตศาสตร์ B')).toBeVisible();
});

test('renders a published timetable as the same read-only board', async ({ page }) => {
	const entry = makeTimetableEntry(timetableIds.entryA, timetableIds.period1, {
		versionId: timetableIds.publishedVersion
	});
	await installTimetableMock(page, { status: 'published', entries: [entry] });

	await page.goto(timetableUrl(timetableIds.publishedVersion));

	await expect(page.getByText('เผยแพร่แล้ว · อ่านอย่างเดียว')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ย้ายคาบ' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'นำออกจากตาราง' })).toHaveCount(0);

	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('heading', { name: 'รายละเอียดคาบ' })).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง' })).toHaveCount(0);
});
