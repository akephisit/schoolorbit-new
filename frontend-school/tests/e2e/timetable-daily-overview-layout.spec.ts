import { expect, test, type Page, type Route } from '@playwright/test';

import { PERMISSIONS } from '../../src/lib/permissions/registry.generated';
import { installTimetableMock, timetableIds } from './timetable-test-harness';
import type { DailyTeachingEntry, DailyTeachingOverview } from '../../src/lib/api/timetable.js';

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

function activityEntry(index: number, homeroomNames: string[]): DailyTeachingEntry {
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
		roomId: null,
		roomName: null,
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

function courseEntry(): DailyTeachingEntry {
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
		roomId: timetableIds.room,
		roomName: 'ห้องคณิตศาสตร์',
		subjectId: '72000000-0000-4000-8000-000000000202',
		subjectVersionDisplayLabel: 'ค21101 คณิตศาสตร์พื้นฐาน',
		title: null
	};
}

function overview(): DailyTeachingOverview {
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

async function installDailyOverview(page: Page, data = overview(), permissions?: string[]) {
	const fixture = await installTimetableMock(page, { periodCount: 10, permissions });
	let reads = 0;
	await page.route('**/api/academic/timetable/daily-teaching**', (route) => {
		reads += 1;
		return fulfill(route, data);
	});
	return { count: () => reads, setPermissions: fixture.setPermissions };
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
	expect(layout.rowHeight).toBeGreaterThanOrEqual(88);
	expect(layout.rowHeight).toBeLessThanOrEqual(104);
});

test('shows reference code, subject and separate context lines while keeping full details available', async ({
	page
}) => {
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
			lineClamp: style.webkitLineClamp,
			height: element.getBoundingClientRect().height,
			lineHeight: parseFloat(style.lineHeight)
		};
	});
	expect(titleLayout.overflow).toBe('hidden');
	expect(titleLayout.whiteSpace).toBe('pre-line');
	expect(titleLayout.lineClamp).toBe('3');
	expect(titleLayout.height).toBeGreaterThan(titleLayout.lineHeight);
	expect(titleLayout.height).toBeLessThanOrEqual(titleLayout.lineHeight * 3 + 1);

	const courseCell = table.getByRole('cell').filter({ hasText: 'ค21101' });
	await expect(courseCell.locator('[data-daily-card-code]')).toHaveText('ค21101');
	await expect(courseCell.locator('[data-daily-card-title]')).toHaveText('คณิตศาสตร์พื้นฐาน');
	await expect(courseCell.locator('[data-daily-card-target]')).toHaveText('ม.1/1');
	await expect(courseCell.locator('[data-daily-card-room]')).toHaveText('ห้องคณิตศาสตร์');
	await expect(courseCell.locator('[data-daily-card-room] svg')).toHaveCount(1);
	await expect(courseCell.locator('[data-daily-card-teachers]')).toHaveCount(0);
	const lineGeometry = await courseCell.evaluate((cell) => {
		const code = cell.querySelector('[data-daily-card-code]');
		const title = cell.querySelector('[data-daily-card-title]');
		const target = cell.querySelector('[data-daily-card-target]');
		const room = cell.querySelector('[data-daily-card-room]');
		if (!code || !title || !target || !room)
			throw new Error('Course reference lines are incomplete');
		return {
			codeBottom: code.getBoundingClientRect().bottom,
			titleTop: title.getBoundingClientRect().top,
			targetBottom: target.getBoundingClientRect().bottom,
			roomTop: room.getBoundingClientRect().top
		};
	});
	expect(lineGeometry.titleTop).toBeGreaterThanOrEqual(lineGeometry.codeBottom);
	expect(lineGeometry.roomTop).toBeGreaterThanOrEqual(lineGeometry.targetBottom);

	await expect(courseCell).not.toContainText('ม.1/1 · คณิตศาสตร์พื้นฐาน');
	await expect(courseCell).not.toContainText('116');

	await courseCell.getByRole('button').click();
	await expect(page.getByRole('dialog')).toContainText('คณิตศาสตร์พื้นฐาน');
	await expect(page.getByRole('dialog')).toContainText('ห้องคณิตศาสตร์');
	await expect(page.getByRole('dialog')).not.toContainText('116');
	await expect(page.getByRole('dialog')).toContainText('ม.1/1');
	await page.keyboard.press('Escape');

	await activityCell.getByRole('button').click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toContainText(longActivityTitle);
	await expect(dialog).toContainText('ACTIVITY-HOMEROOM');
	await expect(dialog).toContainText('กิจกรรมโฮมรูม');
	await expect(dialog).toContainText('กิจกรรมโฮมรูม · v1');
});

test('keeps saved custom course titles in details while cards show subject code and name', async ({
	page
}) => {
	const data = overview();
	const customTitle = 'ทบทวนก่อนสอบ';
	data.teachers[0].periods[9].entries = [{ ...courseEntry(), title: customTitle }];
	await installDailyOverview(page, data);
	await page.goto(todayUrl());
	for (const view of ['ครู', 'ห้องเรียน']) {
		await page.getByRole('radio', { name: view, exact: true }).click();
		const cell = page.getByRole('table').getByRole('cell').filter({ hasText: 'ค21101' });
		await expect(cell.locator('[data-daily-card-code]')).toHaveText('ค21101');
		await expect(cell.locator('[data-daily-card-title]')).toHaveText('คณิตศาสตร์พื้นฐาน');
		await expect(cell).not.toContainText(customTitle);
		await cell.getByRole('button').click();
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByText(customTitle, { exact: true })).toBeVisible();
		await expect(dialog.getByText('คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
		await expect(dialog).toContainText('ห้องคณิตศาสตร์');
		await expect(dialog).toContainText('ค21101');
		await expect(dialog).not.toContainText('116');
		await page.keyboard.press('Escape');
	}
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

function multiOwnerOverview(): DailyTeachingOverview {
	const data = overview();
	const first = data.teachers[0];
	first.periods = data.periods.map((period) => ({
		bellSchedulePeriodId: period.id,
		entries:
			period.orderIndex === 1
				? [activityEntry(1, ['ม.1/10', 'ม.1/2'])]
				: period.orderIndex === 2
					? [{ ...courseEntry(), isTeamTeaching: true }]
					: period.orderIndex === 3
						? [1, 2].map((index) => ({
								...activityEntry(10 + index, ['ม.1/2']),
								activitySchedulingMode: 'independent',
								title: `กิจกรรมแยก ${index}`
							}))
						: period.orderIndex === 4
							? [{ ...activityEntry(20, []), title: 'กิจกรรมไม่ระบุห้อง' }]
							: []
	}));
	data.teachers.push(
		{
			id: timetableIds.teacherB,
			displayName: 'ครูร่วมสอน',
			periods: data.periods.map((period) => ({
				bellSchedulePeriodId: period.id,
				entries:
					period.orderIndex === 1
						? [activityEntry(2, ['ม.1/2'])]
						: period.orderIndex === 2
							? [{ ...courseEntry(), isTeamTeaching: true }]
							: []
			}))
		},
		{
			id: '91000000-0000-4000-8000-000000000003',
			displayName: 'ครูอีกห้อง',
			periods: data.periods.map((period) => ({
				bellSchedulePeriodId: period.id,
				entries: period.orderIndex === 1 ? [activityEntry(3, ['ม.2/1'])] : []
			}))
		}
	);
	data.summary = {
		displayedTeacherCount: 3,
		emptyTeacherCount: 0,
		lessonCount: 7,
		teachersTeachingCount: 3,
		totalTeacherCount: 3
	};
	return data;
}

test('searches and selects owners in either view without another daily read', async ({ page }) => {
	await installDailyOverview(page, multiOwnerOverview());
	let reads = 0;
	page.on('request', (request) => {
		if (request.url().includes('/api/academic/timetable/daily-teaching')) reads += 1;
	});
	await page.goto(todayUrl());
	const table = page.getByRole('table');
	await expect(table.locator('tbody tr')).toHaveCount(3);
	const initialDate = await page.locator('#teaching-date').innerText();
	const teacherPicker = page.getByRole('combobox', { name: 'เลือกครู', exact: true });
	await teacherPicker.click();
	await page.getByPlaceholder('ค้นหาครู...').fill('ร่วม');
	await expect(page.getByRole('option')).toHaveCount(1);
	await page.getByRole('option', { name: 'ครูร่วมสอน', exact: true }).click();
	await expect(table.locator('tbody tr')).toHaveCount(1);
	await expect(teacherPicker).toContainText('ครูร่วมสอน');
	await page.getByRole('radio', { name: 'ครู', exact: true }).click();
	await expect(teacherPicker).toContainText('ครูร่วมสอน');
	await expect(page.getByRole('radio', { name: 'ครู', exact: true })).toHaveAttribute(
		'aria-checked',
		'true'
	);
	await expect(table.locator('tbody tr')).toHaveCount(1);
	await expect(page).toHaveURL(/owner=/);
	await teacherPicker.click();
	await page.getByRole('option', { name: 'ครูทั้งหมด', exact: true }).click();
	await expect(table.locator('tbody tr')).toHaveCount(3);

	await page.getByRole('radio', { name: 'ห้องเรียน', exact: true }).click();
	await expect(table.locator('tbody tr')).toHaveCount(4);
	await expect(table.locator('tbody tr td:first-child')).toHaveText([
		'ม.1/1',
		'ม.1/2',
		'ม.1/10',
		'ม.2/1'
	]);
	await expect(page.getByRole('switch', { name: 'รวมครูที่ไม่มีคาบ' })).toHaveCount(0);
	const homeroomPicker = page.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true });
	await homeroomPicker.click();
	await page.getByPlaceholder('ค้นหาห้องเรียน...').fill('ม.1/2');
	await page.getByRole('option', { name: 'ม.1/2', exact: true }).click();
	await expect(table.locator('tbody tr')).toHaveCount(1);
	await page.getByRole('radio', { name: 'ห้องเรียน', exact: true }).click();
	await expect(homeroomPicker).toContainText('ม.1/2');
	await expect(page.getByRole('radio', { name: 'ห้องเรียน', exact: true })).toHaveAttribute(
		'aria-checked',
		'true'
	);
	await page.getByLabel('ค้นหาวิชา กิจกรรม หรือกลุ่มเรียน').fill('ไม่ตรงกับวิชา');
	await expect(page.getByText('ไม่พบห้องเรียนตามตัวเลือก', { exact: true })).toBeVisible();
	await page.getByLabel('ค้นหาวิชา กิจกรรม หรือกลุ่มเรียน').fill('');
	await expect(table.locator('tbody tr')).toHaveCount(1);
	await expect(page.locator('#teaching-date')).toHaveText(initialDate);
	expect(reads).toBe(1);
	await page.reload();
	await expect(homeroomPicker).toContainText('ม.1/2');
	await expect(table.locator('tbody tr')).toHaveCount(1);
	expect(reads).toBe(2);
});

test('homeroom details deduplicate team teaching and scope activity instructors to their entries', async ({
	page
}) => {
	await installDailyOverview(page, multiOwnerOverview());
	await page.goto(`${todayUrl()}&view=homeroom`);
	const table = page.getByRole('table');
	const teamRow = table
		.locator('tbody tr')
		.filter({ has: page.locator('td:first-child').getByText('ม.1/1', { exact: true }) });
	await expect(teamRow.getByTestId('daily-lesson-card')).toHaveCount(1);
	await expect(teamRow).toContainText('ครูร่วมสอน');
	await expect(teamRow).toContainText('ห้องคณิตศาสตร์');
	await expect(teamRow.locator('[data-daily-card-code]')).toHaveText('ค21101');
	await expect(teamRow.locator('[data-daily-card-title]')).toHaveText('คณิตศาสตร์พื้นฐาน');
	await expect(teamRow.locator('[data-daily-card-teachers]')).toHaveText('ครูร่วมสอน +1');
	await expect(teamRow.locator('[data-daily-card-teachers] svg')).toHaveCount(1);
	await expect(teamRow.locator('[data-daily-card-target]')).toHaveCount(0);

	await teamRow.getByRole('button', { name: /คาบที่ 2/ }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('heading', { name: 'ม.1/1', exact: true })).toBeVisible();
	await expect(dialog.getByText('คณิตศาสตร์พื้นฐาน', { exact: true })).toHaveCount(1);
	await expect(dialog).toContainText('นายทดสอบ ตารางสอน');
	await expect(dialog).toContainText('ครูร่วมสอน');
	await expect(dialog).not.toContainText('ครูอีกห้อง');
	await expect(dialog).not.toContainText('116');
	await page.keyboard.press('Escape');

	const activityRow = table
		.locator('tbody tr')
		.filter({ has: page.locator('td:first-child').getByText('ม.1/2', { exact: true }) });
	const syncCell = activityRow.locator('td').nth(1);
	await expect(syncCell.getByTestId('daily-lesson-card')).toHaveCount(1);
	await expect(syncCell.locator('[data-daily-card-teachers]')).toHaveCount(0);
	await expect(syncCell).not.toContainText('ครูร่วมสอน');
	await expect(syncCell).not.toContainText('นายทดสอบ');

	await syncCell.getByRole('button').click();
	await expect(dialog).toContainText('นายทดสอบ ตารางสอน');
	await expect(dialog).toContainText('ครูร่วมสอน');
	await expect(dialog).not.toContainText('ครูอีกห้อง');
	await page.keyboard.press('Escape');
	const independentCell = activityRow.locator('td').nth(3);
	await expect(independentCell.getByTestId('daily-lesson-card')).toHaveCount(2);
	await expect(independentCell).toContainText('กิจกรรมแยก 1');
	await expect(independentCell).toContainText('กิจกรรมแยก 2');
	await expect(independentCell.locator('[data-daily-card-teachers]')).toHaveText([
		'ครูทดสอบ',
		'ครูทดสอบ'
	]);
	await expect(independentCell.locator('[data-daily-card-room]')).toHaveText([
		'ไม่ระบุห้อง',
		'ไม่ระบุห้อง'
	]);
	const stacking = await activityRow.evaluate((row) => {
		const cells = row.querySelectorAll('td');
		const single = cells[1].querySelector('[data-testid="daily-lesson-card"]');
		const cards = Array.from(cells[3].querySelectorAll('[data-testid="daily-lesson-card"]'));
		if (!single || cards.length !== 2) throw new Error('Missing single/stacked activity cards');
		return {
			singleTopGap: single.getBoundingClientRect().top - cells[1].getBoundingClientRect().top,
			singleBottomGap:
				cells[1].getBoundingClientRect().bottom - single.getBoundingClientRect().bottom,
			cards: cards.map((card) => {
				const lines = card.querySelectorAll(
					'[data-daily-card-title], [data-daily-card-teachers], [data-daily-card-room]'
				);
				const rect = card.getBoundingClientRect();
				return {
					top: rect.top,
					bottom: rect.bottom,
					lineBottom: Math.max(
						...Array.from(lines).map((line) => line.getBoundingClientRect().bottom)
					)
				};
			}),
			cellBottom: cells[3].getBoundingClientRect().bottom
		};
	});
	expect(stacking.singleTopGap).toBeGreaterThanOrEqual(0);
	expect(stacking.singleTopGap).toBeLessThanOrEqual(5);
	expect(stacking.singleBottomGap).toBeGreaterThanOrEqual(0);
	expect(stacking.singleBottomGap).toBeLessThanOrEqual(5);
	expect(stacking.cards[0].bottom).toBeLessThanOrEqual(stacking.cards[1].top);
	for (const card of stacking.cards) {
		expect(card.lineBottom).toBeLessThanOrEqual(card.bottom);
		expect(card.bottom).toBeLessThanOrEqual(stacking.cellBottom);
	}

	await expect(table).not.toContainText('กิจกรรมไม่ระบุห้อง');
	await page.getByRole('radio', { name: 'ครู', exact: true }).click();
	await expect(table).toContainText('กิจกรรมไม่ระบุห้อง');
});

test('an owner that disappears on a date change stays explicitly unavailable', async ({ page }) => {
	await installTimetableMock(page, { periodCount: 10 });
	let reads = 0;
	await page.route('**/api/academic/timetable/daily-teaching**', (route) => {
		reads += 1;
		const data = reads === 1 ? multiOwnerOverview() : overview();
		data.date = new URL(route.request().url()).searchParams.get('date') ?? overviewDate;
		return fulfill(route, data);
	});
	await page.goto(`${todayUrl()}&view=homeroom&owner=${encodeURIComponent('ม.1/10')}`);
	const picker = page.getByRole('combobox', { name: 'เลือกห้องเรียน', exact: true });
	await expect(picker).toContainText('ม.1/10');
	// The next fixture has 18 synchronized rooms, so select a teacher that actually disappears.
	await page.getByRole('radio', { name: 'ครู', exact: true }).click();
	await page.getByRole('combobox', { name: 'เลือกครู', exact: true }).click();
	await page.getByRole('option', { name: 'ครูร่วมสอน', exact: true }).click();
	await page.getByRole('button', { name: 'วันถัดไป' }).click();
	await expect(page.getByRole('combobox', { name: 'เลือกครู', exact: true })).toContainText(
		'ไม่พบครูที่เลือกในวันนี้'
	);
	await expect(page.getByText('ไม่พบครูตามตัวเลือก', { exact: true })).toBeVisible();
	await expect(page.getByRole('table')).toHaveCount(0);
	await page.getByRole('combobox', { name: 'เลือกครู', exact: true }).click();
	await page.getByRole('option', { name: 'ครูทั้งหมด', exact: true }).click();
	await expect(page.getByRole('table').locator('tbody tr')).toHaveCount(1);
	expect(reads).toBe(2);
});

const structuralTitle = 'กิจกรรมหน้าเสาธงและส่งเสริมคุณธรรมประจำวัน';

function structuralEntry(
	index: number,
	overrides: Partial<DailyTeachingEntry> = {}
): DailyTeachingEntry {
	return {
		...activityEntry(100 + index, ['ม.1/1']),
		entryType: 'structural',
		activityId: null,
		activitySchedulingMode: null,
		activityVersionDisplayLabel: null,
		offeringId: null,
		offeringCode: null,
		offeringName: null,
		title: structuralTitle,
		roomId: timetableIds.room,
		roomName: 'ลานกิจกรรม',
		roomCode: 'INTERNAL-ASSEMBLY',
		note: 'หมายเหตุร่วม',
		...overrides
	};
}

function referenceOverview(): DailyTeachingOverview {
	const data = overview();
	data.teachers[0].periods[1].entries = [structuralEntry(0, { roomId: null, roomName: null })];
	return data;
}

function structuralAllocationOverview(): DailyTeachingOverview {
	const data = overview();
	data.teachers = Array.from({ length: 24 }, (_, index) => ({
		id: `91000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
		displayName: `นายสมมติ ${index + 1}`,
		periods: data.periods.map((period) => ({
			bellSchedulePeriodId: period.id,
			entries: period.orderIndex === 1 ? [structuralEntry(index)] : []
		}))
	}));
	data.teachers[0].periods[1].entries = [
		structuralEntry(30),
		structuralEntry(31, { title: 'ประชุมครูประจำวัน' }),
		structuralEntry(32, { note: 'หมายเหตุอีกบริบท' }),
		structuralEntry(33, { roomId: '93000000-0000-4000-8000-000000000001', roomName: 'หอประชุม' })
	];
	data.teachers[0].periods[2].entries = [
		courseEntry(),
		{ ...courseEntry(), entryId: '94000000-0000-4000-8000-000000000001' }
	];
	data.summary = {
		totalTeacherCount: 24,
		displayedTeacherCount: 24,
		teachersTeachingCount: 24,
		emptyTeacherCount: 0,
		lessonCount: 30
	};
	return data;
}

test('homeroom structural cards collapse repeated allocations without losing detail evidence or distinct contexts', async ({
	page
}) => {
	await installDailyOverview(page, structuralAllocationOverview());
	await page.goto(`${todayUrl()}&view=homeroom`);
	const row = page.getByRole('table').locator('tbody tr');
	await expect(row).toHaveCount(1);
	const sharedCell = row.locator('td').nth(1);
	await expect(sharedCell.getByTestId('daily-lesson-card')).toHaveCount(1);
	await expect(sharedCell.locator('[data-daily-card-title]')).toHaveText(structuralTitle);
	await expect(
		sharedCell.locator('[data-daily-card-code], [data-daily-card-teachers]')
	).toHaveCount(0);
	await expect(sharedCell.locator('[data-daily-card-room]')).toHaveText('ลานกิจกรรม');
	await expect(sharedCell).not.toContainText('นายสมมติ');
	await expect(sharedCell).not.toContainText('INTERNAL-ASSEMBLY');
	await sharedCell.getByRole('button').click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('[data-daily-entry-id]')).toHaveCount(24);
	await expect(dialog).toContainText('นายสมมติ 1');
	await expect(dialog).toContainText('นายสมมติ 24');
	await expect(dialog).toContainText('หมายเหตุร่วม');
	await expect(dialog).not.toContainText('INTERNAL-ASSEMBLY');
	await page.keyboard.press('Escape');

	const distinctCell = row.locator('td').nth(2);
	await expect(distinctCell.getByTestId('daily-lesson-card')).toHaveCount(4);
	await expect(distinctCell.locator('[data-daily-card-title]')).toHaveText([
		structuralTitle,
		'ประชุมครูประจำวัน',
		structuralTitle,
		structuralTitle
	]);
	await expect(distinctCell.locator('[data-daily-card-room]')).toHaveText([
		'ลานกิจกรรม',
		'ลานกิจกรรม',
		'ลานกิจกรรม',
		'หอประชุม'
	]);
	await expect(distinctCell.locator('[data-daily-card-teachers]')).toHaveCount(0);
	await distinctCell.getByRole('button').click();
	await expect(dialog.locator('[data-daily-entry-id]')).toHaveCount(4);
	await expect(dialog).toContainText('หมายเหตุอีกบริบท');
	await page.keyboard.press('Escape');
	const courses = row.locator('td').nth(3);
	await expect(courses.getByTestId('daily-lesson-card')).toHaveCount(2);
	await expect(courses.locator('[data-daily-card-title]')).toHaveText([
		'คณิตศาสตร์พื้นฐาน',
		'คณิตศาสตร์พื้นฐาน'
	]);
});

for (const width of [390, 1280]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`keeps neutral compact teacher and homeroom grids contained at ${width}px in ${theme}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 1080 });
			await page.addInitScript(
				(mode) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: mode })),
				theme
			);
			await installDailyOverview(page, referenceOverview());
			await page.goto(todayUrl());
			await expect
				.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
				.toBe(theme === 'dark');
			for (const view of ['ครู', 'ห้องเรียน']) {
				await page.getByRole('radio', { name: view, exact: true }).click();
				const table = page.getByRole('table');
				await expect(table).toBeVisible();
				if (width < 640) {
					const searchGeometry = await page.getByTestId('daily-search-field').evaluate((field) => {
						const input = field.querySelector('input');
						const label = field.querySelector('label');
						const toolbar = field.parentElement;
						const actions = toolbar?.querySelector('[data-testid="daily-toolbar-actions"]');
						if (!input || !label || !toolbar || !actions) {
							throw new Error('Daily timetable search or action controls are missing');
						}
						const toolbarStyle = getComputedStyle(toolbar);
						const fieldRect = field.getBoundingClientRect();
						const inputRect = input.getBoundingClientRect();
						return {
							availableWidth:
								toolbar.clientWidth -
								parseFloat(toolbarStyle.paddingLeft) -
								parseFloat(toolbarStyle.paddingRight),
							fieldWidth: fieldRect.width,
							inputWidth: inputRect.width,
							labelHeight: label.getBoundingClientRect().height,
							labelLineHeight: parseFloat(getComputedStyle(label).lineHeight),
							fieldBottom: fieldRect.bottom,
							inputBottom: inputRect.bottom,
							actionsTop: actions.getBoundingClientRect().top
						};
					});
					expect(
						Math.abs(searchGeometry.fieldWidth - searchGeometry.availableWidth)
					).toBeLessThanOrEqual(1);
					expect(searchGeometry.inputWidth).toBeGreaterThanOrEqual(
						searchGeometry.availableWidth - 1
					);
					expect(searchGeometry.labelHeight).toBeLessThanOrEqual(
						searchGeometry.labelLineHeight * 2 + 1
					);
					expect(searchGeometry.fieldBottom).toBeLessThanOrEqual(searchGeometry.actionsTop);
					expect(searchGeometry.inputBottom).toBeLessThanOrEqual(searchGeometry.actionsTop);
				}
				const geometry = await table.evaluate((element) => {
					const scroller = element.closest('.daily-teaching-scroll');
					if (!scroller) throw new Error('Missing daily timetable scroller');
					const headers = Array.from(element.querySelectorAll('thead th')).slice(1);
					const cardStyles = Array.from(
						element.querySelectorAll('[data-testid="daily-lesson-card"]')
					).map((card) => getComputedStyle(card).backgroundColor);
					const firstOwner = element.querySelector('tbody td');
					return {
						documentOverflow: document.documentElement.scrollWidth - window.innerWidth,
						scrollerRight: scroller.getBoundingClientRect().right,
						periodWidths: headers.map((header) => header.getBoundingClientRect().width),
						cardStyles,
						background: firstOwner ? getComputedStyle(firstOwner).backgroundColor : '',
						cardLayouts: Array.from(
							element.querySelectorAll('[data-testid="daily-lesson-card"]')
						).map((card) => {
							const cell = card.closest('td');
							if (!cell) throw new Error('Card has no daily period cell');
							const cardRect = card.getBoundingClientRect();
							return {
								topGap: cardRect.top - cell.getBoundingClientRect().top,
								bottomGap: cell.getBoundingClientRect().bottom - cardRect.bottom,
								lines: Array.from(
									card.querySelectorAll(
										'[data-daily-card-code], [data-daily-card-title], [data-daily-card-target], [data-daily-card-teachers], [data-daily-card-room]'
									)
								).map((line) => ({
									bottom: line.getBoundingClientRect().bottom,
									fontSize: parseFloat(getComputedStyle(line).fontSize)
								})),
								bottom: cardRect.bottom
							};
						}),
						maxRowHeight: Math.max(
							...Array.from(element.querySelectorAll('tbody tr')).map(
								(row) => row.getBoundingClientRect().height
							)
						)
					};
				});
				expect(geometry.documentOverflow).toBeLessThanOrEqual(1);
				expect(geometry.scrollerRight).toBeLessThanOrEqual(width + 1);
				expect(
					Math.max(...geometry.periodWidths) - Math.min(...geometry.periodWidths)
				).toBeLessThanOrEqual(1);
				expect(geometry.maxRowHeight).toBeGreaterThanOrEqual(88);
				expect(geometry.maxRowHeight).toBeLessThanOrEqual(104);
				expect(geometry.cardStyles.length).toBeGreaterThan(0);
				for (const background of geometry.cardStyles) expect(background).toBe(geometry.background);
				for (const card of geometry.cardLayouts) {
					expect(card.topGap).toBeGreaterThanOrEqual(0);
					expect(card.topGap).toBeLessThanOrEqual(5);
					expect(card.bottomGap).toBeGreaterThanOrEqual(0);
					expect(card.bottomGap).toBeLessThanOrEqual(5);
					for (const line of card.lines) {
						expect(line.bottom).toBeLessThanOrEqual(card.bottom);
						expect(line.fontSize).toBeGreaterThanOrEqual(9);
						expect(line.fontSize).toBeLessThanOrEqual(11);
					}
				}

				await page.screenshot({
					path: testInfo.outputPath(`${view}-${width}-${theme}.png`),
					fullPage: true
				});
			}
		});
	}
}

test('today-only reader sees daily teaching without planner authority', async ({ page }) => {
	const api = await installDailyOverview(page, overview(), [
		PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL
	]);
	await page.goto(todayUrl());
	await expect(page.getByRole('table')).toBeVisible();
	expect(api.count()).toBe(1);
	await expect(page.getByRole('link', { name: 'จัดตาราง', exact: true })).toHaveCount(0);
});

for (const permission of [
	PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
	PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
	PERMISSIONS.ACADEMIC_TIMETABLE_PUBLISH_SCHOOL
]) {
	test(`planner-only ${permission} cannot navigate or preload the daily timetable`, async ({
		page
	}) => {
		const api = await installDailyOverview(page, overview(), [permission]);
		await page.goto(todayUrl());
		await expect(page).toHaveURL(/\/403/);
		await expect(page.getByRole('table')).toHaveCount(0);
		await expect(page.getByTestId('daily-toolbar-actions')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'พิมพ์', exact: true })).toHaveCount(0);
		expect(api.count()).toBe(0);
		const status = await page.evaluate(async (href) => {
			// This is the installed public $app/navigation entry used by the Vite app.
			const modulePath = '/node_modules/@sveltejs/kit/src/runtime/app/navigation/client.js';
			const { preloadData } = await import(modulePath);
			return (await preloadData(href)).status;
		}, `${todayUrl()}&preloadProbe=1`);
		expect(status).toBeLessThan(500);
		expect(api.count()).toBe(0);
	});
}

test('daily reader with planner authority keeps the exact planner context link', async ({
	page
}) => {
	await installDailyOverview(page, overview(), [
		PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
		PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL
	]);
	await page.goto(todayUrl());
	await expect(page.getByRole('table')).toBeVisible();
	await expect(page.getByRole('link', { name: 'จัดตาราง', exact: true })).toHaveAttribute(
		'href',
		`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}`
	);
});

test('revoking daily access removes toolbar and discards a delayed date response', async ({
	page
}) => {
	const fixture = await installTimetableMock(page, {
		permissions: [
			PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL
		]
	});
	let reads = 0;
	let release = () => {};
	let complete = () => {};
	const completed = new Promise<void>((resolve) => {
		complete = resolve;
	});
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route('**/api/academic/timetable/daily-teaching**', async (route) => {
		reads += 1;
		if (reads > 1) await gate;
		await fulfill(route, overview());
		if (reads > 1) complete();
	});
	await page.goto(todayUrl());
	await expect(page.getByRole('table')).toBeVisible();
	await page.getByRole('button', { name: 'วันถัดไป', exact: true }).click();
	await expect.poll(() => reads).toBe(2);
	fixture.setPermissions([PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL]);
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/permissions.ts';
		const { setPermissions } = await import(modulePath);
		setPermissions(['academic_timetable.read.school']);
	});
	await expect(page.getByTestId('daily-toolbar-actions')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'พิมพ์', exact: true })).toHaveCount(0);
	release();
	await completed;
	await expect(page.getByRole('table')).toHaveCount(0);
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(reads).toBe(2);
});

test('revoking daily access closes an opened period detail', async ({ page }) => {
	const api = await installDailyOverview(page, overview(), [
		PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
		PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL
	]);
	await page.goto(todayUrl());
	await page.getByRole('table').locator('tbody button').first().click();
	await expect(page.getByRole('dialog')).toBeVisible();
	api.setPermissions([PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL]);
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/permissions.ts';
		const { setPermissions } = await import(modulePath);
		setPermissions(['academic_timetable.read.school']);
	});
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByTestId('daily-toolbar-actions')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'พิมพ์', exact: true })).toHaveCount(0);
	expect(api.count()).toBe(1);
});
