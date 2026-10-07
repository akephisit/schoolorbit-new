import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	CALENDAR_WEEKDAY_LABELS,
	buildCalendarEmbedCode,
	buildCalendarEmbedUrl,
	buildCalendarMonth,
	buildCalendarMonthWeeks,
	buildCalendarColorKey,
	calendarGridRange,
	eventOverlapsDate,
	formatCalendarDate,
	formatCalendarMonth,
	monthRange,
	normalizeCalendarTime
} from '../../src/lib/utils/calendar.ts';

describe('calendar helpers', () => {
	it('normalizes 24-hour clock input without guessing invalid times', () => {
		for (const [input, expected] of [
			['830', '08:30'],
			['0830', '08:30'],
			['8:30', '08:30'],
			['13:05', '13:05'],
			['0000', '00:00'],
			['2359', '23:59'],
			[' 0900 ', '09:00']
		]) {
			assert.equal(normalizeCalendarTime(input), expected);
		}
		for (const input of ['', '8', '08', '24:00', '2360', '8:3', '08:30 AM', '-1:00', '12345']) {
			assert.equal(normalizeCalendarTime(input), null);
		}
	});
	it('keeps timed entries within each day while all-day and pending spans continue across weeks', () => {
		const shared = { title: 'กิจกรรม', startDate: '2026-07-03', endDate: '2026-07-05' };
		const weeks = buildCalendarMonthWeeks('2026-07-01', [
			{ ...shared, id: 'all-day', allDay: true },
			{ ...shared, id: 'timed', allDay: false, startTime: '08:30' },
			{ ...shared, id: 'pending', allDay: false, pending: true, startTime: '10:00' }
		]);
		const segments = weeks.flatMap((week) => week.segments);
		const timed = segments.filter((segment) => segment.event.id === 'timed');
		assert.equal(timed.length, 3);
		assert.ok(
			timed.every(
				(segment) =>
					segment.span === 1 && !segment.continuesFromPreviousWeek && !segment.continuesIntoNextWeek
			)
		);
		assert.ok(segments.some((segment) => segment.event.id === 'all-day' && segment.span === 2));
		assert.ok(
			segments.some(
				(segment) => segment.event.id === 'pending' && segment.continuesFromPreviousWeek
			)
		);
		assert.ok(weeks.every((week) => week.hiddenEventCounts.every((count) => count === 0)));
	});
	it('builds a 42-cell month grid', () => {
		const cells = buildCalendarMonth('2026-07-01');
		assert.equal(cells.length, 42);
		assert.equal(
			cells.some((cell) => cell.date === '2026-07-01'),
			true
		);
	});

	it('uses Sunday-to-Saturday grid boundaries for a month', () => {
		const cells = buildCalendarMonth('2026-07-01');
		assert.equal(cells[0]?.date, '2026-06-28');
		assert.equal(cells[41]?.date, '2026-08-08');
		assert.equal(cells[0]?.inCurrentMonth, false);
		assert.equal(cells[3]?.date, '2026-07-01');
		assert.equal(cells[3]?.inCurrentMonth, true);
		assert.deepEqual(CALENDAR_WEEKDAY_LABELS, ['อา', 'จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส']);
	});

	it('detects multi-day event overlap', () => {
		assert.equal(
			eventOverlapsDate({ startDate: '2026-07-03', endDate: '2026-07-05' }, '2026-07-04'),
			true
		);
		assert.equal(
			eventOverlapsDate({ startDate: '2026-07-03', endDate: '2026-07-05' }, '2026-07-06'),
			false
		);
	});

	it('returns the inclusive month date range', () => {
		assert.deepEqual(monthRange('2026-07-15'), {
			from: '2026-07-01',
			to: '2026-07-31'
		});
	});

	it('returns the full visible grid range for loading adjacent-month events', () => {
		assert.deepEqual(calendarGridRange('2026-07-15'), {
			from: '2026-06-28',
			to: '2026-08-08'
		});
	});

	it('formats dates with Thai month labels and Buddhist years', () => {
		assert.equal(formatCalendarDate('2026-07-03'), '3 ก.ค. 2569');
		assert.equal(formatCalendarMonth('2026-07-03'), 'กรกฎาคม 2569');
	});

	it('builds a tenant-local calendar embed URL', () => {
		assert.equal(
			buildCalendarEmbedUrl('https://snwsb.schoolorbit.app'),
			'https://snwsb.schoolorbit.app/calendar/embed'
		);
		assert.equal(
			buildCalendarEmbedUrl('https://snwsb.schoolorbit.app/'),
			'https://snwsb.schoolorbit.app/calendar/embed'
		);
	});

	it('builds a WordPress-safe calendar iframe snippet', () => {
		const code = buildCalendarEmbedCode('https://snwsb.schoolorbit.app');

		assert.match(code, /src="https:\/\/snwsb\.schoolorbit\.app\/calendar\/embed"/);
		assert.match(code, /title="ปฏิทินโรงเรียน"/);
		assert.match(code, /width="100%"/);
		assert.match(code, /height="760"/);
		assert.match(code, /loading="lazy"/);
		assert.match(code, /sandbox="allow-scripts allow-same-origin"/);
		assert.match(code, /referrerpolicy="strict-origin-when-cross-origin"/);
		assert.match(code, /style="border:0;border-radius:12px"/);
	});

	it('splits a multi-day event into continuous weekly segments', () => {
		const weeks = buildCalendarMonthWeeks('2026-07-01', [
			{
				id: 'event-1',
				title: 'ค่ายวิชาการ',
				startDate: '2026-07-03',
				endDate: '2026-07-10',
				allDay: true
			}
		]);

		assert.equal(weeks.length, 6);
		assert.deepEqual(weeks[0]?.segments[0], {
			event: {
				id: 'event-1',
				title: 'ค่ายวิชาการ',
				startDate: '2026-07-03',
				endDate: '2026-07-10',
				allDay: true
			},
			startColumn: 5,
			span: 2,
			lane: 0,
			continuesFromPreviousWeek: false,
			continuesIntoNextWeek: true
		});
		assert.deepEqual(weeks[1]?.segments[0], {
			event: {
				id: 'event-1',
				title: 'ค่ายวิชาการ',
				startDate: '2026-07-03',
				endDate: '2026-07-10',
				allDay: true
			},
			startColumn: 0,
			span: 6,
			lane: 0,
			continuesFromPreviousWeek: true,
			continuesIntoNextWeek: false
		});
	});

	it('places a new all-day bar above timed entries continued from the previous week', () => {
		const weeks = buildCalendarMonthWeeks('2026-07-01', [
			{
				id: 'timed',
				title: 'อบรม',
				startDate: '2026-07-03',
				endDate: '2026-07-06',
				allDay: false,
				startTime: '08:30'
			},
			{
				id: 'all-day',
				title: 'ทั้งวัน',
				startDate: '2026-07-05',
				endDate: '2026-07-06',
				allDay: true
			}
		]);
		const nextWeek = weeks[1].segments;
		const bar = nextWeek.find((segment) => segment.event.id === 'all-day');
		assert.ok(bar);
		assert.ok(
			nextWeek
				.filter((segment) => segment.event.id === 'timed')
				.every((segment) => segment.lane > bar.lane)
		);
	});
	it('counts events hidden when all visible lanes are occupied', () => {
		const [firstWeek] = buildCalendarMonthWeeks(
			'2026-07-01',
			[
				{
					id: 'event-1',
					title: 'กิจกรรมต่อเนื่อง',
					startDate: '2026-07-01',
					endDate: '2026-07-03'
				},
				{
					id: 'event-2',
					title: 'กิจกรรมซ้อน',
					startDate: '2026-07-02',
					endDate: '2026-07-02'
				}
			],
			1
		);

		assert.deepEqual(firstWeek?.hiddenEventCounts, [0, 0, 0, 0, 1, 0, 0]);
	});

	it('builds a selected-month color key without adjacent-month-only events', () => {
		const items = buildCalendarColorKey('2026-07-15', [
			{
				id: 'june',
				startDate: '2026-06-30',
				endDate: '2026-06-30',
				categoryId: 'internal',
				categoryName: 'ภายใน',
				categoryColor: '#111827'
			},
			{
				id: 'spanning',
				startDate: '2026-06-29',
				endDate: '2026-07-02',
				categoryId: 'camp',
				categoryName: 'ค่าย',
				categoryColor: '#7c3aed'
			},
			{
				id: 'july',
				startDate: '2026-07-20',
				endDate: '2026-07-20',
				categoryId: 'academic',
				categoryName: 'วิชาการ',
				categoryColor: '#0284c7'
			}
		]);

		assert.deepEqual(items, [
			{ id: 'camp', name: 'ค่าย', color: '#7c3aed' },
			{ id: 'academic', name: 'วิชาการ', color: '#0284c7' }
		]);
	});

	it('deduplicates categories and places the uncategorized fallback last', () => {
		const items = buildCalendarColorKey('2026-07-01', [
			{
				id: 'academic-1',
				startDate: '2026-07-01',
				endDate: '2026-07-01',
				categoryId: 'academic',
				categoryName: 'วิชาการ',
				categoryColor: '#0284c7'
			},
			{
				id: 'academic-2',
				startDate: '2026-07-02',
				endDate: '2026-07-02',
				categoryId: 'academic',
				categoryName: 'วิชาการ',
				categoryColor: '#0284c7'
			},
			{
				id: 'meeting',
				startDate: '2026-07-03',
				endDate: '2026-07-03',
				categoryId: 'meeting',
				categoryName: 'ประชุม',
				categoryColor: '#16a34a'
			},
			{
				id: 'uncategorized',
				startDate: '2026-07-04',
				endDate: '2026-07-04',
				categoryId: null,
				categoryName: null,
				categoryColor: null
			}
		]);

		assert.deepEqual(items, [
			{ id: 'meeting', name: 'ประชุม', color: '#16a34a' },
			{ id: 'academic', name: 'วิชาการ', color: '#0284c7' },
			{ id: 'uncategorized', name: 'ไม่ระบุหมวดหมู่', color: '#64748b' }
		]);
	});
});
