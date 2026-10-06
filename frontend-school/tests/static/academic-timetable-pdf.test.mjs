import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';

const source = readFileSync(
	new URL('../../src/lib/utils/academic-timetable-pdf.ts', import.meta.url),
	'utf8'
);
const moduleCode = stripTypeScriptTypes(
	source
		.replace(
			'../academic/timetable/teacher-label.ts',
			new URL('../../src/lib/academic/timetable/teacher-label.ts', import.meta.url).href
		)
		.replace(
			'#lib/academic/timetable/board-state.js',
			new URL('../../src/lib/academic/timetable/board-state.ts', import.meta.url).href
		),
	{ mode: 'transform' }
);
const { buildAcademicTimetablePdfDownload } = await import(
	`data:text/javascript;base64,${Buffer.from(moduleCode).toString('base64')}`
);

function workspace() {
	const group = (id, homeroomId, roomId, teacherId) => ({
		learningGroupId: id,
		homeroomIds: [homeroomId],
		roomId,
		roomCode: roomId,
		name: id,
		isActive: true,
		instructors: [{ teacherId, displayName: teacherId }]
	});
	const course = {
		id: 'course',
		blockKind: 'course',
		dayOfWeek: 'MON',
		bellSchedulePeriodId: 'p2',
		isActive: true,
		groups: [group('group-a', 'room-a', 'classroom-a', 'teacher-a')],
		homerooms: [],
		teachers: []
	};
	return {
		version: {
			id: 'selected-draft',
			academicYearId: 'year',
			academicTermId: 'term',
			status: 'draft',
			effectiveFrom: null
		},
		blocks: [
			course,
			{
				...course,
				id: 'sync',
				blockKind: 'activity',
				schedulingMode: 'synchronized',
				groups: [
					group('group-a', 'room-a', 'classroom-a', 'teacher-a'),
					group('group-b', 'room-b', 'classroom-b', 'teacher-b')
				]
			},
			{
				...course,
				id: 'special',
				blockKind: 'structural',
				dayOfWeek: 'SAT',
				groups: [],
				homerooms: [{ homeroomId: 'room-a', name: 'ม.1/1', isActive: true }],
				teachers: [{ teacherId: 'teacher-a', displayName: 'ครูเอ', isActive: true }]
			},
			{ ...course, id: 'deleted', isActive: false }
		],
		homerooms: [
			{ id: 'room-a', name: 'ม.1/1', code: 'M1-1' },
			{ id: 'room-b', name: 'ม.1/2', code: 'M1-2' }
		],
		learningGroups: [
			{ id: 'group-a', name: 'กลุ่ม ก' },
			{ id: 'group-b', name: 'กลุ่ม ข' }
		],
		staff: [
			{ id: 'teacher-a', displayName: 'ครูเอ' },
			{ id: 'teacher-b', displayName: 'ครูบี' }
		],
		bellPeriods: [
			{
				id: 'p2',
				orderIndex: 2,
				isActive: true,
				startTime: '09:00',
				endTime: '10:00',
				applicableDays: 'MON,SAT'
			},
			{ id: 'p1', orderIndex: 1, isActive: true, startTime: '08:00', endTime: '09:00' },
			{ id: 'inactive-period', orderIndex: 3, isActive: false }
		],
		rooms: [{ id: 'classroom-a', name: 'ห้องคณิตศาสตร์' }],
		ordinaryDemands: []
	};
}

test('homeroom PDF keeps course, synchronized and special periods with only that rooms targets', () => {
	const input = workspace();
	const original = structuredClone(input);
	const { pages, fileName } = buildAcademicTimetablePdfDownload(
		input,
		'homeroom',
		'room-a',
		'ภาคเรียนที่ 1',
		'2569'
	);
	assert.equal(pages.length, 1);
	assert.equal(pages[0].title, 'ตารางเรียน ม.1/1');
	assert.deepEqual(
		pages[0].timetableBlocks.map((b) => b.id),
		['course', 'sync', 'special']
	);
	assert.deepEqual(
		pages[0].timetableBlocks[1].groups.map((g) => g.roomId),
		['classroom-a']
	);
	assert.deepEqual(
		pages[0].periods.map((p) => p.id),
		['p1', 'p2']
	);
	assert.ok(pages[0].dayValues.includes('SAT'));
	assert.equal(pages[0].roomNames['classroom-a'], 'ห้องคณิตศาสตร์');
	assert.deepEqual(pages[0].homeroomNames, { 'room-a': 'ม.1/1', 'room-b': 'ม.1/2' });
	assert.equal(pages[0].subTitle, 'ภาคเรียนที่ 1 2569');
	assert.equal(fileName, 'ตารางเรียน ม.1-1 ภาคเรียนที่ 1 2569 แบบร่าง');
	assert.deepEqual(input, original);
});

test('group and teacher exports include only their selected responsibilities', () => {
	const input = workspace();
	const group = buildAcademicTimetablePdfDownload(input, 'learning_group', 'group-b', '', '')
		.pages[0];
	assert.deepEqual(
		group.timetableBlocks.map((b) => b.id),
		['sync']
	);
	assert.deepEqual(
		group.timetableBlocks[0].groups.map((g) => g.learningGroupId),
		['group-b']
	);
	const teacher = buildAcademicTimetablePdfDownload(input, 'teacher', 'teacher-a', '', '').pages[0];
	assert.equal(teacher.viewMode, 'INSTRUCTOR');
	assert.equal(teacher.title, 'ตารางสอน ครูเอ');
	assert.ok(teacher.timetableBlocks.some((b) => b.id === 'special'));
	assert.ok(
		teacher.timetableBlocks.every((b) =>
			b.groups.every((g) => g.instructors.some((t) => t.teacherId === 'teacher-a'))
		)
	);
});

test('whole school exports one full week per homeroom from the chosen published snapshot', () => {
	const input = workspace();
	input.version.status = 'published';
	input.version.effectiveFrom = '2026-05-18';
	const { pages, fileName } = buildAcademicTimetablePdfDownload(
		input,
		'wholeSchool',
		null,
		'ภาคเรียนที่ 1',
		'2569'
	);
	assert.equal(pages.length, 2);
	assert.deepEqual(
		pages.map((p) => p.title),
		['ตารางเรียน ม.1/1', 'ตารางเรียน ม.1/2']
	);
	assert.deepEqual(
		pages[1].timetableBlocks.map((b) => b.id),
		['sync']
	);
	assert.equal(pages[0].subTitle, 'ภาคเรียนที่ 1 2569');
	assert.match(fileName, /^ตารางเรียนทุกห้อง/);
	assert.equal(
		buildAcademicTimetablePdfDownload(input, 'teacher', 'unknown', '', '').pages.length,
		0
	);
});

test('multi-owner exports validate IDs and keep workspace order without repeating owners', () => {
	const input = workspace();
	input.staff[0].displayName = 'นายพิสิษฐ ทดสอบ';
	input.staff[1].displayName = 'นางสาวสายใจ ทดสอบ';
	const result = buildAcademicTimetablePdfDownload(input, 'teacher', null, '', '', [
		'teacher-b',
		'missing',
		'teacher-a',
		'teacher-a'
	]);
	assert.deepEqual(
		result.pages.map((page) => page.title),
		['ตารางสอน ครูพิสิษฐ', 'ตารางสอน ครูสายใจ']
	);
	assert.match(result.fileName, /^ตารางสอนครูที่เลือก/);
	assert.equal(
		buildAcademicTimetablePdfDownload(input, 'homeroom', null, '', '', []).pages.length,
		0
	);
	assert.equal(
		buildAcademicTimetablePdfDownload(input, 'homeroom', null, '', '', ['room-b']).pages[0].title,
		'ตารางเรียน ม.1/2'
	);
});
