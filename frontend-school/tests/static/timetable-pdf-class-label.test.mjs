import assert from 'node:assert/strict';
import test from 'node:test';
import { timetablePdfClassLabel } from '../../src/lib/utils/timetable-pdf-class-label.ts';

const course = {
	blockKind: 'course',
	schedulingMode: 'independent',
	homerooms: [],
	groups: [{ homeroomIds: ['room-a', 'room-b'], name: 'ม.1/1 · คณิตศาสตร์พื้นฐาน' }]
};

test('PDF class lines use canonical homeroom names and deduplicate shared targets', () => {
	const block = {
		...course,
		groups: [...course.groups, { homeroomIds: ['room-a'], name: 'ชื่อกลุ่ม' }],
		homerooms: [
			{ name: 'ม.1/2', isActive: true },
			{ name: 'ม.1/3', isActive: false }
		]
	};
	const original = structuredClone(block);
	assert.equal(
		timetablePdfClassLabel(block, { 'room-a': 'ม.1/1', 'room-b': 'ม.1/2' }),
		'ม.1/1, ม.1/2'
	);
	assert.deepEqual(block, original);
});

test('own PDF class lines shorten homeroom-prefixed groups without losing named groups', () => {
	assert.equal(timetablePdfClassLabel(course), 'ม.1/1');
	assert.equal(
		timetablePdfClassLabel({
			...course,
			groups: [{ homeroomIds: [], name: 'ป. 2 / 1, ป.2/2 ภาษาไทย' }]
		}),
		'ป.2/1, ป.2/2'
	);
	assert.equal(
		timetablePdfClassLabel({ ...course, groups: [{ homeroomIds: [], name: 'กลุ่มเลือกเสรี' }] }),
		'กลุ่มเลือกเสรี'
	);
});

test('special and synchronized activity periods omit student targets', () => {
	assert.equal(timetablePdfClassLabel({ ...course, blockKind: 'structural' }), '');
	assert.equal(
		timetablePdfClassLabel({ ...course, blockKind: 'activity', schedulingMode: 'synchronized' }),
		''
	);
	assert.equal(timetablePdfClassLabel({ ...course, blockKind: 'activity' }), 'ม.1/1');
});
