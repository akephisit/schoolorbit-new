import test from 'node:test';
import assert from 'node:assert/strict';
import {
	coverageCounts,
	coverageSelections,
	coverageReady,
	validDefaultContext,
	coverageLessonLabels
} from '../../src/lib/staff-leave/coverage.ts';

const assignment = {
	replacementTeacherId: 'teacher',
	mode: 'substitution',
	taughtOfferingId: 'english',
	taughtActivity: null,
	returnLessonKey: null,
	consent: 'confirmed'
};
const lessons = [
	{ lessonKey: 'math-class', assignment },
	{ lessonKey: 'second-day', assignment: null }
];
test('daily counts distinguish selected and confirmed and edited teaching facts invalidate displayed consent', () => {
	const plan = { days: [{ lessons }] };
	const selections = coverageSelections(plan);
	assert.deepEqual(coverageCounts(lessons), { total: 2, selected: 1, confirmed: 1 });
	assert.deepEqual(coverageCounts(lessons, [{ ...selections[0], taughtActivity: 'enrichment' }]), {
		total: 2,
		selected: 1,
		confirmed: 0
	});
	assert.deepEqual(coverageCounts(lessons, []), { total: 2, selected: 0, confirmed: 0 });
	assert.deepEqual(selections, [
		{
			lessonKey: 'math-class',
			replacementTeacherId: 'teacher',
			mode: 'substitution',
			taughtOfferingId: 'english',
			taughtActivity: null,
			returnLessonKey: null
		}
	]);
});
test('ordinary submission needs all assignments while confirmation remains a later approval gate', () => {
	const plan = {
		timetableState: 'ready',
		state: 'awaiting_confirmation',
		days: [
			{
				lessons: lessons.map((lesson) => ({
					...lesson,
					assignment: { ...assignment, consent: 'pending' }
				}))
			}
		]
	};
	assert.equal(coverageReady(plan), true);
	assert.equal(coverageReady({ ...plan, days: [{ lessons }] }), false);
	assert.equal(coverageReady({ ...plan, days: [] }), true);
	assert.equal(coverageReady({ ...plan, timetableState: 'missing', days: [] }), false);
});
test('school defaults require the exact authorized year-term pair and never choose first context', () => {
	const contexts = [
		{ academicYearId: 'year-a', academicTermId: 'term-a' },
		{ academicYearId: 'year-b', academicTermId: 'term-b' }
	];
	assert.equal(
		validDefaultContext({ contexts, defaultAcademicYearId: null, defaultAcademicTermId: null }),
		null
	);
	assert.equal(
		validDefaultContext({
			contexts,
			defaultAcademicYearId: 'year-a',
			defaultAcademicTermId: 'term-b'
		}),
		null
	);
	assert.equal(
		validDefaultContext({
			contexts,
			defaultAcademicYearId: 'year-b',
			defaultAcademicTermId: 'term-b'
		}),
		contexts[1]
	);
});

test('optional absent assignment is unselected and group labels retain custom identity without repeating known subject or room', () => {
	assert.deepEqual(coverageCounts([{ lessonKey: 'empty' }]), {
		total: 1,
		selected: 0,
		confirmed: 0
	});
	assert.deepEqual(
		coverageLessonLabels({
			offeringName: 'คณิตศาสตร์',
			learningGroupName: 'คณิตศาสตร์ ม.2/1',
			homeroomNames: ['ม.2/1'],
			roomName: 'ห้อง201'
		}),
		{ title: 'คณิตศาสตร์ ม.2/1', details: 'ห้อง201' }
	);
	assert.deepEqual(
		coverageLessonLabels({
			offeringName: 'ภาษาอังกฤษ',
			learningGroupName: 'กลุ่มเลือกเสรี A',
			homeroomNames: ['ม.2/1'],
			roomName: null
		}),
		{ title: 'กลุ่มเลือกเสรี A', details: 'ภาษาอังกฤษ · ม.2/1' }
	);
});
