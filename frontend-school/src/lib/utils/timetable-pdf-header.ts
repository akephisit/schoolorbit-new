import type { TimetableBlockWorkspaceHomeroom } from '#lib/api/timetable.js';

/** Normalize labels from academic context without repeating their Thai prefixes. */
export function timetablePdfHeaderContext(termName: string, yearName: string): string {
	const term = termName.trim().replace(/^(?:ภาคเรียน(?:ที่)?\s*)+/, '');
	const year = yearName.trim().replace(/^(?:ปีการศึกษา\s*)+/, '');
	return `ภาคเรียนที่${term ? ` ${term}` : ''} ปีการศึกษา${year ? ` ${year}` : ''}`;
}

export function timetablePdfHomeroomHeader(room: TimetableBlockWorkspaceHomeroom): string {
	const gradeNames: Record<string, string> = {
		kindergarten: 'อนุบาล',
		primary: 'ประถมศึกษาปีที่',
		secondary: 'มัธยมศึกษาปีที่'
	};
	const grade = gradeNames[room.gradeLevelType];
	if (grade && room.gradeLevelYear > 0) {
		const roomNumber = room.roomNumber?.trim() || room.name.match(/\/\s*(.+)$/)?.[1];
		return `ระดับชั้น${grade} ${room.gradeLevelYear}${roomNumber ? `/${roomNumber}` : ''}`;
	}
	const shortName = room.name.trim().match(/^(อ|ป|ม)\.\s*(\d+(?:\/\S+)?)$/);
	if (shortName) {
		const name = { อ: 'อนุบาล', ป: 'ประถมศึกษาปีที่', ม: 'มัธยมศึกษาปีที่' }[shortName[1]];
		return `ระดับชั้น${name} ${shortName[2]}`;
	}
	return room.name.trim();
}
