import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function teacherUrl(teacherId: string, versionId: string = timetableIds.draftVersion): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${versionId}&view=teacher&ownerId=${teacherId}`
	);
}

test('projects only exact instructor periods and moves one co-taught block for the team', async ({
	page
}) => {
	const solo = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
		instructorIds: [timetableIds.teacherA]
	});
	const coTaught = makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, {
		blocks: [solo, coTaught],
		eligibleInstructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});

	await page.goto(teacherUrl(timetableIds.teacherA));
	await expect(page.locator('article[data-block-id]')).toHaveCount(2);

	const source = page.locator(`article[data-block-id="${timetableIds.blockB}"]`);
	const destination = page
		.locator(`td[data-timetable-period-id="${timetableIds.period3}"]`)
		.first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await source.dispatchEvent('dragstart', { dataTransfer });
	await destination.dispatchEvent('dragover', { dataTransfer });
	await expect(destination).toHaveAttribute('data-state', 'move');
	await destination.dispatchEvent('drop', { dataTransfer });
	await source.dispatchEvent('dragend', { dataTransfer });
	await expect(page.getByText('บันทึกตำแหน่งคาบแล้ว')).toBeVisible();

	await page.goto(teacherUrl(timetableIds.teacherB));
	await expect(page.locator('article[data-block-id]')).toHaveCount(1);
	await expect(
		page
			.locator('td[aria-label^="วันจันทร์ คาบ 3"]')
			.locator(`article[data-block-id="${timetableIds.blockB}"]`)
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

	const trayCard = page.locator('aside article').filter({ hasText: 'ค21101' }).first();
	await trayCard.getByRole('button', { name: /เลือกครู/ }).click();
	await expect(page.getByRole('button', { name: /ครูคณิตศาสตร์ A/ })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page.getByRole('button', { name: /ครูคณิตศาสตร์ B/ })).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	await page.keyboard.press('Escape');
	await trayCard.locator('button').first().click();
	await page.getByRole('button', { name: 'วางคาบที่นี่' }).first().click();

	expect(mock.blocks()[0]?.groups[0]?.instructors.map((teacher) => teacher.teacherId)).toEqual([
		timetableIds.teacherA
	]);
});

test('keeps published teacher boards read-only', async ({ page }) => {
	await installTimetableMock(page, {
		status: 'published',
		blocks: [
			makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
				versionId: timetableIds.publishedVersion,
				instructorIds: [timetableIds.teacherA]
			})
		]
	});
	await page.goto(teacherUrl(timetableIds.teacherA, timetableIds.publishedVersion));

	await expect(page.getByText('เผยแพร่แล้ว · อ่านอย่างเดียว')).toBeVisible();
	await expect(page.locator('article[data-block-id]')).toHaveAttribute('draggable', 'false');
});
