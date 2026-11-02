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
		roomId: timetableIds.room,
		roomName: 'ห้องคณิตศาสตร์',
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

async function installDailyOverview(page: Page, data = overview()): Promise<void> {
	await installTimetableMock(page, { periodCount: 10 });
	await page.route('**/api/academic/timetable/daily-teaching**', (route) => fulfill(route, data));
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
	await expect(courseCell.getByText('ม.1/1 · ห้องคณิตศาสตร์', { exact: true })).toBeVisible();
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

function multiOwnerOverview() {
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
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	await expect(teacherPicker).toContainText('ครูร่วมสอน');
	await expect(table.locator('tbody tr')).toHaveCount(1);
	await expect(page).toHaveURL(/owner=/);
	await teacherPicker.click();
	await page.getByRole('option', { name: 'ครูทั้งหมด', exact: true }).click();
	await expect(table.locator('tbody tr')).toHaveCount(3);

	await page.getByRole('button', { name: 'ห้องเรียน', exact: true }).click();
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
	await page.getByRole('button', { name: 'ห้องเรียน', exact: true }).click();
	await expect(homeroomPicker).toContainText('ม.1/2');
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
	await syncCell.getByRole('button').click();
	await expect(dialog).toContainText('นายทดสอบ ตารางสอน');
	await expect(dialog).toContainText('ครูร่วมสอน');
	await expect(dialog).not.toContainText('ครูอีกห้อง');
	await page.keyboard.press('Escape');
	const independentCell = activityRow.locator('td').nth(3);
	await expect(independentCell.getByTestId('daily-lesson-card')).toHaveCount(2);
	await expect(independentCell).toContainText('กิจกรรมแยก 1');
	await expect(independentCell).toContainText('กิจกรรมแยก 2');
	await expect(table).not.toContainText('กิจกรรมไม่ระบุห้อง');
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
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
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
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
			await installDailyOverview(page);
			await page.goto(todayUrl());
			await expect
				.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
				.toBe(theme === 'dark');
			for (const view of ['ครู', 'ห้องเรียน']) {
				await page.getByRole('button', { name: view, exact: true }).click();
				const table = page.getByRole('table');
				await expect(table).toBeVisible();
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
				expect(geometry.maxRowHeight).toBeLessThanOrEqual(64);
				expect(geometry.cardStyles.length).toBeGreaterThan(0);
				for (const background of geometry.cardStyles) expect(background).toBe(geometry.background);
				await page.screenshot({
					path: testInfo.outputPath(`${view}-${width}-${theme}.png`),
					fullPage: true
				});
			}
		});
	}
}
