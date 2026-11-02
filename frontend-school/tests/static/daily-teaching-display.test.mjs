import assert from 'node:assert/strict';
import test from 'node:test';

import * as dailyTeachingDisplay from '../../src/lib/utils/daily-teaching-display.ts';

const {
	DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
	DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
	dailyTeachingEmptyCellLabel,
	dailyTeachingRows,
	filterDailyTeachingRows,
	dailyTeachingGroupInstructors,
	dailyTeachingEntryTitle,
	dailyTeachingCardPresentation,
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
		entryType: 'activity',
		isTeamTeaching: false,
		note: null,
		roomCode: 'INTERNAL-ROOM-CODE',
		roomId: overrides.roomName ? `room-${overrides.roomName}` : null,
		roomName: null,
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
		entry({ entryId: 'entry-10', homeroomNames: ['ม.1/10'], roomName: '120' }),
		entry({ entryId: 'entry-2b', homeroomNames: ['ม.1/2'], roomName: '115' }),
		entry({ entryId: 'entry-2a', homeroomNames: ['ม.1/2'], roomName: '101' }),
		entry({ entryId: 'entry-duplicate', homeroomNames: ['ม.1/2'], roomName: '101' })
	]);

	assert.deepEqual(groups[0].locations, [
		{
			key: 'ม.1/2\u0000room-101',
			homeroomNames: ['ม.1/2'],
			roomName: '101',
			label: 'ม.1/2 / 101'
		},
		{
			key: 'ม.1/2\u0000room-115',
			homeroomNames: ['ม.1/2'],
			roomName: '115',
			label: 'ม.1/2 / 115'
		},
		{
			key: 'ม.1/10\u0000room-120',
			homeroomNames: ['ม.1/10'],
			roomName: '120',
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
			entryType: 'course',
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
		entry({ roomName: '101' }),
		entry({ entryId: 'entry-b', roomName: '101' }),
		entry({ entryId: 'entry-c', homeroomNames: [], roomName: 'หอประชุม' }),
		entry({ entryId: 'entry-d', homeroomNames: [], roomName: null })
	]);

	assert.deepEqual(groups[0].classroomLabels, ['ป.1/1 / 101', 'หอประชุม']);
	assert.equal(groups[0].entries.length, 4);
	assert.equal(displayGroupCountLabel(groups[0]), '2 ห้อง');
});

test('falls back to an entry count when a synchronized group has no classroom labels', () => {
	const groups = groupDailyTeachingEntries([
		entry({ homeroomNames: [], roomName: null }),
		entry({ entryId: 'entry-b', homeroomNames: [], roomName: null })
	]);

	assert.equal(displayGroupCountLabel(groups[0]), '2 รายการ');
});

test('calculates the readable table minimum from the teacher and period columns', () => {
	assert.equal(DAILY_TEACHING_TEACHER_COLUMN_WIDTH, 104);
	assert.equal(DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH, 84);
	assert.equal(typeof dailyTeachingTableMinWidth, 'function');
	assert.equal(dailyTeachingTableMinWidth(0), 104);
	assert.equal(dailyTeachingTableMinWidth(4), 440);
	assert.equal(dailyTeachingTableMinWidth(10), 944);
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

test('builds an accessible label for a visually empty period cell', () => {
	assert.equal(typeof dailyTeachingEmptyCellLabel, 'function');
	assert.equal(
		dailyTeachingEmptyCellLabel('วิภาวดี วงศ์ศรี', 'คาบที่ 1', '08:40-09:30'),
		'วิภาวดี วงศ์ศรี คาบที่ 1 08:40-09:30: ว่าง'
	);
});

function cardPresentation(
	entries,
	view = 'teacher',
	instructors = [{ id: 'teacher-a', displayName: 'นายสมชาย ใจดี' }]
) {
	const group = groupDailyTeachingEntries(entries, view)[0];
	const cell = {
		bellSchedulePeriodId: 'period-a',
		entries,
		instructorsByEntry: new Map(entries.map((item) => [item.entryId, instructors]))
	};
	return dailyTeachingCardPresentation(group, cell, view);
}

test('course cards show code and subject name separately from target and physical room', () => {
	const course = entry({
		entryType: 'course',
		activitySchedulingMode: null,
		title: null,
		offeringCode: 'ค21101',
		offeringName: 'คณิตศาสตร์พื้นฐาน',
		homeroomNames: ['ม.1/1', 'ม.1/2'],
		roomName: 'ห้องคณิตศาสตร์'
	});
	assert.equal(dailyTeachingEntryTitle(course), 'คณิตศาสตร์พื้นฐาน');
	assert.deepEqual(cardPresentation([course]), {
		code: 'ค21101',
		title: 'คณิตศาสตร์พื้นฐาน',
		titleLineLimit: 1,
		targetLabel: 'ม.1/1 +1',
		teacherLabel: null,
		teacherTitle: null,
		roomLabel: 'ห้องคณิตศาสตร์'
	});
	const homeroom = cardPresentation([course], 'homeroom', [
		{ id: 'a', displayName: 'นายสมชาย ใจดี' },
		{ id: 'b', displayName: 'นางสาวสายใจ วิทยา' }
	]);
	assert.equal(homeroom.targetLabel, null);
	assert.equal(homeroom.teacherLabel, 'ครูสายใจ +1');
	assert.equal(homeroom.teacherTitle, 'นางสาวสายใจ วิทยา, นายสมชาย ใจดี');
	assert.equal(homeroom.roomLabel, 'ห้องคณิตศาสตร์');
	assert.doesNotMatch(JSON.stringify(homeroom), /INTERNAL-ROOM-CODE/);
});

test('shared homeroom cards wrap meaningful titles and hide teacher metadata and invented codes', () => {
	for (const overrides of [
		{ entryType: 'structural', activitySchedulingMode: null },
		{ entryType: 'break', activitySchedulingMode: null },
		{ entryType: 'homeroom', activitySchedulingMode: null },
		{ entryType: 'activity', activitySchedulingMode: 'synchronized' },
		{ entryType: 'activity', activitySchedulingMode: 'synchronized', offeringId: null }
	]) {
		const presentation = cardPresentation(
			[
				entry({
					...overrides,
					offeringCode: 'SHOULD-NOT-BE-A-CODE',
					title: 'กิจกรรมส่งเสริมคุณธรรม',
					roomName: 'หอประชุม'
				})
			],
			'homeroom'
		);
		assert.equal(presentation.code, null);
		assert.equal(presentation.title, 'กิจกรรมส่งเสริมคุณธรรม');
		assert.equal(presentation.titleLineLimit, 3);
		assert.equal(presentation.teacherLabel, null);
		assert.equal(presentation.teacherTitle, null);
		assert.equal(presentation.targetLabel, null);
		assert.equal(presentation.roomLabel, 'หอประชุม');
	}
});

test('independent homeroom activities retain compact instructor and named-room rows', () => {
	const presentation = cardPresentation(
		[
			entry({
				activitySchedulingMode: 'independent',
				title: 'ชุมนุมวิทย์',
				roomId: 'room-a',
				roomName: null
			})
		],
		'homeroom'
	);
	assert.equal(presentation.title, 'ชุมนุมวิทย์');
	assert.equal(presentation.code, null);
	assert.equal(presentation.teacherLabel, 'ครูสมชาย');
	assert.equal(presentation.roomLabel, 'ไม่พบชื่อห้อง');
	assert.equal(presentation.titleLineLimit, 1);
	assert.equal(
		cardPresentation(
			[entry({ activitySchedulingMode: 'independent', roomId: null, roomName: null })],
			'homeroom'
		).roomLabel,
		'ไม่ระบุห้อง'
	);
});

test('teacher cards summarize shared targets while keeping room names on their own field', () => {
	const shared = cardPresentation([
		entry({
			homeroomNames: Array.from({ length: 18 }, (_, index) => `ม.1/${index + 1}`),
			roomName: 'หอประชุม'
		})
	]);
	assert.equal(shared.targetLabel, '18 ห้อง');
	assert.equal(shared.roomLabel, 'หอประชุม');
	assert.equal(shared.teacherLabel, null);
	assert.equal(
		dailyTeachingEntryTitle(entry({ entryType: 'break', title: null, offeringName: null })),
		'พัก'
	);
});

test('physical room names display independently from codes and distinct room IDs remain separate', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'a', roomId: 'room-a', roomName: 'ห้องวิทยาศาสตร์', roomCode: 'SCI-A' }),
		entry({ entryId: 'b', roomId: 'room-b', roomName: 'ห้องวิทยาศาสตร์', roomCode: 'SCI-B' }),
		entry({ entryId: 'c', roomId: 'room-c', roomName: null, roomCode: 'SECRET-ROOM-C' })
	]);
	assert.equal(groups[0].locations.length, 3);
	assert.deepEqual(
		groups[0].classroomLabels.filter((label) => label.includes('ห้องวิทยาศาสตร์')),
		['ป.1/1 / ห้องวิทยาศาสตร์', 'ป.1/1 / ห้องวิทยาศาสตร์']
	);
	assert.ok(groups[0].classroomLabels.includes('ป.1/1 / ไม่พบชื่อห้อง'));
	assert.doesNotMatch(groups[0].classroomLabels.join(' '), /SCI-A|SCI-B|SECRET-ROOM-C|room-c/);
});

function teacher(id, displayName, entries, periodId = 'period-a') {
	return { id, displayName, periods: [{ bellSchedulePeriodId: periodId, entries }] };
}

test('homeroom projection naturally sorts explicit assignments and deduplicates team teaching', () => {
	const shared = entry({
		entryId: 'shared',
		homeroomNames: ['ม.1/10', ' ม.1/2 ', 'ม.1/2'],
		roomName: 'ห้องวิทย์'
	});
	const independent = entry({
		entryId: 'independent',
		activitySchedulingMode: 'independent',
		homeroomNames: ['ม.1/2']
	});
	const source = [
		teacher('teacher-b', 'ครู ข', [
			shared,
			independent,
			entry({ entryId: 'unassigned', homeroomNames: [] })
		]),
		teacher('teacher-a', 'ครู ก', [shared])
	];
	const before = structuredClone(source);
	const rows = dailyTeachingRows(source, 'homeroom');
	assert.deepEqual(
		rows.map((row) => row.displayName),
		['ม.1/2', 'ม.1/10']
	);
	assert.deepEqual(
		rows[0].periods[0].entries.map((item) => item.entryId),
		['shared', 'independent']
	);
	assert.deepEqual(
		dailyTeachingGroupInstructors([shared], rows[0].periods[0]).map((item) => item.id),
		['teacher-a', 'teacher-b']
	);
	assert.equal(
		dailyTeachingCardPresentation(
			groupDailyTeachingEntries([shared])[0],
			rows[0].periods[0],
			'homeroom'
		).teacherLabel,
		null
	);
	assert.deepEqual(source, before);
});

test('homeroom synchronized groups retain independent entries and exact-period instructor sets', () => {
	const syncA = entry({ entryId: 'sync-a', homeroomNames: ['ม.1/1'] });
	const syncB = entry({ entryId: 'sync-b', homeroomNames: ['ม.1/1', 'ม.1/2'] });
	const independentA = entry({
		entryId: 'ind-a',
		activitySchedulingMode: 'independent',
		homeroomNames: ['ม.1/1']
	});
	const independentB = entry({
		entryId: 'ind-b',
		activitySchedulingMode: 'independent',
		homeroomNames: ['ม.1/1']
	});
	const rows = dailyTeachingRows(
		[
			teacher('a', 'ครู ก', [syncA, independentA, independentB]),
			teacher('b', 'ครู ข', [syncB]),
			teacher('c', 'ครู ค', [entry({ ...syncA, homeroomNames: ['ม.2/1'] })], 'period-b')
		],
		'homeroom'
	);
	const cell = rows[0].periods[0];
	assert.deepEqual(
		groupDailyTeachingEntries(cell.entries).map((group) =>
			group.entries.map((item) => item.entryId)
		),
		[['sync-a', 'sync-b'], ['ind-a'], ['ind-b']]
	);
	assert.deepEqual(
		dailyTeachingGroupInstructors([syncA], cell).map((item) => item.id),
		['a']
	);
	const secondHomeroom = rows.find((row) => row.id === 'ม.1/2');
	assert.deepEqual(
		dailyTeachingGroupInstructors(secondHomeroom.periods[0].entries, secondHomeroom.periods[0]).map(
			(item) => item.id
		),
		['b']
	);
});

test('teacher projection keeps empty teachers and unassigned lessons without inventing homerooms', () => {
	const unassigned = entry({ homeroomNames: [], learningGroupName: 'กลุ่มเสรี' });
	const source = [teacher('a', 'ครู ก', [unassigned]), teacher('b', 'ครู ข', [])];
	assert.deepEqual(
		dailyTeachingRows(source, 'teacher').map((row) => row.id),
		['a', 'b']
	);
	assert.equal(dailyTeachingRows(source, 'teacher')[0].periods[0].entries[0], unassigned);
	assert.deepEqual(dailyTeachingRows(source, 'homeroom'), []);
});

test('owner and text filters compose over the whole projection and stale owners stay empty', () => {
	const rows = dailyTeachingRows(
		[
			teacher('a', 'ครู ก', [
				entry({ homeroomNames: ['ม.1/1'], offeringName: 'คณิตศาสตร์', roomName: 'ห้องวิทยาศาสตร์' })
			]),
			teacher('b', 'ครู ข', [
				entry({ entryId: 'b', homeroomNames: ['ม.1/2'], offeringName: 'ภาษาไทย' })
			])
		],
		'homeroom'
	);
	assert.equal(filterDailyTeachingRows(rows, 'all', '').length, 2);
	assert.deepEqual(
		filterDailyTeachingRows(rows, 'all', 'ครู ก').map((row) => row.id),
		['ม.1/1']
	);
	assert.deepEqual(
		filterDailyTeachingRows(rows, 'all', 'ห้องวิทยาศาสตร์').map((row) => row.id),
		['ม.1/1']
	);
	assert.equal(filterDailyTeachingRows(rows, 'ม.1/2', 'คณิตศาสตร์').length, 0);
	assert.equal(filterDailyTeachingRows(rows, 'removed-owner', '').length, 0);
	assert.equal(filterDailyTeachingRows(rows, 'all', 'INTERNAL-ROOM-CODE').length, 0);
});

test('homeroom structural presentation collapses 24 teacher allocations while retaining source evidence', () => {
	const source = Array.from({ length: 24 }, (_, index) =>
		teacher(`teacher-${index}`, `ครูผู้สอน ${index}`, [
			entry({
				entryId: `structural-${index}`,
				entryType: 'structural',
				activitySchedulingMode: null,
				offeringId: null,
				title: 'กิจกรรมหน้าเสาธง',
				homeroomNames: ['ม.1/1'],
				roomId: 'assembly-room',
				roomName: 'ลานกิจกรรม',
				note: 'ร่วมกิจกรรม'
			})
		])
	);
	const before = structuredClone(source);
	const row = dailyTeachingRows(source, 'homeroom')[0];
	const cell = row.periods[0];
	const groups = groupDailyTeachingEntries(cell.entries, 'homeroom');
	assert.equal(groups.length, 1);
	assert.equal(groups[0].entries.length, 24);
	assert.equal(dailyTeachingGroupInstructors(groups[0].entries, cell).length, 24);
	assert.equal(dailyTeachingCardPresentation(groups[0], cell, 'homeroom').teacherLabel, null);
	assert.equal(groupDailyTeachingEntries(cell.entries, 'teacher').length, 24);
	assert.deepEqual(source, before);
});

test('structural visual grouping preserves differing title, note, physical room and offering/group identity', () => {
	const base = entry({
		entryType: 'structural',
		activitySchedulingMode: null,
		title: 'คาบพิเศษ',
		offeringId: null,
		roomId: 'room-a',
		roomName: 'ห้องประชุม',
		note: null
	});
	const changes = [
		{ title: 'อีกกิจกรรม' },
		{ note: 'อีกหมายเหตุ' },
		{ roomId: 'room-b' },
		{ roomName: 'อีกห้อง' },
		{ offeringId: 'offering-b' },
		{ learningGroupId: 'group-b' },
		{ learningGroupName: 'กลุ่มอื่น' }
	];
	const entries = [
		base,
		entry({ ...base, entryId: 'same-presentation' }),
		...changes.map((change, index) => entry({ ...base, entryId: `different-${index}`, ...change }))
	];
	const groups = groupDailyTeachingEntries(entries, 'homeroom');
	assert.equal(groups.length, changes.length + 1);
	assert.deepEqual(
		groups[0].entries.map((item) => item.entryId),
		['entry-a', 'same-presentation']
	);
});

test('homeroom visual grouping never collapses independent activities or regular courses', () => {
	for (const entryType of ['course', 'activity']) {
		const entries = ['a', 'b'].map((entryId) =>
			entry({ entryId, entryType, activitySchedulingMode: 'independent', title: 'เหมือนกัน' })
		);
		assert.equal(groupDailyTeachingEntries(entries, 'homeroom').length, 2);
	}
	for (const entryType of ['break', 'homeroom']) {
		const entries = ['a', 'b'].map((entryId) =>
			entry({ entryId, entryType, activitySchedulingMode: null })
		);
		assert.equal(groupDailyTeachingEntries(entries, 'homeroom').length, 1);
		assert.equal(groupDailyTeachingEntries(entries, 'teacher').length, 2);
	}
});
