import assert from 'node:assert/strict';
import test from 'node:test';
import {
	copyAttendanceDetail,
	mergeAttendanceDetail
} from '../../src/lib/features/attendance/session-merge.ts';
import type { AttendanceDetail } from '../../src/lib/api/attendance.ts';
function detail(): AttendanceDetail {
	const session: AttendanceDetail['session'] = {
		id: 'round',
		academicTermId: 'term',
		date: '2026-10-09',
		kind: 'flag',
		sourceKey: 'room',
		title: 'Round',
		teacherIds: ['teacher'],
		homeroomId: null,
		learningGroupId: null,
		offeringId: null,
		specialRoundId: null,
		startTime: '08:00:00',
		endTime: '08:00:00',
		countOverride: null,
		cancelled: false,
		cancellationReason: '',
		savedAt: null,
		savedBy: null,
		rowVersion: 1
	};
	return {
		session,
		counted: true,
		writable: true,
		students: ['one', 'two'].map((studentId) => ({
			studentId,
			studentAcademicYearId: studentId,
			classNumber: 1,
			displayName: studentId,
			result: 'unchecked',
			origin: 'unchecked',
			note: '',
			observedAt: null,
			arrivalAt: null,
			evidenceFileId: null,
			rowVersion: 1
		}))
	};
}
test('nonoverlapping teacher and scan changes retain latest evidence and revision', () => {
	const base = detail(),
		local = copyAttendanceDetail(base),
		latest = copyAttendanceDetail(base);
	local.students[0].note = 'teacher note';
	local.students[0].result = 'present';
	latest.session.rowVersion = 2;
	latest.students[1].result = 'late';
	latest.students[1].evidenceFileId = 'accepted-evidence';
	const merged = mergeAttendanceDetail(base, local, latest);
	assert.equal(merged.conflicts.length, 0);
	assert.equal(merged.detail.session.rowVersion, 2);
	assert.equal(merged.detail.students[0].note, 'teacher note');
	assert.equal(merged.detail.students[1].result, 'late');
	assert.equal(merged.detail.students[1].evidenceFileId, 'accepted-evidence');
	assert.equal(base.students[0].note, '');
});
test('overlapping fields require a choice while unrelated fields still merge', () => {
	const base = detail(),
		local = copyAttendanceDetail(base),
		latest = copyAttendanceDetail(base);
	local.students[0].result = 'present';
	local.students[0].note = 'local note';
	latest.students[0].result = 'late';
	latest.students[1].note = 'remote note';
	const merged = mergeAttendanceDetail(base, local, latest);
	assert.deepEqual(
		merged.conflicts.map((row) => row.fields),
		[['result']]
	);
	assert.equal(merged.detail.students[0].note, 'local note');
	assert.equal(merged.detail.students[1].note, 'remote note');
	assert.equal(merged.conflicts[0].latest.result, 'late');
});
test('matching concurrent edits do not conflict and removed roster members cannot be reintroduced', () => {
	const base = detail(),
		local = copyAttendanceDetail(base),
		latest = copyAttendanceDetail(base);
	local.students[0].result = latest.students[0].result = 'present';
	latest.students.pop();
	const merged = mergeAttendanceDetail(base, local, latest);
	assert.equal(merged.conflicts.length, 0);
	assert.equal(merged.detail.students.length, 1);
});
