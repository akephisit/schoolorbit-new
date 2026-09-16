import { expect, test, type Page, type Route } from '@playwright/test';

import { installTimetableMock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block', viewport: { width: 1280, height: 1080 } });

const overviewDate = '2026-09-16';
const activityOfferingId = '62000000-0000-4000-8000-000000000201';
const longActivityTitle = 'โฮมรูมป้องกันการทุจริตและส่งเสริมความเป็นพลเมืองดี';

function todayUrl(): string {
	return (
		`/staff/academic/timetable/today?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}`
	);
}

function periodId(orderIndex: number): string {
	return `42000000-0000-4000-8000-${String(orderIndex).padStart(12, '0')}`;
}

function periodTime(totalMinutes: number): string {
	const hours = Math.floor(totalMinutes / 60);
	const minutes = totalMinutes % 60;
	return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:00`;
}

function periods() {
	return Array.from({ length: 10 }, (_, index) => {
		const orderIndex = index + 1;
		const startMinutes = 8 * 60 + index * 50;
		return {
			id: periodId(orderIndex),
			orderIndex,
			name: `คาบที่ ${orderIndex}`,
			startTime: periodTime(startMinutes),
			endTime: periodTime(startMinutes + 50)
		};
	});
}

function activityEntry(index: number, homeroomNames: string[]) {
	return {
		activityId: '72000000-0000-4000-8000-000000000201',
		activitySchedulingMode: 'synchronized',
		activityVersionDisplayLabel: 'กิจกรรมโฮมรูม · v1',
		entryId: `82000000-0000-4000-8000-${String(200 + index).padStart(12, '0')}`,
		entryType: 'activity',
		homeroomNames,
		isTeamTeaching: false,
		learningGroupId: null,
		learningGroupName: null,
		note: null,
		offeringCode: 'ACTIVITY-HOMEROOM',
		offeringId: activityOfferingId,
		offeringName: 'กิจกรรมโฮมรูม',
		roomCode: null,
		subjectId: null,
		subjectVersionDisplayLabel: null,
		title: longActivityTitle
	};
}

function activityEntries() {
	const homeroomNames = Array.from(
		{ length: 18 },
		(_, index) => `ม.${Math.floor(index / 3) + 1}/${(index % 3) + 1}`
	);
	return [activityEntry(1, homeroomNames.slice(0, 9)), activityEntry(2, homeroomNames.slice(9))];
}

function courseEntry() {
	return {
		activityId: null,
		activitySchedulingMode: null,
		activityVersionDisplayLabel: null,
		entryId: '82000000-0000-4000-8000-000000000202',
		entryType: 'course',
		homeroomNames: ['ม.1/1'],
		isTeamTeaching: false,
		learningGroupId: timetableIds.groupA,
		learningGroupName: 'ม.1/1 · คณิตศาสตร์พื้นฐาน',
		note: null,
		offeringCode: 'ค21101',
		offeringId: timetableIds.offeringA,
		offeringName: 'คณิตศาสตร์พื้นฐาน',
		roomCode: '116',
		subjectId: '72000000-0000-4000-8000-000000000202',
		subjectVersionDisplayLabel: 'ค21101 คณิตศาสตร์พื้นฐาน',
		title: null
	};
}

function overview() {
	const dailyPeriods = periods();
	return {
		academicTermId: timetableIds.term,
		date: overviewDate,
		dayOfWeek: 'WED',
		periods: dailyPeriods,
		summary: {
			displayedTeacherCount: 1,
			emptyTeacherCount: 0,
			lessonCount: 3,
			teachersTeachingCount: 1,
			totalTeacherCount: 1
		},
		teachers: [
			{
				displayName: 'นายทดสอบ ตารางสอน',
				id: timetableIds.teacherA,
				periods: dailyPeriods.map((period) => ({
					bellSchedulePeriodId: period.id,
					entries:
						period.orderIndex === 1
							? activityEntries()
							: period.orderIndex === 10
								? [courseEntry()]
								: []
				}))
			}
		]
	};
}

function fulfill(route: Route, data: unknown) {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function installDailyOverview(page: Page): Promise<void> {
	await installTimetableMock(page, { periodCount: 10 });
	await page.route('**/api/academic/timetable/daily-teaching**', (route) =>
		fulfill(route, overview())
	);
}

test('fits ten equal daily periods inside the desktop table', async ({ page }) => {
	await installDailyOverview(page);
	await page.goto(todayUrl());

	const table = page.getByRole('table');
	await expect(table).toBeVisible();
	await expect(table.getByRole('columnheader')).toHaveCount(11);

	const layout = await table.evaluate((element) => {
		const container = element.parentElement;
		const periodHeaders = Array.from(element.querySelectorAll('thead th')).slice(1);
		const firstRow = element.querySelector('tbody tr');
		if (!container || !firstRow) throw new Error('Daily teaching table layout is incomplete');
		return {
			periodWidths: periodHeaders.map((header) => header.getBoundingClientRect().width),
			hasHorizontalOverflow: container.scrollWidth > container.clientWidth + 1,
			tableRight: element.getBoundingClientRect().right,
			containerRight: container.getBoundingClientRect().right,
			rowHeight: firstRow.getBoundingClientRect().height
		};
	});

	expect(layout.hasHorizontalOverflow).toBe(false);
	expect(layout.tableRight).toBeLessThanOrEqual(layout.containerRight + 1);
	expect(Math.max(...layout.periodWidths) - Math.min(...layout.periodWidths)).toBeLessThanOrEqual(
		1
	);
	expect(layout.rowHeight).toBeLessThanOrEqual(64);
});

test('shows only compact lesson context while keeping full details available', async ({ page }) => {
	await installDailyOverview(page);
	await page.goto(todayUrl());

	const table = page.getByRole('table');
	const activityCell = table.getByRole('cell').filter({ hasText: longActivityTitle });
	const activityTitle = activityCell.getByText(longActivityTitle, { exact: true });
	await expect(activityTitle).toBeVisible();
	await expect(activityCell.getByText('18 ห้อง', { exact: true })).toBeVisible();
	await expect(activityCell).not.toContainText('ACTIVITY-HOMEROOM');
	await expect(activityCell).not.toContainText('ม.6/3');
	const titleLayout = await activityTitle.evaluate((element) => {
		const style = getComputedStyle(element);
		return {
			overflow: style.overflow,
			whiteSpace: style.whiteSpace,
			isTruncated: element.scrollWidth > element.clientWidth
		};
	});
	expect(titleLayout).toEqual({ overflow: 'hidden', whiteSpace: 'nowrap', isTruncated: true });

	const courseCell = table.getByRole('cell').filter({ hasText: 'ค21101' });
	await expect(courseCell.getByText('ม.1/1 · ห้อง 116', { exact: true })).toBeVisible();
	await expect(courseCell).not.toContainText('ม.1/1 · คณิตศาสตร์พื้นฐาน');

	await courseCell.getByRole('button').click();
	await expect(page.getByRole('dialog')).toContainText('คณิตศาสตร์พื้นฐาน');
	await expect(page.getByRole('dialog')).toContainText('ม.1/1');
	await page.keyboard.press('Escape');

	await activityCell.getByRole('button').click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toContainText(longActivityTitle);
	await expect(dialog).toContainText('ACTIVITY-HOMEROOM');
	await expect(dialog).toContainText('กิจกรรมโฮมรูม');
	await expect(dialog).toContainText('กิจกรรมโฮมรูม · v1');
});

test('gives empty and scheduled period controls meaningful accessible names', async ({ page }) => {
	await installDailyOverview(page);
	await page.goto(todayUrl());

	const table = page.getByRole('table');
	await expect(
		table.getByRole('button', {
			name: `นายทดสอบ ตารางสอน คาบที่ 2 08:50–09:40: ว่าง`
		})
	).toBeVisible();
	await expect(
		table.getByRole('button', {
			name: `นายทดสอบ ตารางสอน คาบที่ 1 08:00–08:50: ${longActivityTitle}`
		})
	).toBeVisible();
});
