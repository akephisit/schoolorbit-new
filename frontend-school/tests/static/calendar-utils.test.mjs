import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	buildCalendarMonth,
	eventOverlapsDate,
	formatCalendarDate,
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

	it('uses Monday-to-Sunday grid boundaries for a month', () => {
		const cells = buildCalendarMonth('2026-07-01');
		assert.equal(cells[0]?.date, '2026-06-29');
		assert.equal(cells[41]?.date, '2026-08-09');
		assert.equal(cells[0]?.inCurrentMonth, false);
		assert.equal(cells[2]?.date, '2026-07-01');
		assert.equal(cells[2]?.inCurrentMonth, true);
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

	it('formats dates with Thai month labels', () => {
		assert.equal(formatCalendarDate('2026-07-03'), '3 ก.ค. 2026');
	});
});
