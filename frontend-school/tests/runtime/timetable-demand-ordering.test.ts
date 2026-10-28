import assert from 'node:assert/strict';
import test from 'node:test';
import {
	orderTimetableHomerooms,
	relatedSynchronizedDemands,
	timetableDemandCategory,
	timetableTraySections
} from '../../src/lib/academic/timetable/demand-ordering.ts';
import { makeSortingFixture, sortingIds } from '../e2e/timetable-sorting-fixtures.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

const fixture = makeSortingFixture;
const ids = (sections: ReturnType<typeof timetableTraySections>) =>
	sections.flatMap((section) =>
		section.categories.flatMap((category) => category.items.map((item) => item.id))
	);

test('homeroom categories precede natural codes, and synchronized activities share the final category', () => {
	const data = fixture();
	const ordinary = data.ordinaryDemands.filter((item) =>
		item.homeroomIds.includes(sortingIds.room2)
	);
	const sync = data.synchronizedDemands.filter((item) =>
		item.intendedHomeroomIds.includes(sortingIds.room2)
	);
	const sections = timetableTraySections(
		'homeroom',
		ordinary,
		sync,
		data.learningGroups,
		data.homerooms
	);
	assert.deepEqual(
		sections.map((section) => section.gradeLabel),
		[null]
	);
	assert.deepEqual(
		sections[0].categories.map((category) => category.category),
		['BASIC', 'ADDITIONAL', 'OTHER', 'ACTIVITY']
	);
	assert.deepEqual(
		ids(sections),
		[
			timetableIds.groupA,
			timetableIds.groupB,
			sortingIds.additional,
			sortingIds.unknown,
			sortingIds.courseActivity,
			`offering:${sortingIds.sync}`,
			sortingIds.activity
		].map((id) => (id.startsWith('offering:') ? id : `group:${id}`))
	);
});

test('teacher ordering is grade, then category, code and numeric room, with each multi-target item once', () => {
	const data = fixture();
	const sections = timetableTraySections(
		'teacher',
		data.ordinaryDemands,
		data.synchronizedDemands,
		data.learningGroups,
		data.homerooms
	);
	assert.deepEqual(
		sections.map((section) => section.gradeLabel),
		['ป.2', 'ม.1', 'ม.2', 'ไม่ระบุระดับชั้น']
	);
	assert.deepEqual(
		sections[0].categories.map((category) => category.category),
		['BASIC', 'ADDITIONAL', 'ACTIVITY']
	);
	assert.deepEqual(
		sections[0].categories[0].items.map((item) => item.id),
		[`group:${sortingIds.multi}`, `group:${sortingIds.primaryBasic}`]
	);
	assert.deepEqual(
		sections[1].categories[0].items.map((item) => item.id),
		[
			`group:${timetableIds.groupA}`,
			`group:${sortingIds.room10Group}`,
			`group:${timetableIds.groupB}`
		]
	);
	assert.equal(ids(sections).filter((id) => id === `group:${sortingIds.multi}`).length, 1);
	assert.equal(ids(sections).at(-1), `group:${sortingIds.missing}`);
});

test('null and unknown course types remain other courses; activity kind has priority', () => {
	const data = fixture();
	const group = data.learningGroups.find((item) => item.id === sortingIds.unknown)!;
	assert.equal(timetableDemandCategory(group), 'OTHER');
	assert.equal(timetableDemandCategory({ ...group, subjectType: 'CUSTOM' }), 'OTHER');
	assert.equal(
		timetableDemandCategory({ ...group, subjectType: 'BASIC', offeringKind: 'activity' }),
		'ACTIVITY'
	);
	assert.equal(timetableDemandCategory({ ...group, subjectType: 'ACTIVITY' }), 'ACTIVITY');
});

test('learning group shares only activities intersecting its actual target rooms, including no-target refusal', () => {
	const data = fixture();
	const group = data.learningGroups.find((item) => item.id === sortingIds.multi)!;
	assert.deepEqual(
		relatedSynchronizedDemands(group, data.synchronizedDemands)
			.map((item) => item.learningOfferingId)
			.sort(),
		[sortingIds.syncGrade2, sortingIds.syncPrimary].sort()
	);
	assert.deepEqual(
		relatedSynchronizedDemands({ ...group, homeroomIds: [] }, data.synchronizedDemands),
		[]
	);
	assert.deepEqual(relatedSynchronizedDemands(undefined, data.synchronizedDemands), []);
});

test('homeroom rows use grade metadata and numeric room values even with custom codes and names', () => {
	const data = fixture();
	assert.deepEqual(
		orderTimetableHomerooms(data.homerooms).map((room) => room.id),
		[sortingIds.primary, sortingIds.room2, sortingIds.room10, sortingIds.grade2]
	);
});

test('sorting is stable under source shuffling and never mutates source arrays or target lists', () => {
	const data = fixture(),
		before = structuredClone(data);
	const expected = timetableTraySections(
		'teacher',
		data.ordinaryDemands,
		data.synchronizedDemands,
		data.learningGroups,
		data.homerooms
	);
	const shuffled = timetableTraySections(
		'teacher',
		[...data.ordinaryDemands].reverse(),
		[...data.synchronizedDemands].reverse(),
		[...data.learningGroups].reverse(),
		[...data.homerooms].reverse()
	);
	assert.deepEqual(shuffled, expected);
	assert.deepEqual(data, before);
});
