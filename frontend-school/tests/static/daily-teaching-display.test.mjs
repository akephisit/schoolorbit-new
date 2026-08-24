import assert from 'node:assert/strict';
import test from 'node:test';

import * as dailyTeachingDisplay from '../../src/lib/utils/daily-teaching-display.ts';

const {
	DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
	DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
	dailyTeachingEmptyCellLabel,
	dailyTeachingEntryCardPresentation,
	dailyTeachingTableMinWidth,
	dailyTeachingTeacherCell,
	displayGroupCountLabel,
	groupDailyTeachingEntries
} = dailyTeachingDisplay;

function entry(overrides = {}) {
	return {
		activitySchedulingMode: 'synchronized',
		offeringId: 'offering-a',
		homeroomNames: ['ป.1/1'],
		entryId: 'entry-a',
		entryType: 'ACTIVITY',
		isTeamTeaching: false,
		note: null,
		roomCode: null,
		subjectCode: null,
		subjectGroupName: null,
		subjectName: null,
		title: 'ลูกเสือ เนตรนารี',
		...overrides
	};
}

test('merges synchronized entries that share one activity slot', () => {
	const groups = groupDailyTeachingEntries([
		entry(),
		entry({ entryId: 'entry-b', homeroomNames: ['ป.1/2'] })
	]);

	assert.equal(groups.length, 1);
	assert.deepEqual(
		groups[0].entries.map((item) => item.entryId),
		['entry-a', 'entry-b']
	);
	assert.deepEqual(groups[0].classroomLabels, ['ป.1/1', 'ป.1/2']);
	assert.equal(displayGroupCountLabel(groups[0]), '2 ห้อง');
});

test('sorts synchronized locations naturally by classroom then physical room', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'entry-10', homeroomNames: ['ม.1/10'], roomCode: '120' }),
		entry({ entryId: 'entry-2b', homeroomNames: ['ม.1/2'], roomCode: '115' }),
		entry({ entryId: 'entry-2a', homeroomNames: ['ม.1/2'], roomCode: '101' }),
		entry({ entryId: 'entry-duplicate', homeroomNames: ['ม.1/2'], roomCode: '101' })
	]);

	assert.deepEqual(groups[0].locations, [
		{
			key: 'ม.1/2\u0000101',
			homeroomNames: ['ม.1/2'],
			roomCode: '101',
			label: 'ม.1/2 / 101'
		},
		{
			key: 'ม.1/2\u0000115',
			homeroomNames: ['ม.1/2'],
			roomCode: '115',
			label: 'ม.1/2 / 115'
		},
		{
			key: 'ม.1/10\u0000120',
			homeroomNames: ['ม.1/10'],
			roomCode: '120',
			label: 'ม.1/10 / 120'
		}
	]);
	assert.deepEqual(groups[0].classroomLabels, ['ม.1/2 / 101', 'ม.1/2 / 115', 'ม.1/10 / 120']);
	assert.equal(displayGroupCountLabel(groups[0]), '3 ห้อง');
});

test('keeps synchronized activities from different slots separate', () => {
	const groups = groupDailyTeachingEntries([
		entry(),
		entry({ entryId: 'entry-b', offeringId: 'offering-b', homeroomNames: ['ป.1/2'] })
	]);

	assert.deepEqual(
		groups.map((group) => group.entries.map((item) => item.entryId)),
		[['entry-a'], ['entry-b']]
	);
});

test('does not merge independent or incomplete activity entries', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'independent-a', activitySchedulingMode: 'independent' }),
		entry({ entryId: 'independent-b', activitySchedulingMode: 'independent' }),
		entry({ entryId: 'missing-offering-a', offeringId: null }),
		entry({ entryId: 'missing-offering-b', offeringId: null }),
		entry({ entryId: 'missing-mode-a', activitySchedulingMode: null }),
		entry({ entryId: 'missing-mode-b', activitySchedulingMode: null })
	]);

	assert.deepEqual(
		groups.map((group) => group.entries.map((item) => item.entryId)),
		[
			['independent-a'],
			['independent-b'],
			['missing-offering-a'],
			['missing-offering-b'],
			['missing-mode-a'],
			['missing-mode-b']
		]
	);
});

test('preserves the first occurrence order when later synchronized entries merge', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'sync-a-1', offeringId: 'offering-a' }),
		entry({
			entryId: 'course-a',
			entryType: 'COURSE',
			offeringId: 'course-offering-a',
			activitySchedulingMode: null,
			homeroomNames: ['ป.2/1'],
			subjectCode: 'ค12101',
			subjectName: 'คณิตศาสตร์',
			title: null
		}),
		entry({ entryId: 'sync-a-2', offeringId: 'offering-a', homeroomNames: ['ป.1/2'] }),
		entry({ entryId: 'sync-b-1', offeringId: 'offering-b', homeroomNames: ['ป.3/1'] })
	]);

	assert.deepEqual(
		groups.map((group) => group.entries.map((item) => item.entryId)),
		[['sync-a-1', 'sync-a-2'], ['course-a'], ['sync-b-1']]
	);
});

test('deduplicates classroom and room labels without discarding entries', () => {
	const groups = groupDailyTeachingEntries([
		entry({ roomCode: '101' }),
		entry({ entryId: 'entry-b', roomCode: '101' }),
		entry({ entryId: 'entry-c', homeroomNames: [], roomCode: 'หอประชุม' }),
		entry({ entryId: 'entry-d', homeroomNames: [], roomCode: null })
	]);

	assert.deepEqual(groups[0].classroomLabels, ['ป.1/1 / 101', 'หอประชุม']);
	assert.equal(groups[0].entries.length, 4);
	assert.equal(displayGroupCountLabel(groups[0]), '2 ห้อง');
});

test('falls back to an entry count when a synchronized group has no classroom labels', () => {
	const groups = groupDailyTeachingEntries([
		entry({ homeroomNames: [], roomCode: null }),
		entry({ entryId: 'entry-b', homeroomNames: [], roomCode: null })
	]);

	assert.equal(displayGroupCountLabel(groups[0]), '2 รายการ');
});

test('calculates the readable table minimum from the teacher and period columns', () => {
	assert.equal(DAILY_TEACHING_TEACHER_COLUMN_WIDTH, 128);
	assert.equal(DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH, 132);
	assert.equal(typeof dailyTeachingTableMinWidth, 'function');
	assert.equal(dailyTeachingTableMinWidth(0), 128);
	assert.equal(dailyTeachingTableMinWidth(4), 656);
	assert.equal(dailyTeachingTableMinWidth(10), 1448);
});

test('teacher cell presentation excludes subject-group subtitles', () => {
	assert.equal(typeof dailyTeachingTeacherCell, 'function');
	assert.deepEqual(
		dailyTeachingTeacherCell({
			displayName: 'วิภาวดี วงศ์ศรี',
			subjectGroupNames: ['ภาษาไทย']
		}),
		{
			label: 'วิภาวดี วงศ์ศรี',
			title: 'วิภาวดี วงศ์ศรี'
		}
	);
});

test('uses detailed cards for courses and independent activities', () => {
	assert.equal(typeof dailyTeachingEntryCardPresentation, 'function');
	assert.deepEqual(
		dailyTeachingEntryCardPresentation(
			entry({ entryType: 'COURSE', activitySchedulingMode: null })
		),
		{ tone: 'course', layout: 'details', titleLineLimit: 2 }
	);
	assert.deepEqual(
		dailyTeachingEntryCardPresentation(
			entry({ entryType: 'ACTIVITY', activitySchedulingMode: 'independent' })
		),
		{ tone: 'activity', layout: 'details', titleLineLimit: 2 }
	);
	assert.deepEqual(dailyTeachingEntryCardPresentation(entry({ entryType: 'ACTIVITY' })), {
		tone: 'activity',
		layout: 'centered',
		titleLineLimit: 3
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation(entry({ entryType: 'ACADEMIC' })), {
		tone: 'course',
		layout: 'centered',
		titleLineLimit: 3
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation(entry({ entryType: 'HOMEROOM' })), {
		tone: 'activity',
		layout: 'centered',
		titleLineLimit: 3
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation(entry({ entryType: 'BREAK' })), {
		tone: 'break',
		layout: 'centered',
		titleLineLimit: 3
	});
});

test('builds an accessible label for a visually empty period cell', () => {
	assert.equal(typeof dailyTeachingEmptyCellLabel, 'function');
	assert.equal(
		dailyTeachingEmptyCellLabel('วิภาวดี วงศ์ศรี', 'คาบที่ 1', '08:40-09:30'),
		'วิภาวดี วงศ์ศรี คาบที่ 1 08:40-09:30: ว่าง'
	);
});
