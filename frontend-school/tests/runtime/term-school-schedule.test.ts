import assert from 'node:assert/strict';
import test from 'node:test';
import {
	parsePeriodSchoolDays,
	periodAllowsSchoolDay,
	periodDaysOutsideTerm,
	previousSchoolScheduleTerm,
	schoolScheduleSourceFields
} from '../../src/lib/academic-core/school-schedule.ts';
import type {
	AcademicTerm,
	AcademicYear,
	BellSchedulePeriodsSnapshot
} from '../../src/lib/api/academic-core.ts';

test('NULL means all selected term days; explicit weekdays keep their own meaning', () => {
	const days = ['MON', 'SAT'];
	assert.deepEqual(parsePeriodSchoolDays({ applicableDays: null }), { ok: true, days: null });
	assert.equal(periodAllowsSchoolDay({ isActive: true, applicableDays: null }, 'SAT', days), true);
	assert.equal(periodAllowsSchoolDay({ isActive: true, applicableDays: null }, 'SUN', days), false);
	assert.equal(
		periodAllowsSchoolDay({ isActive: true, applicableDays: 'MON' }, 'SAT', days),
		false
	);
	assert.equal(
		periodAllowsSchoolDay({ isActive: true, applicableDays: 'MON,SAT' }, 'MON', ['MON']),
		false
	);
	assert.equal(
		periodAllowsSchoolDay({ isActive: false, applicableDays: null }, 'MON', days),
		false
	);
	assert.deepEqual(
		periodDaysOutsideTerm([{ applicableDays: 'MON,SAT' }, { applicableDays: null }], ['MON']),
		['SAT']
	);
});

test('malformed stored period days are refused without silently normalizing them', () => {
	for (const applicableDays of ['', '{MON,TUE}', ' MON', 'mon', 'MON,,TUE', 'MON,MON', 'HOLIDAY']) {
		assert.equal(parsePeriodSchoolDays({ applicableDays }).ok, false, applicableDays);
		assert.equal(periodAllowsSchoolDay({ isActive: true, applicableDays }, 'MON', ['MON']), false);
	}
	assert.deepEqual(parsePeriodSchoolDays({ applicableDays: 'SAT,MON' }), {
		ok: true,
		days: ['MON', 'SAT']
	});
});

test('previous term source follows year and sequence, independent of input ordering', () => {
	const years = [
		{ id: 'old', year: 2569 },
		{ id: 'new', year: 2570 }
	] as AcademicYear[];
	const terms = [
		{ id: 'future', academicYearId: 'new', sequence: 3 },
		{ id: 'previous-year', academicYearId: 'old', sequence: 2 },
		{ id: 'first', academicYearId: 'new', sequence: 1 },
		{ id: 'target', academicYearId: 'new', sequence: 2 }
	] as AcademicTerm[];
	assert.equal(previousSchoolScheduleTerm(years[1], years, terms, terms[3])?.id, 'first');
	assert.equal(previousSchoolScheduleTerm(years[1], years, terms, terms[2])?.id, 'previous-year');
});

test('copy requests use the paired period snapshot revision and an independent weekday list', () => {
	const term = { id: 'source', rowVersion: 4 } as AcademicTerm;
	const snapshot = { rowVersion: 9 } as BellSchedulePeriodsSnapshot;
	const source = { term, snapshot, schoolDays: ['MON', 'SAT'] as const };
	const request = schoolScheduleSourceFields({ ...source, schoolDays: [...source.schoolDays] });
	assert.deepEqual(request, {
		sourceTermId: 'source',
		sourceTermRowVersion: 4,
		sourceBellScheduleRowVersion: 9,
		schoolDays: ['MON', 'SAT']
	});
	request.schoolDays.pop();
	assert.deepEqual(source.schoolDays, ['MON', 'SAT']);
});
