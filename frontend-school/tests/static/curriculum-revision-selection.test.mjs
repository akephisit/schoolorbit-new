import assert from 'node:assert/strict';
import test from 'node:test';
import {
	curriculumEditionLabel,
	groupCurriculaByRevision,
	programsForGrade,
	studyProgramLabel
} from '../../src/lib/academic-core/curriculum-presentation.ts';

test('a migrated numeric edition has an amendment label without a calendar expiry', () => {
	assert.equal(
		curriculumEditionLabel({ versionName: '2569', revisionYear: 2569 }),
		'ฉบับปรับปรุง พุทธศักราช 2569'
	);
	assert.equal(
		curriculumEditionLabel({ versionName: 'ฉบับทดลอง', revisionYear: null }),
		'ฉบับทดลอง'
	);
});

test('grade selection retains both published editions of a plan and distinguishes them', () => {
	const plans = [2569, 2572].map((year) => ({
		id: `plan-${year}`,
		name: 'วิทยาศาสตร์-คณิตศาสตร์',
		versionName: String(year),
		revisionYear: year,
		curriculumName: 'ระดับมัธยมศึกษาตอนต้น',
		gradeLevelIds: ['M1', 'M2', 'M3']
	}));
	assert.deepEqual(programsForGrade(plans, 'M1'), plans);
	assert.deepEqual(programsForGrade(plans, 'M4'), []);
	assert.notEqual(studyProgramLabel(plans[0]), studyProgramLabel(plans[1]));
	assert.match(studyProgramLabel(plans[0]), /2569/);
});

test('the two secondary levels share an edition heading while draft-only roots stay separate', () => {
	const items = [
		{ displayVersion: { versionName: '2569', revisionYear: 2569 } },
		{ displayVersion: { versionName: 'ฉบับปรับปรุง พุทธศักราช 2569', revisionYear: 2569 } },
		{ displayVersion: null }
	];
	const groups = groupCurriculaByRevision(items);
	assert.equal(groups.length, 2);
	assert.deepEqual(groups[0].items, items.slice(0, 2));
	assert.deepEqual(groups[1].items, [items[2]]);
});
