import assert from 'node:assert/strict';
import test from 'node:test';
import { gradeLevelSummary } from '../../src/lib/academic-core/catalog-presentation.ts';

const grade = {
	id: 'grade-1',
	code: 'M1',
	name: 'มัธยมศึกษาปีที่ 1',
	short_name: 'ม.1',
	level_order: 301,
	level_type: 'secondary'
};

test('saved coverage uses explicit IDs and never presents empty persisted scope as all', () => {
	assert.equal(gradeLevelSummary([], []), 'ไม่ระบุรายชั้น');
	assert.equal(gradeLevelSummary([grade], []), 'ไม่ระบุรายชั้น');
	assert.equal(gradeLevelSummary([grade], [grade.id]), 'ม.1');
	assert.equal(gradeLevelSummary([grade], [grade.id, 'inactive']), 'ม.1, อีก 1 ระดับชั้น');
	assert.equal(gradeLevelSummary([], ['inactive', 'inactive']), '1 ระดับชั้น');
	assert.equal(gradeLevelSummary([grade], ['missing']), '1 ระดับชั้น');
});
