import assert from 'node:assert/strict';
import test from 'node:test';
import {
	canUpdateHomeroomCapacity,
	homeroomCapacityError,
	bellScheduleBlockMessage
} from '../../src/lib/academic-core/period-setup.ts';
import { groupReadinessFindings } from '../../src/lib/academic/lifecycle/readiness-presentation.ts';

test('capacity accepts only editable years and whole counts covering pupils', () => {
	for (const status of ['planning', 'ready', 'active'] as const)
		assert.equal(canUpdateHomeroomCapacity(status), true);
	for (const status of ['closing', 'closed', 'archived'] as const)
		assert.equal(canUpdateHomeroomCapacity(status), false);
	for (const value of [0, -1, 1.5, NaN, Infinity]) assert.ok(homeroomCapacityError(value, 0));
	assert.ok(homeroomCapacityError(29, 30));
	assert.equal(homeroomCapacityError(30, 30), '');
	assert.equal(homeroomCapacityError(1, null), '');
});

test('schedule draft-reference guidance distinguishes managing drafts from cloning', () => {
	assert.match(bellScheduleBlockMessage('timetable_references'), /จัดการฉบับร่างเดิม/);
	assert.match(bellScheduleBlockMessage('already_used'), /คัดลอก/);
	assert.equal(bellScheduleBlockMessage(null), '');
});

test('readiness keeps every authoritative finding, detail, count and target link', () => {
	const findings = [
		{
			code: 'opening.room_capacity',
			severity: 'blocking' as const,
			count: 3,
			message: 'ม.1/1 มี 43 คน ความจุ 40 คน',
			resolutionUrl: '/staff/academic/homerooms?academicYearId=target'
		},
		{
			code: 'opening.calendar_settings',
			severity: 'blocking' as const,
			count: 1,
			message: 'ยังไม่บันทึกวันนับเข้าเรียนของภาคเรียนที่ 2',
			resolutionUrl: '/staff/attendance/settings?academicYearId=target&academicTermId=term2'
		},
		{
			code: 'opening.room_capacity',
			severity: 'blocking' as const,
			count: 2,
			message: 'ม.2/1 มี 42 คน ความจุ 40 คน',
			resolutionUrl: '/staff/academic/homerooms?academicYearId=target'
		}
	];
	const grouped = groupReadinessFindings(findings);
	assert.equal(grouped[0].key, 'time');
	assert.equal(grouped[1].key, 'roster');
	assert.deepEqual(grouped[0].findings, [findings[1]]);
	assert.deepEqual(grouped[1].findings, [findings[0], findings[2]]);
	assert.equal(grouped[1].findings[0], findings[0]);
	for (const code of ['opening.bell_period_days_missing', 'opening.bell_period_overlap']) {
		const granular = { ...findings[1], code };
		assert.deepEqual(groupReadinessFindings([granular]), [
			{ key: 'time', label: 'ปฏิทินและตารางเวลา', findings: [granular] }
		]);
	}
});
