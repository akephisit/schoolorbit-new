import { expect, test } from '@playwright/test';
import type { TimetableBlockWorkspace } from '#lib/api/timetable.js';
import {
	installTimetableMock,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness.js';
import { assertTimetableTheme, setTimetableTheme } from './timetable-theme.js';

test.use({ serviceWorkers: 'block' });

const thaiTeacherId = '51000000-0000-4000-8000-000000000204';
const instructors: TimetableBlockWorkspace['learningGroups'][number]['eligibleInstructors'] = [
	{
		teacherId: timetableIds.teacherA,
		displayName: 'Mr. John Smith',
		role: 'primary',
		orderIndex: 0
	},
	{
		teacherId: timetableIds.teacherB,
		displayName: 'Ms. Mary Jones',
		role: 'secondary',
		orderIndex: 1
	},
	{ teacherId: thaiTeacherId, displayName: 'นายสมชาย ใจดี', role: 'assistant', orderIndex: 2 }
];

function group(): TimetableBlockWorkspace['learningGroups'][number] {
	return {
		id: timetableIds.groupA,
		learningOfferingId: timetableIds.offeringA,
		code: 'M1-1-MATH',
		name: 'ม.1/1 คณิตศาสตร์',
		status: 'published',
		rosterStatus: 'published',
		offeringKind: 'course',
		subjectType: 'BASIC',
		offeringCode: 'ค21101',
		offeringName: 'คณิตศาสตร์พื้นฐาน',
		homeroomIds: [timetableIds.homeroom],
		preferredRoomIds: [],
		eligibleInstructors: instructors
	};
}

for (const theme of ['light', 'dark'] as const)
	for (const width of [1920, 390]) {
		test(`keeps resolved English teacher names selectable and assigned at ${width}px in ${theme}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: width === 390 ? 844 : 1080 });
			const learningGroup = group();
			const existing = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
				instructorIds: [timetableIds.teacherB, thaiTeacherId]
			});
			existing.groups[0].instructors = [instructors[1], instructors[2]].map((teacher) => ({
				...teacher
			}));
			const mock = await installTimetableMock(page, {
				blocks: [existing],
				learningGroups: [learningGroup],
				staff: instructors.map((teacher) => ({
					id: teacher.teacherId,
					displayName: teacher.displayName,
					subjectGroups: [],
					status: 'active'
				})),
				ordinaryDemands: [
					{
						learningGroupId: learningGroup.id,
						learningOfferingId: learningGroup.learningOfferingId,
						offeringCode: learningGroup.offeringCode,
						offeringName: learningGroup.offeringName,
						requiredPeriods: 3,
						scheduledPeriods: 1,
						remainingPeriods: 2,
						homeroomIds: learningGroup.homeroomIds,
						eligibleInstructors: instructors
					}
				]
			});
			await page.goto(
				`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${timetableIds.draftVersion}&view=homeroom&ownerId=${timetableIds.homeroom}`
			);
			await setTimetableTheme(page, theme);
			const existingCard = page.locator(
				`[data-timetable-lesson-card][data-block-id="${timetableIds.blockA}"]`
			);
			await expect(existingCard.getByText('ครูMary +1', { exact: true })).toBeVisible();
			await expect(existingCard).toHaveAttribute('aria-label', /ครูMary, ครูสมชาย/);
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			await page.getByRole('button', { name: 'ครู', exact: true }).click();
			const owner = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
			await expect(owner).toHaveText('Mr. John Smith');
			await expect(page.getByRole('heading', { name: 'ครูJohn', exact: true })).toBeVisible();
			await owner.click();
			for (const teacher of instructors)
				await expect(
					page.getByRole('option', { name: teacher.displayName, exact: true })
				).toBeVisible();
			await page.getByRole('option', { name: 'Ms. Mary Jones', exact: true }).click();
			await expect(owner).toHaveText('Ms. Mary Jones');
			await expect(page.getByRole('heading', { name: 'ครูMary', exact: true })).toBeVisible();
			expect(new URL(page.url()).searchParams.get('ownerId')).toBe(timetableIds.teacherB);
			const trayCard = page.locator(
				`aside article[data-lesson-color="group:${timetableIds.groupA}"]`
			);
			await trayCard.getByRole('button', { name: /เลือกครู/ }).click();
			const john = page.getByRole('button', { name: 'Mr. John Smith ครูหลัก', exact: true });
			const mary = page.getByRole('button', { name: 'Ms. Mary Jones ครูร่วมสอน', exact: true });
			const thai = page.getByRole('button', { name: 'นายสมชาย ใจดี ครูผู้ช่วย', exact: true });
			await expect(john).toBeVisible();
			await expect(john.getByText('Mr. John Smith', { exact: true })).toBeVisible();
			await expect(mary.getByText('Ms. Mary Jones', { exact: true })).toBeVisible();
			await expect(thai.getByText('นายสมชาย ใจดี', { exact: true })).toBeVisible();
			await expect(john).toHaveAttribute('aria-pressed', 'true');
			await expect(mary).toHaveAttribute('aria-pressed', 'false');
			await expect(thai).toHaveAttribute('aria-pressed', 'false');
			await mary.click();
			await expect(john).toHaveAttribute('aria-pressed', 'true');
			await expect(mary).toHaveAttribute('aria-pressed', 'true');
			await expect(thai).toHaveAttribute('aria-pressed', 'false');
			await assertTimetableTheme(page, theme);
			await page.screenshot({
				path: testInfo.outputPath(`teacher-name-picker-${width}-${theme}.png`),
				fullPage: true
			});
			await page.keyboard.press('Escape');
			await trayCard.locator('button').first().click();
			await page
				.locator(`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period2}"]`)
				.getByRole('button', { name: 'วางคาบที่นี่', exact: true })
				.click();
			const saved = page.locator(
				`[data-timetable-lesson-card][data-block-id="${timetableIds.createdBlock}"]`
			);
			await expect(saved).toHaveAttribute('aria-busy', 'false');
			expect(mock.createRequestCount()).toBe(1);
			expect(mock.lastCreateBody()).toMatchObject({
				instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
			});
			expect(
				mock.blocks().find((block) => block.id === timetableIds.createdBlock)?.groups[0].instructors
			).toEqual(instructors.slice(0, 2));
			await saved.getByRole('button', { name: /ดูรายละเอียด/ }).click();
			const dialog = page.getByRole('dialog');
			await expect(
				dialog.getByRole('button', { name: 'Mr. John Smith ครูหลัก', exact: true })
			).toHaveAttribute('aria-pressed', 'true');
			await expect(
				dialog.getByRole('button', { name: 'Ms. Mary Jones ครูร่วมสอน', exact: true })
			).toHaveAttribute('aria-pressed', 'true');
			await expect(
				dialog.getByRole('button', { name: 'นายสมชาย ใจดี ครูผู้ช่วย', exact: true })
			).toHaveAttribute('aria-pressed', 'false');
			await dialog.getByRole('button', { name: 'ปิด', exact: true }).click();
			await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
			await expect(saved.getByText('ครูJohn +1', { exact: true })).toBeVisible();
			await expect(saved).toHaveAttribute('aria-label', /ครูJohn, ครูMary/);
			await page.reload();
			await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
			await assertTimetableTheme(page, theme);
			await expect(saved.getByText('ครูJohn +1', { exact: true })).toBeVisible();
			await expect(existingCard.getByText('ครูMary +1', { exact: true })).toBeVisible();
			expect(mock.createRequestCount()).toBe(1);
			expect(mock.updateRequestCount() + mock.swapRequestCount() + mock.deleteRequestCount()).toBe(
				0
			);
		});
	}
