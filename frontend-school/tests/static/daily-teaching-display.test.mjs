import assert from 'node:assert/strict';
import test from 'node:test';

import * as dailyTeachingDisplay from '../../src/lib/utils/daily-teaching-display.ts';

const {
	DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
	DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
	dailyTeachingTableMinWidth,
	dailyTeachingTeacherCell,
	displayGroupCountLabel,
	groupDailyTeachingEntries
} = dailyTeachingDisplay;

function entry(overrides = {}) {
	return {
		activitySchedulingMode: 'synchronized',
		activitySlotId: 'slot-a',
		classroomName: 'ป.1/1',
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
		entry({ entryId: 'entry-b', classroomName: 'ป.1/2' })
	]);

	assert.equal(groups.length, 1);
	assert.deepEqual(
		groups[0].entries.map((item) => item.entryId),
		['entry-a', 'entry-b']
	);
	assert.deepEqual(groups[0].classroomLabels, ['ป.1/1', 'ป.1/2']);
	assert.equal(displayGroupCountLabel(groups[0]), '2 ห้อง');
});

test('keeps synchronized activities from different slots separate', () => {
	const groups = groupDailyTeachingEntries([
		entry(),
		entry({ entryId: 'entry-b', activitySlotId: 'slot-b', classroomName: 'ป.1/2' })
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
		entry({ entryId: 'missing-slot-a', activitySlotId: null }),
		entry({ entryId: 'missing-slot-b', activitySlotId: null }),
		entry({ entryId: 'missing-mode-a', activitySchedulingMode: null }),
		entry({ entryId: 'missing-mode-b', activitySchedulingMode: null })
	]);

	assert.deepEqual(
		groups.map((group) => group.entries.map((item) => item.entryId)),
		[
			['independent-a'],
			['independent-b'],
			['missing-slot-a'],
			['missing-slot-b'],
			['missing-mode-a'],
			['missing-mode-b']
		]
	);
});

test('preserves the first occurrence order when later synchronized entries merge', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'sync-a-1', activitySlotId: 'slot-a' }),
		entry({
			entryId: 'course-a',
			entryType: 'COURSE',
			activitySlotId: null,
			activitySchedulingMode: null,
			classroomName: 'ป.2/1',
			subjectCode: 'ค12101',
			subjectName: 'คณิตศาสตร์',
			title: null
		}),
		entry({ entryId: 'sync-a-2', activitySlotId: 'slot-a', classroomName: 'ป.1/2' }),
		entry({ entryId: 'sync-b-1', activitySlotId: 'slot-b', classroomName: 'ป.3/1' })
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
		entry({ entryId: 'entry-c', classroomName: null, roomCode: 'หอประชุม' }),
		entry({ entryId: 'entry-d', classroomName: null, roomCode: null })
	]);

	assert.deepEqual(groups[0].classroomLabels, ['ป.1/1 / 101', 'หอประชุม']);
	assert.equal(groups[0].entries.length, 4);
	assert.equal(displayGroupCountLabel(groups[0]), '2 ห้อง');
});

test('falls back to an entry count when a synchronized group has no classroom labels', () => {
	const groups = groupDailyTeachingEntries([
		entry({ classroomName: null, roomCode: null }),
		entry({ entryId: 'entry-b', classroomName: null, roomCode: null })
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
