import assert from 'node:assert/strict';
import test from 'node:test';
import {
	curriculumEditionLabel,
	programsForGrade,
	studyProgramLabel
} from '../../src/lib/academic-core/curriculum-presentation.ts';

test('a migrated numeric edition has an amendment label without a calendar expiry', () => {
	assert.equal(
		curriculumEditionLabel({ editionName: '2569', revisionYear: 2569 }),
		'ฉบับปรับปรุง พุทธศักราช 2569'
	);
	assert.equal(
		curriculumEditionLabel({ editionName: 'ฉบับทดลอง', revisionYear: null }),
		'ฉบับทดลอง'
	);
});

test('grade selection retains both published editions of a plan and distinguishes them', () => {
	const plans = [2569, 2572].map((year) => ({
		id: `plan-${year}`,
		name: 'วิทยาศาสตร์-คณิตศาสตร์',
		editionName: String(year),
		revisionYear: year,
		levelName: 'ระดับมัธยมศึกษาตอนต้น',
		gradeLevelIds: ['M1', 'M2', 'M3']
	}));
	assert.deepEqual(programsForGrade(plans, 'M1'), plans);
	assert.deepEqual(programsForGrade(plans, 'M4'), []);
	assert.notEqual(studyProgramLabel(plans[0]), studyProgramLabel(plans[1]));
	assert.match(studyProgramLabel(plans[0]), /2569/);
});
