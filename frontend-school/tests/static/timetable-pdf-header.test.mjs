import assert from 'node:assert/strict';
import test from 'node:test';
import {
	timetablePdfHeaderContext,
	timetablePdfHomeroomHeader
} from '../../src/lib/utils/timetable-pdf-header.ts';

test('normalizes numeric and already-prefixed academic labels without duplicating prefixes', () => {
	for (const term of ['1', 'ภาคเรียนที่ 1', 'ภาคเรียน 1']) {
		for (const year of ['2569', 'ปีการศึกษา 2569']) {
			assert.equal(timetablePdfHeaderContext(term, year), 'ภาคเรียนที่ 1 ปีการศึกษา 2569');
		}
	}
	assert.equal(
		timetablePdfHeaderContext(' ภาคเรียนที่ ๑ ', ' ปีการศึกษา ๒๕๖๙ '),
		'ภาคเรียนที่ ๑ ปีการศึกษา ๒๕๖๙'
	);
	assert.equal(timetablePdfHeaderContext('', ''), 'ภาคเรียนที่ ปีการศึกษา');
	assert.equal(
		timetablePdfHeaderContext('ภาคเรียนฤดูร้อน', '2569'),
		'ภาคเรียนที่ ฤดูร้อน ปีการศึกษา 2569'
	);
});

test('uses actual grade metadata and preserves custom class names when no grade can be resolved', () => {
	assert.equal(
		timetablePdfHomeroomHeader({
			name: 'ม.1/1',
			gradeLevelType: 'secondary',
			gradeLevelYear: 2,
			roomNumber: '3'
		}),
		'ระดับชั้นมัธยมศึกษาปีที่ 2/3'
	);
	assert.equal(
		timetablePdfHomeroomHeader({ name: 'ป.4/2', gradeLevelType: 'primary', gradeLevelYear: 4 }),
		'ระดับชั้นประถมศึกษาปีที่ 4/2'
	);
	assert.equal(
		timetablePdfHomeroomHeader({
			name: 'อ.2/1',
			gradeLevelType: 'kindergarten',
			gradeLevelYear: 2,
			roomNumber: '1'
		}),
		'ระดับชั้นอนุบาล 2/1'
	);
	assert.equal(timetablePdfHomeroomHeader({ name: 'ม.1/1' }), 'ระดับชั้นมัธยมศึกษาปีที่ 1/1');
	assert.equal(timetablePdfHomeroomHeader({ name: 'ชั้นเตรียมความพร้อม' }), 'ชั้นเตรียมความพร้อม');
});
