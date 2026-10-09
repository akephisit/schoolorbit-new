import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
	compareHomerooms,
	orderStudentYears
} from '../../src/lib/academic-core/roster-ordering.ts';
import { summarizeEducationLevels } from '../../src/lib/school-public/statistics.ts';

const room = (code, number, name = code) => ({ id: code, code, roomNumber: number, name });
test('classrooms sort by education stage, grade and numeric room even with custom names', () => {
	const rooms = [
		room('M6-1', '1'),
		room('M1-10', '10'),
		room('P6-1', '1'),
		room('M1-2', '2', 'ห้องพิเศษ'),
		room('K3-1', '1')
	];
	assert.deepEqual(
		[...rooms].sort(compareHomerooms).map((r) => r.code),
		['K3-1', 'P6-1', 'M1-2', 'M1-10', 'M6-1']
	);
	assert.equal(rooms[0].code, 'M6-1');
});
test('Thai slash room codes group grades before numeric rooms', () => {
	const rooms = ['ม.2/1', 'ม.1/10', 'ม.1/2', 'ม.1/1', 'ม.2/2', 'ป.6/1'].map((code) =>
		room(code, code.split('/')[1])
	);
	assert.deepEqual(
		rooms.sort(compareHomerooms).map((r) => r.code),
		['ป.6/1', 'ม.1/1', 'ม.1/2', 'ม.1/10', 'ม.2/1', 'ม.2/2']
	);
});
test('annual students sort by grade, room and class number, with unassigned entries last', () => {
	const rooms = [room('M1-10', '10'), room('M1-2', '2')];
	const student = (id, grade = 'มัธยมศึกษาปีที่ 1') => ({
		id,
		gradeLevelName: grade,
		studentName: id,
		studentCode: id
	});
	const students = [
		student('unassigned'),
		student('ten'),
		student('two-2'),
		student('two-null'),
		student('two-1'),
		student('primary', 'ประถมศึกษาปีที่ 6')
	];
	const placement = (homeroomId, classNumber, status = 'current') => ({
		homeroomId,
		classNumber,
		status
	});
	const placements = new Map([
		['ten', [placement('M1-10', 1)]],
		['two-2', [placement('M1-10', 9, 'ended'), placement('M1-2', 2)]],
		['two-null', [placement('M1-2', null)]],
		['two-1', [placement('M1-2', 1, 'planned')]]
	]);
	assert.deepEqual(
		orderStudentYears(students, placements, rooms).map((s) => s.id),
		['primary', 'two-1', 'two-2', 'two-null', 'ten', 'unassigned']
	);
	assert.equal(students[0].id, 'unassigned');
});
test('school summaries include configured stages and all gender counts without double-counting unassigned students', () => {
	const counts = (male, female, other = 0) => ({
		male,
		female,
		otherOrUnspecified: other,
		total: male + female + other
	});
	const grade = (levelType, year, students) => ({
		levelType,
		year,
		students,
		unassignedStudents: counts(1, 0),
		homerooms: []
	});
	const groups = summarizeEducationLevels({
		grades: [
			grade('secondary', 6, counts(2, 3)),
			grade('secondary', 1, counts(3, 4, 1)),
			grade('primary', 6, counts(1, 2)),
			grade('kindergarten', 1, counts(2, 2)),
			grade('secondary', 3, counts(1, 2))
		]
	});
	assert.deepEqual(
		groups.map((g) => g.label),
		['อนุบาล', 'ประถมศึกษา', 'มัธยมศึกษาตอนต้น', 'มัธยมศึกษาตอนปลาย']
	);
	assert.deepEqual(groups[2].students, counts(4, 6, 1));
	assert.deepEqual(summarizeEducationLevels({ grades: [] }), []);
	assert.deepEqual(
		summarizeEducationLevels({ grades: [grade('secondary', 4, counts(1, 1))] }).map((g) => g.label),
		['มัธยมศึกษาตอนปลาย']
	);
});
