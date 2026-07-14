import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	CALENDAR_WEEKDAY_LABELS,
	buildCalendarMonth,
	buildCalendarMonthWeeks,
	calendarGridRange,
	eventOverlapsDate,
	formatCalendarDate,
	formatCalendarMonth,
	monthRange
} from '../../src/lib/utils/calendar.ts';

describe('calendar helpers', () => {
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
});
