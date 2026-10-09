import type { Homeroom, HomeroomPlacement, StudentAcademicYear } from '#lib/api/academic-core.js';

const natural = new Intl.Collator('th', { numeric: true });
function levelRank(value: string): number {
	if (/^(?:K\d|อ\.|อนุบาล)/u.test(value)) return 0;
	if (/^(?:P\d|ป\.|ประถม)/u.test(value)) return 1;
	if (/^(?:M\d|ม\.|มัธยม)/u.test(value)) return 2;
	return 3;
}

function homeroomGrade(code: string): string {
	// Migrated codes can include the academic year, e.g. 69-M1-2 instead of M1-2.
	return code.match(/(?:^|-)((?:[KPM]|[อปม]\.)\d+)(?=[-/]|$)/u)?.[1] ?? code.split(/[-/]/u)[0];
}

/** Canonical room codes retain the grade even when a room has a custom display name. */
export function compareHomerooms(left: Homeroom, right: Homeroom): number {
	const leftGrade = homeroomGrade(left.code);
	const rightGrade = homeroomGrade(right.code);
	return (
		levelRank(leftGrade) - levelRank(rightGrade) ||
		natural.compare(leftGrade, rightGrade) ||
		Number(!left.roomNumber) - Number(!right.roomNumber) ||
		natural.compare(left.roomNumber ?? '', right.roomNumber ?? '') ||
		natural.compare(left.code, right.code) ||
		left.id.localeCompare(right.id)
	);
}

export function orderStudentYears(
	students: StudentAcademicYear[],
	placements: ReadonlyMap<string, HomeroomPlacement[]>,
	rooms: Homeroom[]
): StudentAcademicYear[] {
	const roomById = new Map(rooms.map((room) => [room.id, room]));
	const active = new Map(
		students.map((student) => [
			student.id,
			placements
				.get(student.id)
				?.find((placement) => ['current', 'planned'].includes(placement.status))
		])
	);
	return [...students].sort((left, right) => {
		const grade =
			levelRank(left.gradeLevelName) - levelRank(right.gradeLevelName) ||
			natural.compare(left.gradeLevelName, right.gradeLevelName);
		if (grade) return grade;
		const a = active.get(left.id),
			b = active.get(right.id);
		const aRoom = a ? roomById.get(a.homeroomId) : undefined;
		const bRoom = b ? roomById.get(b.homeroomId) : undefined;
		return (
			Number(!aRoom) - Number(!bRoom) ||
			(aRoom && bRoom ? compareHomerooms(aRoom, bRoom) : 0) ||
			(a?.classNumber ?? Number.MAX_SAFE_INTEGER) - (b?.classNumber ?? Number.MAX_SAFE_INTEGER) ||
			Number(!left.studentCode) - Number(!right.studentCode) ||
			natural.compare(left.studentCode ?? '', right.studentCode ?? '') ||
			natural.compare(left.studentName, right.studentName) ||
			left.id.localeCompare(right.id)
		);
	});
}
