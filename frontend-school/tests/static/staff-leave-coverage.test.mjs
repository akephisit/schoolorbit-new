import test from 'node:test';
import assert from 'node:assert/strict';
import {
	coverageCounts,
	coverageByOriginalTeacher,
	coverageReturnCaption,
	coverageSelectionMatches,
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
test('coverage groups by teacher identity before date while keeping every original lesson key', () => {
	const days = [
		{
			date: '2026-10-19',
			lessons: [
				{ lessonKey: 'a-one', teacherId: 'a', originalTeacherName: 'ครูสาธิต' },
				{ lessonKey: 'b-one', teacherId: 'b', originalTeacherName: 'ครูสาธิต' }
			]
		},
		{ date: '2026-10-20', lessons: [{ lessonKey: 'b-two', teacherId: 'b' }] }
	];
	const groups = coverageByOriginalTeacher({ days }, { ownerId: 'b', ownerName: 'ครูสาธิต' });
	assert.deepEqual(
		groups.map((group) => group.teacherId),
		['b', 'a']
	);
	assert.deepEqual(
		groups[0].days.map((day) => day.lessons.map((lesson) => lesson.lessonKey)),
		[['b-one'], ['b-two']]
	);
	assert.deepEqual(
		groups[1].days.map((day) => day.lessons.map((lesson) => lesson.lessonKey)),
		[['a-one']]
	);
});
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

test('unchanged confirmed rows tolerate omitted optional fields while only edited rows lose displayed consent', () => {
	const first = {
		lessonKey: 'first',
		assignment: {
			replacementTeacherId: 'teacher',
			mode: 'substitution',
			consent: 'confirmed',
			taughtOfferingId: 'english'
		}
	};
	const second = {
		lessonKey: 'second',
		assignment: {
			replacementTeacherId: 'teacher',
			mode: 'substitution',
			consent: 'confirmed',
			taughtActivity: 'reading'
		}
	};
	const selections = coverageSelections({ days: [{ lessons: [first, second] }] });
	assert.equal(coverageSelectionMatches(first, selections[0]), true);
	selections[1].taughtActivity = 'changed activity';
	assert.equal(coverageSelectionMatches(second, selections[1]), false);
	assert.deepEqual(coverageCounts([first, second], selections), {
		total: 2,
		selected: 2,
		confirmed: 1
	});
});

test('reciprocal caption distinguishes actual return teaching from displaced slot and omits unknown parts', () => {
	assert.equal(
		coverageReturnCaption({ offeringName: 'คณิตศาสตร์' }, { returnLesson: null }, '22 ต.ค. 2569'),
		'คณิตศาสตร์ คืน 22 ต.ค. 2569'
	);
	assert.equal(
		coverageReturnCaption(
			{ offeringName: 'คณิตศาสตร์' },
			{
				returnLesson: {
					periodName: 'คาบ 9',
					startTime: '10:00:00',
					endTime: '10:50:00',
					offeringName: 'ภาษาอังกฤษ'
				}
			},
			'22 ต.ค. 2569'
		),
		'คณิตศาสตร์ คืน 22 ต.ค. 2569 · คาบ 9 · 10:00–10:50 (แทนคาบภาษาอังกฤษ)'
	);
	assert.equal(
		coverageReturnCaption({ offeringName: null }, {}, '22 ต.ค. 2569'),
		'กิจกรรมตามตาราง คืน 22 ต.ค. 2569'
	);
});
