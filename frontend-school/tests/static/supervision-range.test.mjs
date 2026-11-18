import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(
	new URL('../../src/lib/supervision/booking-range.ts', import.meta.url),
	'utf8'
);
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 }
}).outputText;
const {
	bookingRangeError,
	adjacentBookingOptions,
	orderedBookingRange,
	bookedObservationFor,
	bookingWeekStart,
	addCivilDays,
	cycleDateRangeError,
	withinCycleDates,
	clampedBookingWeek,
	bookingOptionUnavailable,
	bookingAvailabilityLabel,
	reconcileBookingRange
} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
const lesson = (index, patch = {}) => ({
	blockGroupId: `lesson-${index}`,
	observedDate: '2026-11-16',
	homeroomId: 'homeroom-3-2',
	homeroomName: 'ม.3/2',
	bookingAvailability: { pendingCount: 0, confirmedCount: 0, blocked: false },
	startTime: `${String(8 + index).padStart(2, '0')}:00:00`,
	endTime: `${String(9 + index).padStart(2, '0')}:00:00`,
	...patch
});

test('booking permits one, two, three and arbitrarily many consecutive periods of the exact same homeroom', () => {
	for (const count of [1, 2, 3, 12]) {
		const lessons = Array.from({ length: count }, (_, index) =>
			lesson(index, { subjectName: index % 2 ? 'คณิตศาสตร์' : 'ภาษาไทย' })
		);
		assert.equal(bookingRangeError(lessons), null);
		assert.deepEqual(
			orderedBookingRange([...lessons].reverse()).map((item) => item.blockGroupId),
			lessons.map((item) => item.blockGroupId)
		);
	}
});

test('labels and grade never substitute for an exact homeroom UUID', () => {
	assert.match(
		bookingRangeError([
			lesson(0),
			lesson(1, { homeroomId: 'homeroom-3-1', homeroomName: 'ม.3/1' })
		]),
		/ห้องประจำชั้นเดียวกัน/
	);
	assert.match(
		bookingRangeError([
			lesson(0),
			lesson(1, { homeroomId: 'different-uuid', homeroomName: 'ม.3/2' })
		]),
		/ห้องประจำชั้นเดียวกัน/
	);
	assert.match(bookingRangeError([lesson(0, { homeroomId: null })]), /ไม่มีห้องประจำชั้น/);
});

test('duplicate, cross-date, overlap, skipped free period and lunch gap cannot form one booking', () => {
	assert.match(bookingRangeError([lesson(0), lesson(0)]), /ซ้ำ/);
	assert.match(
		bookingRangeError([lesson(0), lesson(1, { observedDate: '2026-11-17' })]),
		/วันเดียวกัน/
	);
	for (const second of [
		lesson(2),
		lesson(1, { startTime: '08:30:00' }),
		lesson(1, { startTime: '09:30:00', endTime: '10:30:00' })
	]) {
		assert.match(bookingRangeError([lesson(0), second]), /ต่อเนื่อง/);
	}
	assert.match(bookingRangeError([lesson(0, { endTime: '08:00:00' })]), /เวลา/);
});

test('only eligible edges can be extended, and removing either edge leaves a valid range', () => {
	const items = Array.from({ length: 6 }, (_, index) => lesson(index));
	const selected = items.slice(2, 4);
	assert.deepEqual(
		adjacentBookingOptions(items, selected).map((item) => item.blockGroupId),
		['lesson-1', 'lesson-4']
	);
	assert.equal(bookingRangeError(selected.slice(1)), null);
	assert.equal(bookingRangeError(selected.slice(0, -1)), null);
	assert.deepEqual(
		adjacentBookingOptions(
			[lesson(1, { homeroomId: 'other' }), lesson(1, { observedDate: '2026-11-17' })],
			[lesson(0)]
		),
		[]
	);
});

test('booked-cell detection inspects later saved periods and excludes cancelled or edited observations', () => {
	const item = lesson(1);
	const observation = {
		id: 'saved',
		status: 'planned',
		periods: [
			{ timetableBlockGroupId: 'unrelated', observedAt: '2026-11-16T01:00:00Z' },
			{ timetableBlockGroupId: item.blockGroupId, observedAt: '2026-11-16T02:00:00Z' }
		]
	};
	assert.equal(bookedObservationFor(item, [observation]), observation);
	assert.equal(bookedObservationFor(item, [observation], 'saved'), undefined);
	assert.equal(bookedObservationFor(item, [{ ...observation, status: 'cancelled' }]), undefined);
	assert.equal(
		bookedObservationFor({ ...item, observedDate: '2026-11-17' }, [observation]),
		undefined
	);
});

test('week navigation uses civil dates across month/year and leap-day boundaries', () => {
	assert.equal(bookingWeekStart('2027-01-01'), '2026-12-28');
	assert.equal(addCivilDays('2028-02-28', 1), '2028-02-29');
});

test('booked intervals overlap different source IDs within the supplied cycle scope, while touching boundaries remain available', () => {
	const item = lesson(1);
	const saved = (observedAt, endsAt) => ({
		id: 'saved-in-cycle',
		status: 'planned',
		periods: [{ timetableBlockGroupId: 'different-source', observedAt, endsAt }]
	});
	assert.ok(bookedObservationFor(item, [saved('2026-11-16T01:30:00Z', '2026-11-16T02:30:00Z')]));
	assert.ok(bookedObservationFor(item, [saved('2026-11-16T02:30:00Z', null)]));
	assert.equal(
		bookedObservationFor(item, [saved('2026-11-16T01:00:00Z', '2026-11-16T02:00:00Z')]),
		undefined
	);
	assert.equal(bookedObservationFor(item, [saved('2026-11-16T03:00:00Z', null)]), undefined);
});

test('cycle dates require a complete real ordered pair and include both endpoints', () => {
	for (const pair of [
		[null, null],
		['2026-11-16', null],
		['2026-02-30', '2026-03-01'],
		['2026-11-18', '2026-11-16']
	])
		assert.ok(cycleDateRangeError(...pair));
	assert.equal(cycleDateRangeError('2026-11-16', '2026-11-16'), null);
	for (const day of ['2026-11-16', '2026-11-17', '2026-11-18'])
		assert.equal(withinCycleDates(day, '2026-11-16', '2026-11-18'), true);
	for (const day of ['2026-11-15', '2026-11-19'])
		assert.equal(withinCycleDates(day, '2026-11-16', '2026-11-18'), false);
	assert.equal(withinCycleDates('2026-11-16', null, null), false);
});

test('week defaults clamp to first and last available weeks, including partial weeks across years', () => {
	assert.equal(clampedBookingWeek('2026-12-30', '2027-01-06', '2026-10-01'), '2026-12-28');
	assert.equal(clampedBookingWeek('2026-12-30', '2027-01-06', '2027-01-10'), '2027-01-04');
	assert.equal(clampedBookingWeek('2026-11-18', '2026-11-25', '2026-11-23'), '2026-11-23');
});

test('pending and parallel confirmed bookings remain eligible while authoritative blocked and outside dates are refused', () => {
	const bounds = ['2026-11-16', '2026-11-18'];
	for (const availability of [
		{ pendingCount: 2, confirmedCount: 0, blocked: false },
		{ pendingCount: 0, confirmedCount: 2, blocked: false }
	])
		assert.equal(
			bookingOptionUnavailable(lesson(0, { bookingAvailability: availability }), ...bounds),
			null
		);
	assert.match(
		bookingOptionUnavailable(
			lesson(0, { bookingAvailability: { pendingCount: 0, confirmedCount: 1, blocked: true } }),
			...bounds
		),
		/ทับกัน/
	);
	assert.match(
		bookingOptionUnavailable(lesson(0, { observedDate: '2026-11-19' }), ...bounds),
		/นอกช่วง/
	);
	assert.equal(bookingAvailabilityLabel(lesson(0)), 'ว่าง');
	assert.equal(
		bookingAvailabilityLabel(
			lesson(0, { bookingAvailability: { pendingCount: 2, confirmedCount: 3, blocked: true } })
		),
		'รออนุมัติ 2 · นัดหมาย 3 · จองไม่ได้'
	);
	assert.match(
		bookingAvailabilityLabel(
			lesson(0, { bookingAvailability: { pendingCount: 2, confirmedCount: 0, blocked: false } })
		),
		/รออนุมัติ 2/
	);
	assert.match(
		bookingAvailabilityLabel(
			lesson(0, { bookingAvailability: { pendingCount: 0, confirmedCount: 2, blocked: false } })
		),
		/จองขนานได้/
	);
});

test('refresh preserves an unchanged eligible range but drops blocked, missing, moved, or newly outside selections', () => {
	const selected = [lesson(0), lesson(1), lesson(2)];
	const bounds = ['2026-11-16', '2026-11-18'];
	assert.deepEqual(
		reconcileBookingRange(
			selected,
			selected.map((item) => ({
				...item,
				bookingAvailability: { pendingCount: 3, confirmedCount: 1, blocked: false }
			})),
			...bounds
		).map((item) => item.blockGroupId),
		selected.map((item) => item.blockGroupId)
	);
	assert.deepEqual(
		reconcileBookingRange(
			selected,
			[
				lesson(0),
				lesson(1, { bookingAvailability: { pendingCount: 0, confirmedCount: 1, blocked: true } }),
				lesson(2)
			],
			...bounds
		),
		[]
	);
	assert.deepEqual(reconcileBookingRange(selected, selected.slice(1), ...bounds), []);
	assert.deepEqual(
		reconcileBookingRange(
			selected,
			[lesson(0), lesson(1, { startTime: '09:30:00' }), lesson(2)],
			...bounds
		),
		[]
	);
	assert.deepEqual(reconcileBookingRange(selected, selected, '2026-11-17', '2026-11-18'), []);
});
