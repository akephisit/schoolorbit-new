import assert from 'node:assert/strict';
import test from 'node:test';
import {
	timetableTeacherLabel,
	timetableTeacherFullLabel
} from '../../src/lib/academic/timetable/teacher-label.ts';
import { buildTimetableBlockDisplay } from '../../src/lib/academic/timetable/block-display.ts';

test('timetable labels strip Thai titles and surnames without doubling the teacher prefix', () => {
	for (const name of [
		'นายพิสิษฐ สกุลทดสอบ',
		'นาย พิสิษฐ สกุลทดสอบ',
		'ครูพิสิษฐ สกุลทดสอบ',
		'ครู นางสาวพิสิษฐ สกุลทดสอบ',
		' พิสิษฐ  สกุลทดสอบ ',
		'ดร. พิสิษฐ สกุลทดสอบ'
	]) {
		assert.equal(timetableTeacherLabel(name), 'ครูพิสิษฐ');
	}
	assert.equal(timetableTeacherLabel('นางนัฎฐา สอดโคกสูง'), 'ครูนัฎฐา');
	assert.equal(timetableTeacherLabel('Mr. John Smith'), 'ครูJohn');
	assert.equal(timetableTeacherLabel('Mrs. Mary Smith'), 'ครูMary');
	assert.equal(timetableTeacherLabel('Ms. Mary Smith'), 'ครูMary');
	assert.equal(timetableTeacherLabel(''), '-');
	assert.equal(timetableTeacherLabel('-'), '-');
});

test('teachers with the same first name remain separate responsibilities', () => {
	const block = {
		blockKind: 'course',
		schedulingMode: null,
		groups: [
			{
				name: 'ม.1/1',
				homeroomIds: [],
				instructors: [
					{ teacherId: 'a', displayName: 'นายพิสิษฐ หนึ่ง' },
					{ teacherId: 'b', displayName: 'นายพิสิษฐ สอง' }
				]
			}
		],
		homerooms: [],
		teachers: [{ teacherId: 'a', displayName: 'นายพิสิษฐ หนึ่ง' }]
	};
	assert.equal(buildTimetableBlockDisplay(block, 'scheduler').teacherLabel, 'ครูพิสิษฐ +1');
	assert.equal(buildTimetableBlockDisplay(block, 'personal').teacherLabel, 'ครูพิสิษฐ, ครูพิสิษฐ');
});

test('PDF teacher labels preserve full names while replacing honorifics', () => {
	for (const name of [
		'นายพิสิษฐ ชนามุยา',
		'นาย พิสิษฐ ชนามุยา',
		'ครูพิสิษฐ ชนามุยา',
		'ครู นายพิสิษฐ ชนามุยา',
		' พิสิษฐ  ชนามุยา '
	]) {
		assert.equal(timetableTeacherFullLabel(name), 'ครูพิสิษฐ ชนามุยา');
	}
	assert.equal(timetableTeacherFullLabel('นางนัฎฐา สอดโคกสูง'), 'ครูนัฎฐา สอดโคกสูง');
	assert.equal(timetableTeacherFullLabel('Mrs. Mary Smith'), 'ครูMary Smith');
	assert.equal(timetableTeacherFullLabel('-'), '-');
	assert.equal(timetableTeacherFullLabel(''), '-');
});
