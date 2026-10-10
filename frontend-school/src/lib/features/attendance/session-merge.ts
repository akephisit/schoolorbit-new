import type { AttendanceDetail } from '#lib/api/attendance.js';

type Record = AttendanceDetail['students'][number];
export type AttendanceConflict = {
	studentId: string;
	local: Record;
	latest: Record;
	fields: ('result' | 'note')[];
};
export function copyAttendanceDetail(detail: AttendanceDetail): AttendanceDetail {
	return {
		...detail,
		session: { ...detail.session },
		students: detail.students.map((row) => ({ ...row }))
	};
}
export function mergeAttendanceDetail(
	base: AttendanceDetail,
	local: AttendanceDetail,
	latest: AttendanceDetail
) {
	const bases = new Map(base.students.map((row) => [row.studentId, row]));
	const locals = new Map(local.students.map((row) => [row.studentId, row]));
	const conflicts: AttendanceConflict[] = [];
	const students = latest.students.map((row) => {
		const before = bases.get(row.studentId),
			edited = locals.get(row.studentId);
		const merged = { ...row };
		if (!before || !edited) return merged;
		const fields: AttendanceConflict['fields'] = [];
		if (edited.result !== before.result) {
			if (row.result !== before.result && row.result !== edited.result) fields.push('result');
			merged.result = edited.result;
		}
		if (edited.note !== before.note) {
			if (row.note !== before.note && row.note !== edited.note) fields.push('note');
			merged.note = edited.note;
		}
		if (fields.length)
			conflicts.push({
				studentId: row.studentId,
				local: { ...edited },
				latest: { ...row },
				fields
			});
		return merged;
	});
	return { detail: { ...latest, students }, conflicts };
}
