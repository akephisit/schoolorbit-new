import assert from 'node:assert/strict';
import test from 'node:test';

import {
	loadHomeroomCollections,
	loadStudentYearCollections,
	loadTimetableCollections
} from '../../src/lib/workspaces/academic-batch.ts';

type RecordedCall = { name: string; contextId: string; signal: AbortSignal | undefined };

function countCalls(calls: RecordedCall[], name: string): number {
	return calls.filter((call) => call.name === name).length;
}

test('timetable collection calls stay fixed for 300 offerings and share one signal', async () => {
	const calls: RecordedCall[] = [];
	const controller = new AbortController();
	const offerings = Array.from({ length: 300 }, (_, index) => ({ id: `offering-${index}` }));
	const deps = {
		async listLearningOfferings(termId: string, options?: { signal?: AbortSignal }) {
			calls.push({ name: 'offerings', contextId: termId, signal: options?.signal });
			return offerings;
		},
		async listLearningGroupsForTerm(termId: string, options?: { signal?: AbortSignal }) {
			calls.push({ name: 'groups', contextId: termId, signal: options?.signal });
			return [
				{ id: 'group-1', learningOfferingId: 'offering-1' },
				{ id: 'group-2', learningOfferingId: 'offering-1' }
			];
		},
		async listHomerooms(yearId: string, options?: { signal?: AbortSignal }) {
			calls.push({ name: 'homerooms', contextId: yearId, signal: options?.signal });
			return [{ id: 'homeroom-1' }];
		}
	};

	const result = await loadTimetableCollections(deps, 'term-1', 'year-1', controller.signal);

	assert.equal(result.offerings.length, 300);
	assert.equal(result.groupsByOfferingId.get('offering-1')?.length, 2);
	assert.equal(countCalls(calls, 'offerings'), 1);
	assert.equal(countCalls(calls, 'groups'), 1);
	assert.equal(countCalls(calls, 'homerooms'), 1);
	assert.ok(calls.every((call) => call.signal === controller.signal));
});

test('student-year collection batches placements and options once for every parent count', async () => {
	const calls: RecordedCall[] = [];
	const controller = new AbortController();
	const studentYears = Array.from({ length: 300 }, (_, index) => ({ id: `student-year-${index}` }));
	const placements = studentYears.map((record, index) => ({
		id: `placement-${index}`,
		studentAcademicYearId: record.id
	}));
	const record = (name: string, contextId: string, signal: AbortSignal | undefined) =>
		calls.push({ name, contextId, signal });
	const deps = {
		async listStudentAcademicYears(yearId: string, options?: { signal?: AbortSignal }) {
			record('studentYears', yearId, options?.signal);
			return studentYears;
		},
		async listPlacementsForAcademicYear(yearId: string, options?: { signal?: AbortSignal }) {
			record('placements', yearId, options?.signal);
			return placements;
		},
		async listHomerooms(yearId: string, options?: { signal?: AbortSignal }) {
			record('homerooms', yearId, options?.signal);
			return [];
		},
		async listGradeLevelOptions(yearId: string, options?: { signal?: AbortSignal }) {
			record('gradeLevels', yearId, options?.signal);
			return [];
		},
		async listStudyProgramOptionsForAcademicYear(
			yearId: string,
			options?: { signal?: AbortSignal }
		) {
			record('programs', yearId, options?.signal);
			return [];
		}
	};

	const result = await loadStudentYearCollections(deps, 'year-1', controller.signal);

	assert.equal(result.studentYears.length, 300);
	assert.equal(result.placementsByStudentYearId.size, 300);
	assert.equal(result.placementsByStudentYearId.get('student-year-299')?.length, 1);
	for (const name of ['studentYears', 'placements', 'homerooms', 'gradeLevels', 'programs'])
		assert.equal(countCalls(calls, name), 1, name);
	assert.equal(
		countCalls(calls, 'students'),
		0,
		'candidate students must load only after create opens'
	);
	assert.ok(calls.every((call) => call.signal === controller.signal));
});

test('homeroom collection batches advisor assignments once and indexes them by homeroom', async () => {
	const calls: RecordedCall[] = [];
	const controller = new AbortController();
	const homerooms = Array.from({ length: 300 }, (_, index) => ({ id: `homeroom-${index}` }));
	const advisors = homerooms.map((room, index) => ({
		id: `advisor-${index}`,
		homeroomId: room.id,
		userId: `staff-${index}`,
		role: 'primary'
	}));
	const record = (name: string, contextId: string, signal: AbortSignal | undefined) =>
		calls.push({ name, contextId, signal });
	const deps = {
		async listHomerooms(yearId: string, options?: { signal?: AbortSignal }) {
			record('homerooms', yearId, options?.signal);
			return homerooms;
		},
		async listHomeroomAdvisorsForAcademicYear(yearId: string, options?: { signal?: AbortSignal }) {
			record('advisors', yearId, options?.signal);
			return advisors;
		},
		async listGradeLevelOptions(yearId: string, options?: { signal?: AbortSignal }) {
			record('gradeLevels', yearId, options?.signal);
			return [];
		},
		async listStudyProgramOptionsForAcademicYear(
			yearId: string,
			options?: { signal?: AbortSignal }
		) {
			record('programs', yearId, options?.signal);
			return [];
		},
		async listStaffOptions(options?: { signal?: AbortSignal }) {
			record('staff', 'all', options?.signal);
			return [];
		}
	};

	const result = await loadHomeroomCollections(deps, 'year-1', controller.signal);

	assert.equal(result.homerooms.length, 300);
	assert.equal(result.advisorsByHomeroomId.size, 300);
	assert.equal(result.advisorsByHomeroomId.get('homeroom-299')?.[0]?.userId, 'staff-299');
	for (const name of ['homerooms', 'advisors', 'gradeLevels', 'programs'])
		assert.equal(countCalls(calls, name), 1, name);
	assert.equal(
		countCalls(calls, 'staff'),
		0,
		'staff options must load only after advisor dialog opens'
	);
	assert.ok(calls.every((call) => call.signal === controller.signal));
});
