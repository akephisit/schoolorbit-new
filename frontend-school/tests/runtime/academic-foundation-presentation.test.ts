import assert from 'node:assert/strict';
import test from 'node:test';

import {
	customNameFromStored,
	normalizeSchoolDays,
	standardAcademicYearName,
	standardTermName
} from '../../src/lib/academic-core/foundation-presentation.ts';

test('academic foundation standard names match the backend-derived Thai labels', () => {
	assert.equal(standardAcademicYearName(2571), 'ปีการศึกษา 2571');
	assert.equal(standardTermName('regular', 2), 'ภาคเรียนที่ 2');
	assert.equal(standardTermName('summer', 3), 'ภาคฤดูร้อน');
	assert.equal(standardTermName('remedial', 4), 'ภาคซ่อมเสริม');
	assert.equal(standardTermName('custom', 5), 'ภาคเรียนกำหนดเอง 5');
});

test('academic foundation custom-name input is empty for a stored standard label', () => {
	assert.equal(customNameFromStored('ปีการศึกษา 2571', 'ปีการศึกษา 2571'), '');
	assert.equal(customNameFromStored(' ปีการศึกษา 2571 ', 'ปีการศึกษา 2571'), '');
	assert.equal(customNameFromStored('ปีการศึกษา 2569', 'ปีการศึกษา 2571'), '');
	assert.equal(customNameFromStored('ภาคเรียนที่ 4', 'ภาคเรียนที่ 2'), '');
	assert.equal(customNameFromStored('ม.1/2', 'ม.1/3'), '');
	assert.equal(customNameFromStored('ปีแห่งการอ่าน', 'ปีการศึกษา 2571'), 'ปีแห่งการอ่าน');
	assert.equal(customNameFromStored('ห้องส่งเสริมวิทยาศาสตร์', 'ม.1/3'), 'ห้องส่งเสริมวิทยาศาสตร์');
});

test('school days are canonical, unique, and ignore unsupported values', () => {
	assert.deepEqual(normalizeSchoolDays(['FRI', 'MON', 'MON']), ['MON', 'FRI']);
	assert.deepEqual(normalizeSchoolDays(['sun', 'TUE', 'HOLIDAY', '']), ['TUE', 'SUN']);
});
